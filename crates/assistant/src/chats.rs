//! Conversations, kept per workspace in the app data dir (never in the workspace, so they don't
//! end up in git): `<data dir>/assistant/chats/<workspace hash>/<id>.json`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use plumbq_core::fs_util;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::types::{ChatMessage, ChatRole};
use crate::{Error, Result};

/// Older chats of a workspace are deleted when a new one is saved.
const KEEP: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Chat {
    pub id: String,
    pub title: String,
    /// RFC 3339.
    pub updated_at: String,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    pub updated_at: String,
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// A title from the first user message.
pub fn title(messages: &[ChatMessage]) -> String {
    let first = messages
        .iter()
        .find(|m| m.role == ChatRole::User)
        .map(|m| m.content.trim())
        .unwrap_or_default();
    let line = first.lines().next().unwrap_or_default();
    match line.char_indices().nth(60) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None if line.is_empty() => "New chat".into(),
        None => line.to_string(),
    }
}

/// Newest first.
pub fn list(data_dir: &Path, workspace: &Path) -> Result<Vec<ChatSummary>> {
    let dir = dir(data_dir, workspace);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut chats: Vec<ChatSummary> = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let path = entry?.path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        // Serde skips the messages, so the list never builds whole chats. One unreadable chat
        // shouldn't hide the others.
        match read_summary(&path) {
            Ok(summary) => chats.push(summary),
            Err(err) => tracing::warn!("skipping unreadable chat {}: {err}", path.display()),
        }
    }
    chats.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(chats)
}

pub fn get(data_dir: &Path, workspace: &Path, id: &str) -> Result<Chat> {
    read(&file(data_dir, workspace, id)?)
}

pub fn save(data_dir: &Path, workspace: &Path, chat: &Chat) -> Result<()> {
    let path = file(data_dir, workspace, &chat.id)?;
    fs_util::write_private(&path, serde_json::to_string(chat)?)?;
    trim(&dir(data_dir, workspace))
}

pub fn remove(data_dir: &Path, workspace: &Path, id: &str) -> Result<()> {
    match fs::remove_file(file(data_dir, workspace, id)?) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

fn read(path: &Path) -> Result<Chat> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn read_summary(path: &Path) -> Result<ChatSummary> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

/// Keeps the newest [`KEEP`] chats, by file time, without reading them.
fn trim(dir: &Path) -> Result<()> {
    let mut files: Vec<(SystemTime, PathBuf)> = fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .filter_map(|path| Some((fs::metadata(&path).ok()?.modified().ok()?, path)))
        .collect();
    if files.len() <= KEEP {
        return Ok(());
    }
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    for (_, path) in files.drain(KEEP..) {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn dir(data_dir: &Path, workspace: &Path) -> PathBuf {
    let hash = Sha256::digest(workspace.display().to_string().as_bytes());
    let key: String = hash.iter().take(8).map(|b| format!("{b:02x}")).collect();
    data_dir.join("assistant").join("chats").join(key)
}

fn file(data_dir: &Path, workspace: &Path, id: &str) -> Result<PathBuf> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(Error::Invalid(format!("invalid chat id {id}")));
    }
    Ok(dir(data_dir, workspace).join(format!("{id}.json")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::temp_dir;

    fn chat(id: &str, at: &str, text: &str) -> Chat {
        let messages = vec![ChatMessage::user(text)];
        Chat {
            id: id.into(),
            title: title(&messages),
            updated_at: at.into(),
            messages,
        }
    }

    #[test]
    fn saves_lists_newest_first_per_workspace_and_removes() {
        let data = temp_dir("chats");
        let (a, b) = (Path::new("/work/a"), Path::new("/work/b"));
        save(
            &data,
            a,
            &chat("one", "2026-10-11T10:00:00Z", "Organize my workspace"),
        )
        .unwrap();
        save(
            &data,
            a,
            &chat("two", "2026-10-12T10:00:00Z", "Why does login fail?"),
        )
        .unwrap();
        save(
            &data,
            b,
            &chat("three", "2026-10-12T11:00:00Z", "Other workspace"),
        )
        .unwrap();

        let ids: Vec<_> = list(&data, a).unwrap().into_iter().map(|c| c.id).collect();
        assert_eq!(ids, ["two", "one"]);
        assert_eq!(get(&data, a, "one").unwrap().title, "Organize my workspace");
        remove(&data, a, "two").unwrap();
        assert_eq!(list(&data, a).unwrap().len(), 1);
    }

    #[test]
    fn rejects_path_like_ids_and_titles_long_questions() {
        let data = temp_dir("chats-ids");
        assert!(get(&data, Path::new("/w"), "../x").is_err());
        let long = "x".repeat(100);
        assert!(title(&[ChatMessage::user(long)]).ends_with('…'));
        assert_eq!(title(&[]), "New chat");
    }

    #[test]
    fn keeps_at_most_keep_chats_per_workspace() {
        let data = temp_dir("chats-keep");
        let workspace = Path::new("/work/many");
        for n in 0..KEEP + 2 {
            let id = format!("c{n}");
            save(&data, workspace, &chat(&id, "2026-10-12T10:00:00Z", "hi")).unwrap();
        }
        assert_eq!(list(&data, workspace).unwrap().len(), KEEP);
    }
}
