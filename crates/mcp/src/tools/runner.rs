use std::collections::BTreeMap;
use std::path::Path;

use plumbq_core::guard::{self, SendAssessment};
use plumbq_core::permissions::{Permission, PermissionContext};
use plumbq_core::requests;
use plumbq_core::runner::{self, RunOptions};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{schemars, tool, tool_router};
use serde::Deserialize;

use crate::format;
use crate::server::{Call, PlumbqMcp, bodies_hidden, cancellation, error_text};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RunParams {
    /// Folder path from workspace_tree, a request id, or an empty string for the whole
    /// workspace.
    pub folder: String,
    /// Environment name from environments_list. Omit to run without variables.
    pub environment: Option<String>,
    /// Only requests with one of these tags (`# @tag smoke`). Empty runs everything.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Stop at the first failing request.
    #[serde(default)]
    pub bail: bool,
    /// Answers for `{{$prompt name}}` variables, by name. Ask the user; never guess.
    #[serde(default)]
    pub prompts: BTreeMap<String, String>,
    /// One short sentence for the user on why you want to run it; shown when they're asked to
    /// approve it.
    pub reason: Option<String>,
}

#[tool_router(router = runner_router, vis = "pub(crate)")]
impl PlumbqMcp {
    /// Sends every request in a folder (or the whole workspace) in sidebar order and checks
    /// their assertions, like a test run. Returns one line per request and a pass/fail
    /// summary. When the run includes requests that change data, go to production or send
    /// secrets to a new host, it waits for the user's approval in the app first.
    #[tool(annotations(title = "Run folder", read_only_hint = false, open_world_hint = true))]
    async fn folders_run(&self, Parameters(p): Parameters<RunParams>) -> Result<String, String> {
        let root = self.root()?;
        let env = p.environment.as_deref();
        self.check_environment(&root, env)?;
        let selection = runner::select(&root, &p.folder, &p.tags).map_err(error_text)?;
        if selection.is_empty() {
            return Err(format!("no requests to run in {}", display(&p.folder)));
        }
        let plan = plan(&root, &selection, env)?;
        self.authorize(Call {
            tool: "folders_run",
            permission: plan.permission,
            context: plan.context.clone(),
            action: format!(
                "run {} ({} requests, {} that change data){}",
                display(&p.folder),
                selection.len(),
                plan.writes,
                env.map(|e| format!(" with environment {e}"))
                    .unwrap_or_default()
            ),
            detail: plan.lines.join("\n"),
            environment: p.environment.clone(),
            reason: p.reason.clone(),
        })
        .await?;
        let options = RunOptions {
            environment: p.environment.clone(),
            bail: p.bail,
            prompts: p.prompts,
            tags: p.tags,
            safe_methods_only: self.is_read_only(),
            context: self.send_context(&root, env),
            agent: Some(plan.approved),
            ..Default::default()
        };
        let cancel = cancellation();
        let mut report = runner::run(&root, &selection, &options, |_| !cancel.is_cancelled()).await;
        if bodies_hidden() {
            for assertion in report.outcomes.iter_mut().flat_map(|o| &mut o.assertions) {
                assertion.message = format::VALUES_HIDDEN.to_string();
            }
        }
        Ok(format::cap_with(
            &runner::pretty(&report),
            "run a smaller folder or filter by tag to see the rest",
        ))
    }
}

/// What a run would do, for one decision covering all of it.
struct RunPlan {
    permission: Permission,
    context: PermissionContext,
    writes: usize,
    /// `POST {{baseUrl}}/users  users/create.http  [production]`, one per request.
    lines: Vec<String>,
    /// Each request as the user sees it here, so one that changes later isn't sent.
    approved: BTreeMap<String, SendAssessment>,
}

fn plan(root: &Path, selection: &[runner::Selected], env: Option<&str>) -> Result<RunPlan, String> {
    let mut plan = RunPlan {
        permission: Permission::SendSafe,
        context: PermissionContext::default(),
        writes: 0,
        lines: Vec::new(),
        approved: BTreeMap::new(),
    };
    for item in selection {
        let request = requests::get(root, &item.id).map_err(error_text)?;
        let assessment =
            guard::assess_agent_send(root, &item.id, &request, env).map_err(error_text)?;
        let mut flags = Vec::new();
        if !assessment.safe {
            plan.writes += 1;
            plan.permission = Permission::SendWrite;
        }
        if let Some(prod) = &assessment.production {
            flags.push(format!("production {prod}"));
            plan.context.production |= !assessment.safe;
        }
        if let Some(why) = &assessment.secrets_to_new_host {
            flags.push("secrets to a new host".to_string());
            plan.context.secrets_to_new_host = Some(why.clone());
        }
        if assessment.process_env.is_some() {
            plan.context.process_env = true;
        }
        if assessment.forbidden.is_some() {
            flags.push("never sent by AI".to_string());
        }
        let flags = match flags.is_empty() {
            true => String::new(),
            false => format!("  [{}]", flags.join(", ")),
        };
        plan.lines.push(format!(
            "{} {}  {}{flags}",
            assessment.method, request.url, item.id
        ));
        plan.approved.insert(item.id.clone(), assessment);
    }
    Ok(plan)
}

fn display(folder: &str) -> &str {
    if folder.is_empty() {
        "the workspace"
    } else {
        folder
    }
}
