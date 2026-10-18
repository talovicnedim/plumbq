//! Sent requests and their responses, kept on this machine, never in the workspace:
//! `<history dir>/<workspace>/<request>/<entry id>.json`.

mod follow;

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::send::Response;
use crate::{Error, Result, fs_util, time};

pub use follow::follow;

/// Older entries of a request are deleted when a new one is added.
const KEEP_PER_REQUEST: usize = 50;
/// Binary bodies above this (as base64, about 1 MB of bytes) aren't written to history, so 50
/// entries of large downloads don't fill the disk. The returned entry still has the body.
pub const MAX_BINARY_BASE64: usize = 1_400_000;
/// Text bodies are cut at this many bytes in history (and marked `truncated`, which the UI
/// shows), for the same reason. The returned entry still has the whole body.
const MAX_TEXT_BODY: usize = 1_000_000;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    /// Sortable: newer entries have larger ids.
    pub id: String,
    pub request_id: String,
    pub environment: Option<String>,
    /// RFC 3339.
    pub sent_at: String,
    pub response: Response,
}

/// A list row: everything but the bodies.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HistorySummary {
    pub id: String,
    pub environment: Option<String>,
    pub sent_at: String,
    pub method: String,
    pub url: String,
    pub status: u16,
    pub duration_ms: u32,
    /// Assertion results when it was sent.
    pub passed: u32,
    pub failed: u32,
}

/// Records a response (already masked by `send`). Returns the new entry.
pub fn add(
    history_dir: &Path,
    root: &Path,
    request_id: &str,
    environment: Option<&str>,
    response: Response,
) -> Result<HistoryEntry> {
    let entry = entry(request_id, environment, response);
    save(history_dir, root, &entry).map(|()| entry)
}

/// A new entry for a response that just arrived, not saved yet.
pub fn entry(request_id: &str, environment: Option<&str>, response: Response) -> HistoryEntry {
    let now = time::now();
    HistoryEntry {
        id: format!("{:012}{:09}", now.0, now.1),
        request_id: request_id.to_string(),
        environment: environment.map(str::to_string),
        sent_at: time::format(now),
        response,
    }
}

/// Saves `entry` and drops the oldest entries of its request over the limit.
pub fn save(history_dir: &Path, root: &Path, entry: &HistoryEntry) -> Result<()> {
    let dir = request_dir(history_dir, root, &entry.request_id);
    let large = entry.response.body_base64.len() > MAX_BINARY_BASE64
        || entry.response.body.len() > MAX_TEXT_BODY;
    let stored = match large {
        true => serde_json::to_string(&slim(entry.clone()))?,
        false => serde_json::to_string(entry)?,
    };
    fs_util::write_private(&dir.join(format!("{}.json", entry.id)), stored)?;
    prune(&dir)
}

/// What history keeps of a large response: no binary body, and the text body cut at a
/// character boundary.
fn slim(mut entry: HistoryEntry) -> HistoryEntry {
    let response = &mut entry.response;
    if response.body_base64.len() > MAX_BINARY_BASE64 {
        response.body_base64.clear();
    }
    if response.body.len() > MAX_TEXT_BODY {
        let cut = (0..=MAX_TEXT_BODY)
            .rev()
            .find(|i| response.body.is_char_boundary(*i))
            .unwrap_or(0);
        response.body.truncate(cut);
        response.truncated = true;
    }
    entry
}

/// Newest first. Entries that can't be read (e.g. cut off by a crash) are skipped.
pub fn list(history_dir: &Path, root: &Path, request_id: &str) -> Result<Vec<HistorySummary>> {
    let files = files_newest_first(history_dir, root, request_id)?;
    Ok(files.iter().filter_map(|path| read_summary(path)).collect())
}

/// The newest entry sent with `environment`. Reads entries newest first and stops at the first
/// match, so chaining doesn't read all of them. Unreadable entries are skipped, as in [`list`].
pub fn latest(
    history_dir: &Path,
    root: &Path,
    request_id: &str,
    environment: Option<&str>,
) -> Result<Option<HistoryEntry>> {
    #[derive(Deserialize)]
    struct StoredEnvironment {
        environment: Option<String>,
    }
    for path in files_newest_first(history_dir, root, request_id)? {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let matches = serde_json::from_str::<StoredEnvironment>(&text)
            .is_ok_and(|stored| stored.environment.as_deref() == environment);
        if !matches {
            continue;
        }
        match serde_json::from_str(&text) {
            Ok(entry) => return Ok(Some(entry)),
            Err(err) => tracing::warn!(
                "skipping unreadable history entry {}: {err}",
                path.display()
            ),
        }
    }
    Ok(None)
}

fn files_newest_first(history_dir: &Path, root: &Path, request_id: &str) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for dir in request_dirs(history_dir, root, request_id) {
        files.extend(entry_files(&dir)?);
    }
    files.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    Ok(files)
}

// Only the fields a list row needs: serde skips the bodies and headers, so a list of 50 large
// responses never holds them in memory.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredEntry {
    id: String,
    environment: Option<String>,
    sent_at: String,
    response: StoredResponse,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredResponse {
    status: u16,
    duration_ms: u32,
    sent: StoredSent,
    #[serde(default)]
    assertions: Vec<StoredAssertion>,
}

#[derive(Deserialize)]
struct StoredSent {
    method: String,
    url: String,
}

#[derive(Deserialize)]
struct StoredAssertion {
    passed: bool,
}

fn read_summary(path: &Path) -> Option<HistorySummary> {
    let parsed = fs::read_to_string(path)
        .map_err(Error::from)
        .and_then(|text| Ok(serde_json::from_str::<StoredEntry>(&text)?));
    let entry = match parsed {
        Ok(entry) => entry,
        Err(err) => {
            tracing::warn!(
                "skipping unreadable history entry {}: {err}",
                path.display()
            );
            return None;
        }
    };
    let response = entry.response;
    let passed = response.assertions.iter().filter(|a| a.passed).count();
    let failed = response.assertions.len() - passed;
    Some(HistorySummary {
        id: entry.id,
        environment: entry.environment,
        sent_at: entry.sent_at,
        method: response.sent.method,
        url: response.sent.url,
        status: response.status,
        duration_ms: response.duration_ms,
        passed: u32::try_from(passed).unwrap_or(u32::MAX),
        failed: u32::try_from(failed).unwrap_or(u32::MAX),
    })
}

pub fn get(history_dir: &Path, root: &Path, request_id: &str, id: &str) -> Result<HistoryEntry> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return Err(Error::Invalid(format!("invalid history id {id}")));
    }
    let path = request_dirs(history_dir, root, request_id)
        .into_iter()
        .map(|dir| dir.join(format!("{id}.json")))
        .find(|path| path.is_file())
        .ok_or_else(|| Error::NotFound(format!("history entry {id}")))?;
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn request_dir(history_dir: &Path, root: &Path, request_id: &str) -> PathBuf {
    history_dir
        .join(dir_name(&root.display().to_string()))
        .join(dir_name(request_id))
}

// The current directory, then the one older versions used (read only), when it exists.
fn request_dirs(history_dir: &Path, root: &Path, request_id: &str) -> Vec<PathBuf> {
    let legacy = history_dir
        .join(flatten(&root.display().to_string()))
        .join(flatten(request_id));
    let mut dirs = vec![request_dir(history_dir, root, request_id)];
    if legacy.is_dir() && legacy != dirs[0] {
        dirs.push(legacy);
    }
    dirs
}

/// A single safe directory name for any path or id that never collides: readable start plus a
/// hash of the original (`users/list.http` and `users_list.http` differ).
pub(crate) fn dir_name(text: &str) -> String {
    let hash = Sha256::digest(text.as_bytes());
    let short: String = hash.iter().take(6).map(|b| format!("{b:02x}")).collect();
    let readable: String = flatten(text).chars().take(60).collect();
    format!("{readable}-{short}")
}

/// How older versions named directories; lossy (`/` and `_` both become `_`), so only used to
/// find old history and cookie jars.
pub(crate) fn flatten(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '.' => c,
            _ => '_',
        })
        .collect::<String>()
        .trim_start_matches('.')
        .to_string()
}

// Newest first: entry ids sort by time.
fn entry_files(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    files.reverse();
    Ok(files)
}

fn prune(dir: &Path) -> Result<()> {
    for old in entry_files(dir)?.iter().skip(KEEP_PER_REQUEST) {
        fs::remove_file(old)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
