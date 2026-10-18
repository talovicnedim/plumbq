//! The AI activity log: one line per MCP tool call, from the built-in chat and outside AI apps
//! alike, so the user can see exactly what an AI looked at, changed and sent. Stored as JSON
//! lines in the app data dir, redacted before writing; both the app's MCP server and the stdio
//! one write it.

use std::fs;
use std::path::{Path, PathBuf};

use plumbq_core::fs_util;
use plumbq_core::permissions::Permission;
use serde::{Deserialize, Serialize};

use crate::Result;

const FILE: &str = "ai-activity.jsonl";
/// Entries kept when the file is trimmed.
const KEEP: usize = 1000;
/// The log is trimmed once it grows past this size, not on every write. That is room for more
/// than [`KEEP`] entries of the longest summary, so a trim only happens when there are more.
const TRIM_ABOVE_BYTES: u64 = 2_000_000;
/// A trim keeps no more than this, so even very long entries leave room before the next trim.
const KEEP_BYTES: usize = 1_000_000;
/// ...but always at least this many of the newest entries, however long they are.
const MIN_KEEP: usize = 100;
/// Every text field is cut at this many characters: summaries can be long, and the actor comes
/// from the AI client itself.
const MAX_CHARS: usize = 300;
/// [`record`] never writes a line this long, so a trim drops longer ones as damage.
const MAX_LINE_BYTES: usize = 16_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ActivityOutcome {
    /// Ran without asking (allowed by the settings). Logs from before the rename say `allowed`.
    #[serde(alias = "allowed")]
    Ran,
    /// The user approved it.
    Approved,
    /// The user denied it, or nobody answered in time.
    Denied,
    /// Refused by the settings: read-only, a Deny rule, a hidden environment.
    Blocked,
    /// Allowed, but it failed.
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEntry {
    /// RFC 3339.
    pub at: String,
    /// The AI client, e.g. `claude-code`.
    pub actor: String,
    #[serde(default)]
    pub from_chat: bool,
    pub workspace: String,
    pub tool: String,
    /// The environment the call used, if any.
    #[serde(default)]
    pub environment: Option<String>,
    pub permission: Permission,
    pub outcome: ActivityOutcome,
    /// One line: the action, or why it failed or was denied.
    pub summary: String,
}

fn path(data_dir: &Path) -> PathBuf {
    data_dir.join(FILE)
}

/// Appends an entry; the summary is redacted, and every text field is cut first.
pub fn record(data_dir: &Path, mut entry: ActivityEntry) -> Result<()> {
    entry.summary = cut(&redact(&entry.summary));
    entry.actor = cut(&entry.actor);
    entry.workspace = cut(&entry.workspace);
    entry.tool = cut(&entry.tool);
    entry.environment = entry.environment.as_deref().map(cut);
    let file = path(data_dir);
    fs_util::append_private(&file, &serde_json::to_string(&entry)?)?;
    if fs::metadata(&file).map(|m| m.len()).unwrap_or(0) > TRIM_ABOVE_BYTES {
        trim(&file)?;
    }
    Ok(())
}

/// Newest first, at most [`KEEP`].
pub fn list(data_dir: &Path, limit: u32) -> Result<Vec<ActivityEntry>> {
    let mut entries = read(&path(data_dir))?;
    entries.reverse();
    entries.truncate((limit as usize).min(KEEP));
    Ok(entries)
}

pub fn clear(data_dir: &Path) -> Result<()> {
    match fs::remove_file(path(data_dir)) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

/// Keeps the newest [`KEEP`] lines without parsing them: at most [`KEEP_BYTES`], but never
/// fewer than [`MIN_KEEP`]. Oversized lines are skipped, so one can't push the others out.
fn trim(file: &Path) -> Result<()> {
    let text = fs::read_to_string(file)?;
    let mut bytes = 0;
    let newest: Vec<&str> = text
        .lines()
        .rev()
        .filter(|line| !line.is_empty() && line.len() <= MAX_LINE_BYTES)
        .take(KEEP)
        .enumerate()
        .take_while(|(kept, line)| {
            bytes += line.len() + 1;
            *kept < MIN_KEEP || bytes <= KEEP_BYTES
        })
        .map(|(_, line)| line)
        .collect();
    let kept: String = newest
        .iter()
        .rev()
        .map(|line| format!("{line}\n"))
        .collect();
    fs_util::write_private(file, kept)?;
    Ok(())
}

fn read(file: &Path) -> Result<Vec<ActivityEntry>> {
    match fs::read_to_string(file) {
        Ok(text) => Ok(text
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(err.into()),
    }
}

fn cut(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default();
    match line.char_indices().nth(MAX_CHARS) {
        Some((i, _)) => format!("{}…", &line[..i]),
        None => line.to_string(),
    }
}

/// Hides what looks like a credential: the value after `Bearer`/`Basic`, after `token=`-like
/// keys, and long opaque strings. Secret values are already `{{name}}` in what agents see; this
/// catches literals an agent typed itself.
pub fn redact(text: &str) -> String {
    let mut out = Vec::new();
    let mut hide_next = false;
    for word in text.split(' ') {
        if hide_next && !word.is_empty() {
            out.push("•••".to_string());
            hide_next = false;
            continue;
        }
        let lower = word.to_lowercase();
        if lower == "bearer" || lower == "basic" {
            hide_next = true;
            out.push(word.to_string());
            continue;
        }
        out.push(redact_word(word));
    }
    out.join(" ")
}

fn redact_word(word: &str) -> String {
    const KEYS: [&str; 7] = ["token", "key", "secret", "password", "pwd", "auth", "sig"];
    if let Some((key, value)) = word.split_once('=')
        && !value.is_empty()
        && !value.starts_with("{{")
        && KEYS.iter().any(|k| key.to_lowercase().contains(k))
    {
        return format!("{key}=•••");
    }
    let opaque = word.len() >= 32
        && !word.contains("{{")
        && !word.contains('/')
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+' | '='));
    if opaque {
        return "•••".to_string();
    }
    word.to_string()
}

#[cfg(test)]
#[path = "activity_tests.rs"]
mod tests;
