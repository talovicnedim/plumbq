//! Approval requests shown in the app: from the server here (a channel) and from the stdio
//! server (request files), in the order they reached the app.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use plumbq_core::approvals::{self, ApprovalRequest};
use plumbq_mcp::Approval;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, oneshot};

use super::McpState;
use crate::error::AppError;

/// Event telling the UI that the waiting approvals changed: one came in, was answered, or the
/// agent stopped waiting.
const APPROVAL: &str = "mcp-approval";
/// How often the stdio server's request files and dropped in-app requests are checked; files
/// and closed channels have no event of their own.
const CHECK_EVERY: Duration = Duration::from_secs(1);

/// The requests waiting for the user.
pub(super) struct Approvals {
    data_dir: PathBuf,
    queue: Mutex<Queue>,
}

/// Every request gets a number when it reaches the app, so a request file can't jump the
/// queue with an earlier timestamp of its own while the user is about to click.
#[derive(Default)]
struct Queue {
    next: u64,
    waiting: Vec<Waiting>,
    /// Request files by id, with the number they got when first seen.
    filed: HashMap<String, u64>,
}

struct Waiting {
    arrived: u64,
    request: ApprovalRequest,
    respond: oneshot::Sender<bool>,
}

impl Queue {
    fn number(&mut self) -> u64 {
        self.next += 1;
        self.next
    }
}

impl Approvals {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            queue: Mutex::default(),
        }
    }

    /// A request from the server in the app.
    pub fn add(&self, Approval { request, respond }: Approval) {
        let mut queue = self.lock();
        let arrived = queue.number();
        queue.waiting.push(Waiting {
            arrived,
            request,
            respond,
        });
    }

    /// Requests waiting for the user, first come first: from the server here and from the
    /// stdio server. Ones the agent stopped waiting for (timed out or disconnected) are dropped.
    pub fn pending(&self) -> Vec<ApprovalRequest> {
        let filed = approvals::pending(&self.data_dir);
        let mut queue = self.lock();
        queue.waiting.retain(|w| !w.respond.is_closed());
        let mut all: Vec<(u64, ApprovalRequest)> = queue
            .waiting
            .iter()
            .map(|w| (w.arrived, w.request.clone()))
            .collect();
        match filed {
            Ok(filed) => all.extend(number_files(&mut queue, filed)),
            Err(err) => tracing::warn!("failed to read MCP approval requests: {err}"),
        }
        all.sort_by_key(|(arrived, _)| *arrived);
        all.into_iter().map(|(_, request)| request).collect()
    }

    /// Answers a request. Approving a strong one needs its name in `typed`; without it the
    /// request keeps waiting. Unknown ids (already answered or timed out) are ignored.
    pub fn answer(&self, id: &str, approved: bool, typed: Option<&str>) -> Result<(), AppError> {
        let verdict = if approved { "approved" } else { "denied" };
        let mut queue = self.lock();
        if let Some(index) = queue.waiting.iter().position(|w| w.request.id == id) {
            if approved {
                approvals::check_typed(&queue.waiting[index].request, typed)?;
            }
            let waiting = queue.waiting.remove(index);
            tracing::info!(target: "audit", "{verdict} MCP request from {}: {}", waiting.request.actor, waiting.request.action);
            let _ = waiting.respond.send(approved);
            return Ok(());
        }
        drop(queue);
        match approvals::answer(&self.data_dir, id, approved, typed) {
            Ok(()) => {
                tracing::info!(target: "audit", "{verdict} MCP request {id} from the stdio server");
                Ok(())
            }
            Err(plumbq_core::Error::NotFound(_)) => {
                tracing::info!("MCP approval {id} was no longer waiting");
                Ok(())
            }
            Err(err) => Err(err.into()),
        }
    }

    /// A waiting request by id.
    pub fn find(&self, id: &str) -> Option<ApprovalRequest> {
        self.pending().into_iter().find(|r| r.id == id)
    }

    /// Denies everything waiting, from the server here and the stdio server.
    pub fn deny_all(&self) {
        let waiting = std::mem::take(&mut self.lock().waiting);
        let filed = approvals::pending(&self.data_dir).unwrap_or_default();
        if waiting.is_empty() && filed.is_empty() {
            return;
        }
        tracing::info!(target: "audit", "denied {} waiting MCP requests: AI is turned off", waiting.len() + filed.len());
        for waiting in waiting {
            let _ = waiting.respond.send(false);
        }
        for request in filed {
            if let Err(err) = approvals::answer(&self.data_dir, &request.id, false, None) {
                tracing::warn!("failed to deny MCP request {}: {err}", request.id);
            }
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Queue> {
        self.queue.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The request files with their arrival numbers; new files get the next ones, and files that
/// are gone are forgotten.
fn number_files(queue: &mut Queue, filed: Vec<ApprovalRequest>) -> Vec<(u64, ApprovalRequest)> {
    let ids: HashSet<&str> = filed.iter().map(|r| r.id.as_str()).collect();
    queue.filed.retain(|id, _| ids.contains(id.as_str()));
    filed
        .into_iter()
        .map(|request| {
            let arrived = match queue.filed.get(&request.id) {
                Some(arrived) => *arrived,
                None => {
                    let arrived = queue.number();
                    queue.filed.insert(request.id.clone(), arrived);
                    arrived
                }
            };
            (arrived, request)
        })
        .collect()
}

/// Shows each approval request in the app and brings the window forward, since the user is
/// probably looking at their AI client.
pub(super) async fn forward(
    app: AppHandle,
    state: Arc<McpState>,
    mut approvals: mpsc::Receiver<Approval>,
) {
    while let Some(approval) = approvals.recv().await {
        if state.ai_off() {
            let _ = approval.respond.send(false);
            continue;
        }
        state.approvals.add(approval);
        notify(&app);
    }
}

/// Shows new request files of the stdio server like the server's own, and tells the UI when
/// any waiting request goes away (answered, timed out, or the agent disconnected).
pub(super) async fn watch(app: AppHandle, state: Arc<McpState>) {
    let mut seen: HashSet<String> = HashSet::new();
    loop {
        tokio::time::sleep(CHECK_EVERY).await;
        let waiting: HashSet<String> = state.pending().into_iter().map(|r| r.id).collect();
        if waiting.iter().any(|id| !seen.contains(id)) {
            notify(&app);
        } else if waiting != seen {
            changed(&app);
        }
        seen = waiting;
    }
}

/// Tells the UI and brings the window forward, since the user is probably looking at their AI
/// client.
fn notify(app: &AppHandle) {
    changed(app);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.set_focus();
        let _ = window.request_user_attention(Some(tauri::UserAttentionType::Critical));
    }
}

fn changed(app: &AppHandle) {
    if let Err(err) = app.emit(APPROVAL, ()) {
        tracing::warn!("failed to show MCP approval: {err}");
    }
}

#[cfg(test)]
#[path = "approvals_tests.rs"]
mod tests;
