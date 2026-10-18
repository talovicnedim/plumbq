use plumbq_core::environments::{self, EnvironmentTag};
use plumbq_core::permissions::{Permission, PermissionContext};
use plumbq_core::{blocking, folders, guard, text_diff};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{schemars, tool, tool_router};
use serde::Deserialize;

use crate::format;
use crate::server::{Call, PlumbqMcp, error_text};
use crate::settings::is_hidden;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateParams {
    /// Environment name, e.g. `staging`.
    pub name: String,
    /// One of `local`, `development`, `staging`, `production`. Production makes writes need
    /// the user's confirmation in the app.
    pub tag: String,
    /// One short sentence for the user on why you want this; shown when they're asked to
    /// approve it.
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct VariableParams {
    /// Environment name from environments_list.
    pub environment: String,
    /// Variable name, used as `{{name}}` in requests.
    pub name: String,
    /// Plain value. Never put tokens or passwords here; the user sets secrets in the app.
    pub value: String,
    /// One short sentence for the user on why you want this; shown when they're asked to
    /// approve it.
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveVariableParams {
    /// Environment name from environments_list.
    pub environment: String,
    /// Variable name to remove.
    pub name: String,
    /// One short sentence for the user on why you want this; shown when they're asked to
    /// approve it.
    pub reason: Option<String>,
}

#[tool_router(router = environments_router, vis = "pub(crate)")]
impl PlumbqMcp {
    /// Environments with their tag, plain variables and secret names (secret values are never
    /// shown).
    #[tool(annotations(title = "List environments", read_only_hint = true))]
    async fn environments_list(&self) -> Result<String, String> {
        let root = self.root()?;
        let hidden = self.hidden_environments();
        let mut list = {
            let root = root.clone();
            blocking::run(move || environments::list(&root)).await
        }
        .map_err(error_text)?;
        list.retain(|env| !is_hidden(&hidden, &env.name));
        let defaults = folders::get_settings(&root, "")
            .map_err(error_text)?
            .variables;
        let mut out = String::new();
        if !defaults.is_empty() {
            out.push_str("Workspace defaults (root folder.toml; environments override them)\n");
            for (name, value) in &defaults {
                out.push_str(&format!("  {name} = {value}\n"));
            }
        }
        out.push_str(&format::environments(&list));
        Ok(out)
    }

    /// Creates an empty environment (a TOML file in `environments/`). Returns its name.
    #[tool(annotations(
        title = "Create environment",
        read_only_hint = false,
        destructive_hint = false
    ))]
    async fn environments_create(
        &self,
        Parameters(p): Parameters<CreateParams>,
    ) -> Result<String, String> {
        let root = self.root()?;
        let tag = parse_tag(&p.tag)?;
        self.authorize(Call {
            tool: "environments_create",
            permission: Permission::ChangeEnvironment,
            context: PermissionContext::default(),
            action: format!("create environment {} ({})", p.name, p.tag.to_lowercase()),
            detail: String::new(),
            environment: Some(p.name.clone()),
            reason: p.reason.clone(),
        })
        .await?;
        let _lock = self.write_lock().await;
        let name = environments::create(&root, &p.name, tag).map_err(error_text)?;
        Ok(format!("Created environment {name}"))
    }

    /// Sets a plain variable, e.g. `baseUrl`, overwriting its value; secrets can't be set here
    /// (the user adds them in the app). Moving a URL to another host, or pointing a
    /// non-production environment at a production host, needs the user's explicit approval.
    /// Returns a short confirmation.
    #[tool(annotations(
        title = "Set variable",
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = true
    ))]
    async fn environments_set_variable(
        &self,
        Parameters(p): Parameters<VariableParams>,
    ) -> Result<String, String> {
        let root = self.root()?;
        let repoints = guard::assess_agent_variable(&root, &p.environment, &p.name, &p.value)
            .map_err(error_text)?;
        let old = environments::plain_variables(&root, Some(&p.environment))
            .map_err(error_text)?
            .get(&p.name)
            .cloned();
        self.authorize(Call {
            tool: "environments_set_variable",
            permission: Permission::ChangeEnvironment,
            context: PermissionContext {
                production: environments::tag(&root, &p.environment).map_err(error_text)?
                    == EnvironmentTag::Production,
                repoints_url: repoints,
                ..PermissionContext::default()
            },
            action: format!("set {} in environment {}", p.name, p.environment),
            detail: text_diff::unified(
                &format!("environments/{}.toml", p.environment),
                &old.map(|v| format!("{} = {v}\n", p.name))
                    .unwrap_or_default(),
                &format!("{} = {}\n", p.name, p.value),
            ),
            environment: Some(p.environment.clone()),
            reason: p.reason.clone(),
        })
        .await?;
        let _lock = self.write_lock().await;
        environments::set_variable(&root, &p.environment, &p.name, &p.value).map_err(error_text)?;
        Ok(format!("Set {} in {}", p.name, p.environment))
    }

    /// Removes a plain variable from an environment. Returns a short confirmation.
    #[tool(annotations(
        title = "Remove variable",
        read_only_hint = false,
        destructive_hint = true
    ))]
    async fn environments_remove_variable(
        &self,
        Parameters(p): Parameters<RemoveVariableParams>,
    ) -> Result<String, String> {
        let root = self.root()?;
        self.authorize(Call {
            tool: "environments_remove_variable",
            permission: Permission::Destructive,
            context: PermissionContext::default(),
            action: format!("remove {} from environment {}", p.name, p.environment),
            detail: String::new(),
            environment: Some(p.environment.clone()),
            reason: p.reason.clone(),
        })
        .await?;
        let _lock = self.write_lock().await;
        environments::remove_variable(&root, &p.environment, &p.name).map_err(error_text)?;
        Ok(format!("Removed {} from {}", p.name, p.environment))
    }
}

fn parse_tag(tag: &str) -> Result<EnvironmentTag, String> {
    match tag.to_lowercase().as_str() {
        "production" => Ok(EnvironmentTag::Production),
        "staging" => Ok(EnvironmentTag::Staging),
        "development" => Ok(EnvironmentTag::Development),
        "local" => Ok(EnvironmentTag::Local),
        other => Err(format!(
            "unknown tag {other}; use local, development, staging or production"
        )),
    }
}
