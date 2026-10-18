use std::collections::BTreeMap;
use std::path::Path;

use plumbq_core::permissions::{self, PermissionDecision};
use plumbq_core::send::SentRequest;
use plumbq_core::{
    Error, blocking, chain, curl, environments, guard, history, requests, send, variables,
};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{schemars, tool, tool_router};
use serde::Deserialize;

use crate::server::{Call, PlumbqMcp, bodies_hidden, error_text};
use crate::settings::is_hidden;
use crate::{format, outline};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SendParams {
    /// Request id from workspace_tree.
    pub id: String,
    /// Environment name from environments_list. Omit to send without variables.
    pub environment: Option<String>,
    /// Answers for `{{$prompt name}}` variables in the request, by name. Ask the user for
    /// them; never guess.
    #[serde(default)]
    pub prompts: BTreeMap<String, String>,
    /// One short sentence for the user on why you want to send it; shown when they're asked
    /// to approve it.
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PreviewParams {
    /// Request id from workspace_tree.
    pub id: String,
    /// Environment name from environments_list. Omit for no environment.
    pub environment: Option<String>,
}

#[tool_router(router = send_router, vis = "pub(crate)")]
impl PlumbqMcp {
    /// Sends a saved request and returns status, timing, headers and the body (long bodies are
    /// cut). Secret values appear as `{{name}}`. Requests that change data, go to production, or
    /// would send secrets to a new host wait for the user's approval in the app.
    #[tool(annotations(title = "Send request", read_only_hint = false, open_world_hint = true))]
    async fn requests_send(&self, Parameters(p): Parameters<SendParams>) -> Result<String, String> {
        let root = self.root()?;
        let env = p.environment.as_deref();
        self.check_environment(&root, env)?;
        let request = requests::get(&root, &p.id).map_err(error_text)?;
        let assessment =
            guard::assess_agent_send(&root, &p.id, &request, env).map_err(error_text)?;
        self.authorize(Call {
            tool: "requests_send",
            permission: assessment.permission(),
            context: assessment.context(),
            action: send_action(
                &request,
                &p.id,
                env,
                &assessment,
                &self.hidden_environments(),
            ),
            detail: preview_text(&root, &p.id, &request, env).await,
            environment: p.environment.clone(),
            reason: p.reason,
        })
        .await?;
        let confirmed = guard::check_approved(&root, &p.id, &request, env, Some(&assessment))
            .map_err(error_text)?;
        let response = send::send(
            &root,
            &p.id,
            &request,
            env,
            &p.prompts,
            &self.send_context(&root, env),
            confirmed,
        )
        .await
        .map_err(error_text)?;
        Ok(self.sent_text(&root, &p.id, env, &request, response))
    }

    /// What a request would send, without sending it: the final method, URL, headers and body
    /// after variables and folder headers (secrets stay `{{name}}`), plus values it would ask
    /// for and whether this server is allowed to send it. Use it to check a request you wrote.
    #[tool(annotations(title = "Preview request", read_only_hint = true))]
    async fn requests_preview(
        &self,
        Parameters(p): Parameters<PreviewParams>,
    ) -> Result<String, String> {
        let root = self.root()?;
        let env = p.environment.as_deref();
        self.check_environment(&root, env)?;
        let request = requests::get(&root, &p.id).map_err(error_text)?;
        let prepared = match prepare(&root, &p.id, &request, env).await {
            Ok(prepared) => Ok(prepared),
            Err(Error::Invalid(problem)) => Err(problem),
            Err(err) => return Err(error_text(err)),
        };
        let prompts = send::prompts_needed(&root, &p.id, &request).map_err(error_text)?;
        let assessment =
            guard::assess_agent_send(&root, &p.id, &request, env).map_err(error_text)?;
        let verdict = self.verdict(&assessment);
        Ok(outline::preview(&prepared, &prompts, &verdict))
    }
}

impl PlumbqMcp {
    /// A clear error for a misspelled or hidden environment, before anything is sent.
    pub fn check_environment(&self, root: &Path, environment: Option<&str>) -> Result<(), String> {
        let Some(name) = environment else {
            return Ok(());
        };
        self.check_visible(Some(name))?;
        if environments::exists(root, name) {
            return Ok(());
        }
        let hidden = self.hidden_environments();
        let mut known = environments::names(root).map_err(error_text)?;
        known.retain(|n| !is_hidden(&hidden, n));
        Err(format!(
            "unknown environment {name}{}; environments: {}",
            environments::unknown_hint(name, &known),
            if known.is_empty() {
                "none yet".to_string()
            } else {
                known.join(", ")
            }
        ))
    }
}

impl PlumbqMcp {
    /// Records the response in history and words it for the agent.
    fn sent_text(
        &self,
        root: &Path,
        id: &str,
        env: Option<&str>,
        request: &plumbq_core::http_file::Request,
        mut response: send::Response,
    ) -> String {
        if let Some(dir) = &self.history_dir
            && let Err(err) = history::add(dir, root, id, env, response.clone())
        {
            tracing::warn!("failed to record history: {err}");
        }
        // Values chained from earlier responses are body content too.
        if bodies_hidden()
            && variables::used(&request.url)
                .iter()
                .any(|n| chain::is_reference(n))
        {
            response.sent.url = request.url.clone();
        }
        format::response(&response)
    }

    /// What [`Self::authorize`] would do with a send, in words for `requests_preview`.
    fn verdict(&self, assessment: &guard::SendAssessment) -> Result<(), String> {
        let decision = permissions::decide(
            assessment.permission(),
            &assessment.context(),
            &self.permissions(),
        );
        match decision {
            PermissionDecision::Allow => Ok(()),
            PermissionDecision::Ask { .. } => Err(format!(
                "sending it waits for the user's approval in the app{}",
                assessment
                    .secrets_to_new_host
                    .as_ref()
                    .map(|why| format!(" ({why})"))
                    .unwrap_or_default()
            )),
            PermissionDecision::Deny { reason } => {
                Err(format!("this server won't send it: {reason}"))
            }
        }
    }
}

/// `send DELETE {{baseUrl}}/users/7 (users/remove.http) with environment prod`. The agent sees
/// it when the user denies, so a hidden environment is never named.
fn send_action(
    request: &plumbq_core::http_file::Request,
    id: &str,
    environment: Option<&str>,
    assessment: &guard::SendAssessment,
    hidden: &[String],
) -> String {
    let env = environment
        .map(|e| format!(" with environment {e}"))
        .unwrap_or_default();
    let production = match assessment.production.as_deref() {
        Some(prod) if Some(prod) == environment => String::new(),
        Some(prod) if is_hidden(hidden, prod) => " (the host of a production environment)".into(),
        Some(prod) => format!(" (the host of production environment {prod})"),
        None => String::new(),
    };
    format!(
        "send {} {} ({id}){env}{production}",
        assessment.method, request.url
    )
}

/// The request as it would go out, secrets as `{{name}}`, for the approval dialog.
async fn preview_text(
    root: &Path,
    id: &str,
    request: &plumbq_core::http_file::Request,
    environment: Option<&str>,
) -> String {
    match prepare(root, id, request, environment).await {
        Ok(sent) => {
            let mut text = format!("{} {}\n", sent.method, sent.url);
            for header in &sent.headers {
                text.push_str(&format!("{}: {}\n", header.name, header.value));
            }
            if !sent.body.is_empty() {
                text.push('\n');
                text.push_str(&format::cap(&sent.body));
            }
            text
        }
        Err(err) => format!(
            "{} {}\n(can't resolve it yet: {err})",
            request.method, request.url
        ),
    }
}

/// Resolving the environment reads secrets from the keychain, which blocks.
async fn prepare(
    root: &Path,
    id: &str,
    request: &plumbq_core::http_file::Request,
    environment: Option<&str>,
) -> plumbq_core::Result<SentRequest> {
    let (root, id, request) = (root.to_path_buf(), id.to_string(), request.clone());
    let environment = environment.map(str::to_string);
    blocking::run(move || curl::prepare_for_export(&root, &id, &request, environment.as_deref()))
        .await
}
