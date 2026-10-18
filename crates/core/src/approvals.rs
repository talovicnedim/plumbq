//! Approval requests from AI agents. Inside the app the MCP server asks the user directly; the
//! stdio MCP server (a separate process) writes a request file here instead, the running app
//! shows it (Deny / Allow once / Always allow) and writes the answer next to it. No app running means no
//! answer, and the request is denied when it times out, so an agent can never approve itself
//! through the server. (Any other process of the same user can write files there; that is the
//! same trust boundary as the keychain and the workspace itself.)

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::permissions::{self, Permission};
use crate::text_diff::DiffLine;
use crate::{Error, Result, fs_util, time};

/// Unanswered requests are denied after this long, so an agent never waits forever.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// At most this many requests wait at once; older ones are removed, which denies them.
const KEEP: usize = 50;

/// The user's answer in the approval dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ApprovalAnswer {
    Deny,
    AllowOnce,
    /// Allow this call, and switch its kind of call to Allow in the AI permissions.
    AlwaysAllow,
}

/// "Always allow" as offered for one request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AlwaysAllowOffer {
    /// The rule that switches to Allow.
    pub permission: Permission,
    /// What the AI may then do without asking, e.g. "creating and editing requests and folders".
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRequest {
    pub id: String,
    /// RFC 3339.
    pub created_at: String,
    /// The AI client as it introduced itself, e.g. `claude-code`.
    pub actor: String,
    /// The AI client for people, e.g. `Claude Code`; the raw name when it isn't a known app.
    #[serde(default)]
    pub actor_label: String,
    /// The workspace root the change is for.
    pub workspace: String,
    /// The name the app shows for that workspace (see [`crate::workspace::name`]).
    #[serde(default)]
    pub workspace_name: String,
    pub tool: String,
    pub permission: Permission,
    /// Readable action, e.g. `send DELETE {{baseUrl}}/users/7`.
    pub action: String,
    pub environment: Option<String>,
    pub production: bool,
    /// Type the environment (or workspace) name to approve.
    pub strong: bool,
    /// The name to type when `strong`: the environment, or the workspace folder name.
    #[serde(default)]
    pub confirm_name: Option<String>,
    /// Offered as a third answer; `None` when a rule can't cover this call.
    #[serde(default)]
    pub always_allow: Option<AlwaysAllowOffer>,
    /// What exactly would happen: the request as it would be sent (secrets masked) or a diff of
    /// the file change.
    pub detail: String,
    /// `detail` line by line, with added and removed lines marked.
    #[serde(default)]
    pub lines: Vec<DiffLine>,
    /// Risks to point out above the detail, e.g. secrets going to a new host.
    #[serde(default)]
    pub warnings: Vec<String>,
    /// The agent's own explanation, if it gave one.
    pub reason: Option<String>,
}

fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join("approvals")
}

fn request_path(data_dir: &Path, id: &str) -> Result<PathBuf> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(Error::Invalid(format!("invalid approval id {id}")));
    }
    Ok(dir(data_dir).join(format!("{id}.json")))
}

/// A fresh id and timestamp for a request about to be asked, the name to type for it, and
/// whether "Always allow" is offered.
pub fn stamp(request: &mut ApprovalRequest) {
    request.id = uuid::Uuid::new_v4().simple().to_string();
    request.created_at = time::format(time::now());
    request.workspace_name = crate::workspace::name(Path::new(&request.workspace));
    request.confirm_name = confirm_name(request);
    request.always_allow = always_allow_offer(request);
}

/// The kind of call "Always allow" switches to Allow, or why it can't be used for `request`.
pub fn check_always_allow(request: &ApprovalRequest) -> Result<Permission> {
    always_allow_offer(request)
        .map(|offer| offer.permission)
        .ok_or_else(|| {
            Error::Invalid(
                "this change can't be allowed for good; allow it once or deny it".to_string(),
            )
        })
}

/// Strong requests and ones with warnings ask every time whatever the rules say (see
/// `permissions::decide`), so a rule can't cover them. Reads have no rule to switch.
fn always_allow_offer(request: &ApprovalRequest) -> Option<AlwaysAllowOffer> {
    let switchable = matches!(
        request.permission,
        Permission::SendSafe
            | Permission::SendWrite
            | Permission::EditFiles
            | Permission::ChangeEnvironment
    );
    (switchable && !request.strong && request.warnings.is_empty()).then(|| AlwaysAllowOffer {
        permission: request.permission,
        label: permissions::describe(request.permission).to_string(),
    })
}

/// Refuses to approve a strong request unless `typed` is its name. Checked here, not only in
/// the dialog, because production changes must never pass on a click.
pub fn check_typed(request: &ApprovalRequest, typed: Option<&str>) -> Result<()> {
    let Some(name) = confirm_name(request) else {
        return Ok(());
    };
    if typed.map(str::trim) == Some(name.as_str()) {
        return Ok(());
    }
    Err(Error::NeedsConfirmation(format!(
        "type {name} to allow this change"
    )))
}

fn confirm_name(request: &ApprovalRequest) -> Option<String> {
    if !request.strong {
        return None;
    }
    let folder = || {
        Path::new(&request.workspace)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| request.workspace.clone())
    };
    Some(request.environment.clone().unwrap_or_else(folder))
}

/// Files the request for the app and waits up to `timeout` for the user's answer.
pub async fn request_and_wait(
    data_dir: &Path,
    mut request: ApprovalRequest,
    timeout: Duration,
) -> Result<bool> {
    stamp(&mut request);
    let file = request_path(data_dir, &request.id)?;
    fs_util::write_private(&file, serde_json::to_string(&request)?)?;
    let filed = Filed {
        answer: file.with_extension("answer"),
        request: file,
    };

    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Ok(answer) = fs::read_to_string(&filed.answer) {
            return Ok(answer.trim() == "allow");
        }
        if tokio::time::Instant::now() >= deadline {
            return Ok(false);
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// A filed request and its answer, removed when the wait ends: also when the caller stops
/// waiting (the agent cancelled the call), so the app stops showing it.
struct Filed {
    request: PathBuf,
    answer: PathBuf,
}

impl Drop for Filed {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.request);
        let _ = fs::remove_file(&self.answer);
    }
}

/// Requests waiting for an answer, oldest first (for the app). Files left behind by a stdio
/// server that stopped while waiting are removed first.
pub fn pending(data_dir: &Path) -> Result<Vec<ApprovalRequest>> {
    let files = match prune(&dir(data_dir)) {
        Ok(files) => files,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };
    let mut requests: Vec<ApprovalRequest> = files
        .into_iter()
        .filter(|p| {
            p.extension().is_some_and(|x| x == "json") && !p.with_extension("answer").exists()
        })
        .filter_map(|p| fs::read_to_string(p).ok())
        .filter_map(|text| serde_json::from_str(&text).ok())
        .collect();
    requests.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    Ok(requests)
}

/// Deletes request and answer files older than the timeout (nobody waits for them any more)
/// and the oldest requests over [`KEEP`], by file time, without reading them. Returns the
/// files that are left.
fn prune(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let now = std::time::SystemTime::now();
    let mut kept: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let modified = entry.metadata()?.modified()?;
        let stale = now
            .duration_since(modified)
            .is_ok_and(|age| age > DEFAULT_TIMEOUT * 2);
        if stale {
            let _ = fs::remove_file(entry.path());
        } else {
            kept.push((modified, entry.path()));
        }
    }
    kept.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    let mut requests = 0;
    kept.retain(|(_, path)| {
        if path.extension().is_none_or(|x| x != "json") {
            return true;
        }
        requests += 1;
        if requests <= KEEP {
            return true;
        }
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(path.with_extension("answer"));
        false
    });
    Ok(kept.into_iter().map(|(_, path)| path).collect())
}

/// The user's answer (from the app). `typed` is what they typed to confirm a strong request.
pub fn answer(data_dir: &Path, id: &str, allow: bool, typed: Option<&str>) -> Result<()> {
    let file = request_path(data_dir, id)?;
    if !file.exists() {
        return Err(Error::NotFound(format!("approval request {id}")));
    }
    if allow {
        let request: ApprovalRequest = serde_json::from_str(&fs::read_to_string(&file)?)?;
        check_typed(&request, typed)?;
    }
    fs_util::write_private(
        &file.with_extension("answer"),
        if allow { "allow" } else { "deny" },
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "approvals_tests.rs"]
mod tests;
