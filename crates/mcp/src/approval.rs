//! Asking the human before an agent changes anything. Inside the app the server sends an
//! [`Approval`] over a channel and the app shows a dialog; the stdio server files a request the
//! running app answers (see `plumbq_core::approvals`). Either way no answer means denied.

use std::path::PathBuf;
use std::time::Duration;

use plumbq_core::approvals::{self, ApprovalRequest};
use tokio::sync::{mpsc, oneshot};

use crate::apps;

/// An approval request plus the channel for its answer (`true` = approved).
#[derive(Debug)]
pub struct Approval {
    pub request: ApprovalRequest,
    pub respond: oneshot::Sender<bool>,
}

/// Sends approval requests to the user.
#[derive(Debug, Clone)]
pub enum Approver {
    /// Inside the app: requests go to it over a channel.
    Channel {
        tx: mpsc::Sender<Approval>,
        timeout: Duration,
    },
    /// A separate process: request files in the app data dir, answered by the running app.
    Files {
        data_dir: PathBuf,
        timeout: Duration,
    },
    /// No one to ask: anything that needs approval is denied.
    Nobody,
}

impl Approver {
    /// The approver and the receiving end the host app listens on.
    pub fn channel() -> (Self, mpsc::Receiver<Approval>) {
        let (tx, rx) = mpsc::channel(16);
        (
            Self::Channel {
                tx,
                timeout: timeout(),
            },
            rx,
        )
    }

    pub fn files(data_dir: PathBuf) -> Self {
        Self::Files {
            data_dir,
            timeout: timeout(),
        }
    }

    /// Waits for the user's answer. `Err` explains a denial in words the agent can relay.
    pub async fn ask(&self, mut request: ApprovalRequest) -> Result<(), String> {
        request.actor_label = apps::client_label(&request.actor);
        let action = request.action.clone();
        tracing::info!(target: "mcp", "{} asks to {action}", request.actor);
        let (answer, waited) = match self {
            Self::Nobody => {
                return Err(format!(
                    "this needs the user's approval in the Plumbq app, which isn't available \
                     here: {action}"
                ));
            }
            Self::Channel { tx, timeout } => {
                approvals::stamp(&mut request);
                let (respond, answer) = oneshot::channel();
                tx.send(Approval { request, respond })
                    .await
                    .map_err(|_| "the app can't show approvals right now".to_string())?;
                let answer = match tokio::time::timeout(*timeout, answer).await {
                    Ok(Ok(approved)) => Some(approved),
                    Ok(Err(_)) => Some(false),
                    Err(_) => None,
                };
                (answer, *timeout)
            }
            Self::Files { data_dir, timeout } => {
                let started = tokio::time::Instant::now();
                let approved = approvals::request_and_wait(data_dir, request, *timeout)
                    .await
                    .map_err(|err| format!("couldn't ask the user: {err}"))?;
                // Without an answer file, `request_and_wait` reports false after the timeout.
                let answer = (approved || started.elapsed() < *timeout).then_some(approved);
                (answer, *timeout)
            }
        };
        match answer {
            Some(true) => Ok(()),
            Some(false) => Err(format!(
                "the user denied this in the Plumbq app: {action}; don't retry unless they \
                 ask you to"
            )),
            None => Err(format!(
                "nobody approved this in the Plumbq app within {}s (is the app open?): \
                 {action}",
                waited.as_secs()
            )),
        }
    }
}

/// How long to wait for the user; `PLUMBQ_APPROVAL_TIMEOUT_SECS` overrides the default.
fn timeout() -> Duration {
    std::env::var("PLUMBQ_APPROVAL_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(Duration::from_secs)
        .unwrap_or(approvals::DEFAULT_TIMEOUT)
}

#[cfg(test)]
mod tests {
    use plumbq_core::permissions::Permission;

    use super::*;

    fn request() -> ApprovalRequest {
        ApprovalRequest {
            id: String::new(),
            created_at: String::new(),
            actor: "claude-code".into(),
            actor_label: String::new(),
            workspace_name: String::new(),
            workspace: "/ws".into(),
            tool: "requests_update".into(),
            permission: Permission::EditFiles,
            action: "update users/list.http".into(),
            environment: None,
            production: false,
            strong: false,
            confirm_name: None,
            always_allow: None,
            detail: String::new(),
            lines: Vec::new(),
            warnings: Vec::new(),
            reason: None,
        }
    }

    #[tokio::test]
    async fn channel_answers_approve_and_deny() {
        let (approver, mut rx) = Approver::channel();
        let host = tokio::spawn(async move {
            let first = rx.recv().await.unwrap();
            assert!(!first.request.id.is_empty(), "requests get an id");
            assert_eq!(first.request.actor_label, "Claude Code");
            first.respond.send(true).unwrap();
            rx.recv().await.unwrap().respond.send(false).unwrap();
        });
        assert!(approver.ask(request()).await.is_ok());
        let denied = approver.ask(request()).await.unwrap_err();
        assert!(denied.contains("denied"), "{denied}");
        host.await.unwrap();
    }

    #[tokio::test]
    async fn nobody_to_ask_means_denied() {
        let err = Approver::Nobody.ask(request()).await.unwrap_err();
        assert!(err.contains("approval"), "{err}");
    }

    #[tokio::test]
    async fn unanswered_file_requests_time_out_as_denied() {
        let dir = std::env::temp_dir().join(format!(
            "ac-test-approval-{}",
            &crate::settings::new_token()[..12]
        ));
        let approver = Approver::Files {
            data_dir: dir.clone(),
            timeout: Duration::from_millis(300),
        };
        let err = approver.ask(request()).await.unwrap_err();
        assert!(err.contains("within"), "{err}");
        assert!(approvals::pending(&dir).unwrap().is_empty());
    }
}
