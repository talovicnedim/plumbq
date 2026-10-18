use plumbq_core::cookies;
use plumbq_core::permissions::{Permission, PermissionContext};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{schemars, tool, tool_router};
use serde::Deserialize;

use crate::format;
use crate::server::{Call, PlumbqMcp, error_text};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct JarParams {
    /// Environment name from environments_list; omit for the "no environment" jar.
    pub environment: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ClearParams {
    /// Environment name from environments_list; omit for the "no environment" jar.
    pub environment: Option<String>,
    /// One short sentence for the user on why you want this; shown when they're asked to
    /// approve it.
    pub reason: Option<String>,
}

#[tool_router(router = cookies_router, vis = "pub(crate)")]
impl PlumbqMcp {
    /// Cookies stored from earlier responses for an environment (name, domain, path, expiry;
    /// values are never shown). They are sent automatically with matching requests.
    #[tool(annotations(title = "List cookies", read_only_hint = true))]
    async fn cookies_list(&self, Parameters(p): Parameters<JarParams>) -> Result<String, String> {
        let root = self.root()?;
        let Some(dir) = &self.cookie_dir else {
            return Ok("Cookies are unavailable on this machine.".into());
        };
        self.check_visible(p.environment.as_deref())?;
        let jar = cookies::list(dir, &root, p.environment.as_deref()).map_err(error_text)?;
        Ok(format::cookies(&jar))
    }

    /// Deletes every stored cookie of an environment, e.g. to log out or start a fresh session.
    /// Returns a short confirmation.
    #[tool(annotations(
        title = "Clear cookies",
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = true
    ))]
    async fn cookies_clear(
        &self,
        Parameters(p): Parameters<ClearParams>,
    ) -> Result<String, String> {
        let root = self.root()?;
        let Some(dir) = &self.cookie_dir else {
            return Ok("Cookies are unavailable on this machine.".into());
        };
        self.authorize(Call {
            tool: "cookies_clear",
            permission: Permission::Destructive,
            context: PermissionContext::default(),
            action: format!(
                "delete the cookies of {}",
                p.environment
                    .as_deref()
                    .map_or("\"no environment\"".to_string(), |e| format!(
                        "environment {e}"
                    ))
            ),
            detail: String::new(),
            environment: p.environment.clone(),
            reason: p.reason.clone(),
        })
        .await?;
        let _lock = self.write_lock().await;
        cookies::clear(dir, &root, p.environment.as_deref()).map_err(error_text)?;
        Ok("Cookies cleared".into())
    }
}
