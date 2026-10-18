//! What an AI agent (over MCP) may do, decided in code. Every tool call gets a [`Permission`]
//! from the tool itself; [`decide`] turns it plus what the call touches ([`PermissionContext`])
//! into allow / ask the user / deny under the user's [`AgentPermissions`]. The agent never
//! classifies its own requests.

use serde::{Deserialize, Serialize};

/// Ordered from harmless to dangerous.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub enum Permission {
    /// The tree, requests, search, previews, history, examples, environments without secrets.
    ReadWorkspace,
    /// Sending a GET, HEAD or OPTIONS request.
    SendSafe,
    /// Sending anything else (POST, PUT, PATCH, DELETE, custom methods).
    SendWrite,
    /// Creating, editing, moving and importing requests and folders.
    EditFiles,
    /// Creating environments and setting their variables.
    ChangeEnvironment,
    /// Deleting requests, folders, variables or cookies.
    Destructive,
}

/// What the user chose for one permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum PermissionRule {
    Allow,
    Ask,
    Deny,
}

/// The user's choices for AI agents. Saved with the MCP settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentPermissions {
    pub send_safe: PermissionRule,
    pub send_write: PermissionRule,
    pub edit_files: PermissionRule,
    pub change_environment: PermissionRule,
    pub destructive: PermissionRule,
    /// Resolve `{{$processEnv NAME}}` / `{{$env.NAME}}`, which read the server's environment.
    pub allow_process_env: bool,
}

impl Default for AgentPermissions {
    fn default() -> Self {
        Self {
            send_safe: PermissionRule::Allow,
            send_write: PermissionRule::Ask,
            edit_files: PermissionRule::Ask,
            change_environment: PermissionRule::Ask,
            destructive: PermissionRule::Ask,
            allow_process_env: false,
        }
    }
}

impl AgentPermissions {
    /// Reads and safe sends only; everything else is refused without asking.
    pub fn read_only() -> Self {
        Self {
            send_safe: PermissionRule::Allow,
            send_write: PermissionRule::Deny,
            edit_files: PermissionRule::Deny,
            change_environment: PermissionRule::Deny,
            destructive: PermissionRule::Deny,
            allow_process_env: false,
        }
    }

    /// Changes the rule for `permission`; reads have none and always run.
    pub fn set_rule(&mut self, permission: Permission, rule: PermissionRule) {
        let field = match permission {
            Permission::ReadWorkspace => return,
            Permission::SendSafe => &mut self.send_safe,
            Permission::SendWrite => &mut self.send_write,
            Permission::EditFiles => &mut self.edit_files,
            Permission::ChangeEnvironment => &mut self.change_environment,
            Permission::Destructive => &mut self.destructive,
        };
        *field = rule;
    }

    pub fn rule(&self, permission: Permission) -> PermissionRule {
        match permission {
            Permission::ReadWorkspace => PermissionRule::Allow,
            Permission::SendSafe => self.send_safe,
            Permission::SendWrite => self.send_write,
            Permission::EditFiles => self.edit_files,
            Permission::ChangeEnvironment => self.change_environment,
            Permission::Destructive => self.destructive,
        }
    }
}

/// What a call touches, beyond its permission.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PermissionContext {
    /// The request goes to a production environment or production host.
    pub production: bool,
    /// Why secrets would go somewhere new (a host that isn't a URL of the environment), if so.
    pub secrets_to_new_host: Option<String>,
    /// Why a URL variable would point somewhere new (another host, or a production host), if so.
    pub repoints_url: Option<String>,
    /// Why the call reaches a host the user may not expect, if so: an agent's download from
    /// this machine, the local network, production or a host no environment points at.
    pub unexpected_host: Option<String>,
    /// The request reads the server's process environment.
    pub process_env: bool,
    /// Why agents may never do this, if so (e.g. the request sends an environment file).
    pub forbidden: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum PermissionDecision {
    Allow,
    /// The user must approve first; `strong` asks them to type the environment or workspace name.
    Ask {
        strong: bool,
    },
    Deny {
        reason: String,
    },
}

pub fn decide(
    permission: Permission,
    context: &PermissionContext,
    policy: &AgentPermissions,
) -> PermissionDecision {
    if let Some(reason) = &context.forbidden {
        return PermissionDecision::Deny {
            reason: reason.clone(),
        };
    }
    if context.process_env && !policy.allow_process_env {
        return PermissionDecision::Deny {
            reason: "the request reads the MCP server's process environment ({{$processEnv}} / \
                     {{$env.}}), which is off in the app's MCP settings"
                .into(),
        };
    }
    let base = match policy.rule(permission) {
        PermissionRule::Deny => {
            return PermissionDecision::Deny {
                reason: format!(
                    "{} is turned off for AI agents in the app's MCP settings",
                    describe(permission)
                ),
            };
        }
        PermissionRule::Allow => PermissionDecision::Allow,
        PermissionRule::Ask => PermissionDecision::Ask { strong: false },
    };
    // Rules can't switch these off: they need a person every time.
    let writes = !matches!(permission, Permission::ReadWorkspace | Permission::SendSafe);
    let strong = (writes && context.production)
        || permission == Permission::Destructive
        || context.repoints_url.is_some();
    let risky =
        strong || context.secrets_to_new_host.is_some() || context.unexpected_host.is_some();
    match base {
        PermissionDecision::Ask { .. } => PermissionDecision::Ask { strong },
        PermissionDecision::Allow if risky => PermissionDecision::Ask { strong },
        other => other,
    }
}

/// Readable name, e.g. for the settings page and denial messages.
pub fn describe(permission: Permission) -> &'static str {
    match permission {
        Permission::ReadWorkspace => "reading the workspace",
        Permission::SendSafe => "sending GET, HEAD and OPTIONS requests",
        Permission::SendWrite => "sending requests that change data",
        Permission::EditFiles => "creating and editing requests and folders",
        Permission::ChangeEnvironment => "changing environments",
        Permission::Destructive => "deleting requests, folders, variables and cookies",
    }
}

#[cfg(test)]
#[path = "permissions_tests.rs"]
mod tests;
