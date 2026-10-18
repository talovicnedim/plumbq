//! What each tool call did, kept while it runs and written to the activity log afterwards.

use std::cell::RefCell;

use plumbq_core::permissions::Permission;
use rmcp::ErrorData;
use rmcp::model::CallToolResponse;
use tokio_util::sync::CancellationToken;

use super::PlumbqMcp;
use crate::activity::{self, ActivityEntry, ActivityOutcome};
use crate::settings::McpSettings;

tokio::task_local! {
    /// What the current tool call did, for its activity line.
    static CALL: RefCell<CallRecord>;
}

#[derive(Debug, Default)]
pub(super) struct CallRecord {
    pub actor: String,
    /// Response bodies are left out of this call's output (the assistant's privacy setting).
    pub hide_bodies: bool,
    pub permission: Option<Permission>,
    pub environment: Option<String>,
    pub outcome: Option<ActivityOutcome>,
    pub summary: Option<String>,
    /// Cancelled when the client cancels the call or goes away.
    pub cancel: CancellationToken,
    /// The stdio server's saved settings, read once when the call started.
    pub settings: Option<McpSettings>,
}

/// Runs `work` with `record` as the current call's record and returns both afterwards.
pub(super) async fn recorded<T>(
    record: CallRecord,
    work: impl Future<Output = T>,
) -> (T, CallRecord) {
    CALL.scope(RefCell::new(record), async {
        let result = work.await;
        let record = CALL.with(|r| r.take());
        (result, record)
    })
    .await
}

/// Updates the current call's activity record (a no-op outside a tool call, e.g. in tests).
pub(super) fn note(update: impl FnOnce(&mut CallRecord)) {
    let _ = CALL.try_with(|record| update(&mut record.borrow_mut()));
}

/// Whether the current call must leave response bodies out of its output.
pub(crate) fn bodies_hidden() -> bool {
    CALL.try_with(|record| record.borrow().hide_bodies)
        .unwrap_or(false)
}

/// The current call's cancellation; outside a call, one that is never cancelled.
pub(crate) fn cancellation() -> CancellationToken {
    CALL.try_with(|record| record.borrow().cancel.clone())
        .unwrap_or_default()
}

/// The settings the current call read when it started, if it read any.
pub(super) fn saved_settings() -> Option<McpSettings> {
    CALL.try_with(|record| record.borrow().settings.clone())
        .ok()
        .flatten()
}

pub(super) fn current_actor() -> String {
    CALL.try_with(|record| record.borrow().actor.clone())
        .ok()
        .filter(|a| !a.is_empty())
        .unwrap_or_else(|| "An AI agent".to_string())
}

impl PlumbqMcp {
    pub(super) fn log_activity(
        &self,
        tool: &str,
        record: CallRecord,
        result: &Result<CallToolResponse, ErrorData>,
    ) {
        let Some(dir) = &self.activity_dir else {
            return;
        };
        let error = error_of(result);
        let outcome = outcome(record.outcome, error.is_some());
        let summary = summary(tool, outcome, error, record.summary);
        let entry = ActivityEntry {
            at: plumbq_core::time::now_rfc3339(),
            actor: record.actor,
            from_chat: self.from_chat,
            workspace: self
                .root()
                .map(|r| r.display().to_string())
                .unwrap_or_default(),
            tool: tool.to_string(),
            environment: record.environment,
            permission: record.permission.unwrap_or(Permission::ReadWorkspace),
            outcome,
            summary,
        };
        if let Err(err) = activity::record(dir, entry) {
            tracing::warn!("failed to write the AI activity log: {err}");
        }
    }
}

fn error_of(result: &Result<CallToolResponse, ErrorData>) -> Option<String> {
    match result {
        Ok(CallToolResponse::Complete(done)) if done.is_error == Some(true) => Some(
            done.content
                .iter()
                .filter_map(|c| c.as_text().map(|t| t.text.clone()))
                .collect::<String>(),
        ),
        Ok(_) => None,
        Err(err) => Some(err.message.to_string()),
    }
}

/// A refusal stays a refusal even when the tool then returns an error.
fn outcome(noted: Option<ActivityOutcome>, failed: bool) -> ActivityOutcome {
    match (noted, failed) {
        (Some(refused @ (ActivityOutcome::Denied | ActivityOutcome::Blocked)), _) => refused,
        (_, true) => ActivityOutcome::Failed,
        (Some(outcome), false) => outcome,
        (None, false) => ActivityOutcome::Ran,
    }
}

fn summary(
    tool: &str,
    outcome: ActivityOutcome,
    error: Option<String>,
    action: Option<String>,
) -> String {
    match (error, action) {
        (Some(err), Some(action)) if outcome == ActivityOutcome::Failed => {
            format!("{action}: {err}")
        }
        (Some(err), None) => err,
        (_, Some(action)) => action,
        (None, None) => tool.replace('_', " "),
    }
}
