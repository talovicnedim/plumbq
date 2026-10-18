//! Code decides, never the model: every change goes through `authorize`, which runs it, asks
//! the user, or refuses.

use plumbq_core::approvals::ApprovalRequest;
use plumbq_core::permissions::{self, Permission, PermissionContext, PermissionDecision};
use plumbq_core::text_diff;

use super::PlumbqMcp;
use super::approval_text::{escaped, escaped_lines, shown, shown_name};
use super::call_log::{cancellation, current_actor, note};
use crate::activity::ActivityOutcome;
use crate::settings::is_hidden;

/// One call that needs a decision: what it is and what exactly it would do.
pub(crate) struct Call {
    pub tool: &'static str,
    pub permission: Permission,
    pub context: PermissionContext,
    /// Readable action, e.g. `send DELETE {{baseUrl}}/users/7`.
    pub action: String,
    /// The request as it would be sent, or a diff of the file change.
    pub detail: String,
    pub environment: Option<String>,
    pub reason: Option<String>,
}

impl PlumbqMcp {
    /// Refuses environments the user hid from the AI, as if they didn't exist.
    pub fn check_visible(&self, environment: Option<&str>) -> Result<(), String> {
        match environment {
            Some(name) if is_hidden(&self.hidden_environments(), name) => {
                note(|r| r.outcome = Some(ActivityOutcome::Blocked));
                Err(format!(
                    "environment {name} is hidden from AI in the app; don't use it"
                ))
            }
            _ => Ok(()),
        }
    }

    /// Decides a call: runs it, asks the user and waits, or refuses with a reason the agent can
    /// relay. Records the outcome for the activity log.
    pub(crate) async fn authorize(&self, call: Call) -> Result<(), String> {
        note(|r| {
            r.permission = Some(call.permission);
            r.environment = call.environment.clone();
            r.summary = Some(call.action.clone());
        });
        self.check_visible(call.environment.as_deref())?;
        match permissions::decide(call.permission, &call.context, &self.permissions()) {
            PermissionDecision::Allow => {
                note(|r| r.outcome = Some(ActivityOutcome::Ran));
                Ok(())
            }
            PermissionDecision::Deny { reason } => {
                note(|r| r.outcome = Some(ActivityOutcome::Blocked));
                Err(format!(
                    "refused: {reason}; don't retry unless the user asks you to"
                ))
            }
            PermissionDecision::Ask { strong } => self.ask(call, strong).await,
        }
    }

    /// A call the client cancels while waiting is denied, and the app's question goes away.
    async fn ask(&self, call: Call, strong: bool) -> Result<(), String> {
        let request = self.approval_request(call, strong)?;
        let cancel = cancellation();
        let answer = tokio::select! {
            answer = self.approver.ask(request) => answer,
            () = cancel.cancelled() => Err("the call was cancelled before the user answered".into()),
        };
        note(|r| {
            r.outcome = Some(match answer {
                Ok(()) => ActivityOutcome::Approved,
                Err(_) => ActivityOutcome::Denied,
            })
        });
        answer
    }

    fn approval_request(&self, call: Call, strong: bool) -> Result<ApprovalRequest, String> {
        let warnings = [
            &call.context.secrets_to_new_host,
            &call.context.repoints_url,
            &call.context.unexpected_host,
        ]
        .into_iter()
        .flatten()
        .map(|warning| escaped(warning))
        .collect();
        let detail = escaped_lines(&call.detail);
        let lines = text_diff::lines(&detail);
        Ok(ApprovalRequest {
            id: String::new(),
            created_at: String::new(),
            actor: shown_name(&current_actor()),
            actor_label: String::new(),
            workspace_name: String::new(),
            workspace: self.root()?.display().to_string(),
            tool: call.tool.to_string(),
            permission: call.permission,
            action: escaped(&call.action),
            environment: call.environment,
            production: call.context.production,
            strong,
            confirm_name: None,
            always_allow: None,
            detail,
            lines,
            warnings,
            reason: call.reason.as_deref().map(shown),
        })
    }
}
