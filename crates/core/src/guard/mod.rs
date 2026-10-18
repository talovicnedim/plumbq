//! Extra rules for sends by an AI agent: where secrets and writes may go. The rules:
//! `docs/formats.md` (Rules for sends by an AI agent).

mod files;
mod hosts;
mod redirects;

use std::path::Path;

use crate::environments::{self, EnvironmentTag};
use crate::http_file::Request;
use crate::permissions::{Permission, PermissionContext};
use crate::{Error, Result, chain, file_vars, send, variables};
use hosts::{known_hosts, production_owner};

pub use files::private_file;
pub(crate) use hosts::{allowed_hosts, destination_host};
pub use hosts::{is_private_host, url_host};
pub use redirects::AgentRedirects;
pub(crate) use redirects::AgentResolver;

/// What an agent's send would touch, for [`crate::permissions::decide`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SendAssessment {
    pub method: String,
    /// GET, HEAD or OPTIONS.
    pub safe: bool,
    pub destination: Option<String>,
    /// The production environment the request goes to: the chosen one, or the one owning the
    /// destination host.
    pub production: Option<String>,
    /// Why secrets would go to a host that isn't a URL of the environment, if they would.
    pub secrets_to_new_host: Option<String>,
    /// The `$processEnv` / `$env.` variable used, if any.
    pub process_env: Option<String>,
    /// Why an agent may never send it, if so (e.g. the body sends an environment file).
    pub forbidden: Option<String>,
}

impl SendAssessment {
    pub fn permission(&self) -> Permission {
        match self.safe {
            true => Permission::SendSafe,
            false => Permission::SendWrite,
        }
    }

    pub fn context(&self) -> PermissionContext {
        PermissionContext {
            production: self.production.is_some(),
            secrets_to_new_host: self.secrets_to_new_host.clone(),
            repoints_url: None,
            unexpected_host: None,
            process_env: self.process_env.is_some(),
            forbidden: self.forbidden.clone(),
        }
    }
}

/// Looks at a send without deciding anything.
pub fn assess_agent_send(
    root: &Path,
    id: &str,
    request: &Request,
    environment: Option<&str>,
) -> Result<SendAssessment> {
    let mut text = send::request_text(root, id, request)?;
    for value in file_vars::raw_values(root, id, request)? {
        text.push('\n');
        text.push_str(&value);
    }
    let used = variables::used(&text);
    let method = send::effective_method(root, id, request)?;
    let destination = destination_host(root, id, request, environment)?;
    let files = files::body_files(root, id, request, environment)?;
    let mut sensitive = sensitive_names(root, environment, &used)?;
    sensitive.extend(files.paths.iter().map(|path| format!("file {path}")));
    Ok(SendAssessment {
        safe: send::SAFE_METHODS.contains(&method.as_str()),
        method,
        process_env: used.iter().find(|n| is_process_env(n)).cloned(),
        secrets_to_new_host: secrets_to_new_host(
            root,
            environment,
            &sensitive,
            destination.as_ref(),
        )?,
        production: production_target(root, environment, destination.as_ref())?,
        destination,
        forbidden: files.forbidden,
    })
}

/// The variables in `used` an agent mustn't be able to send anywhere.
fn sensitive_names(root: &Path, environment: Option<&str>, used: &[String]) -> Result<Vec<String>> {
    let secret_names = match environment {
        Some(env) => environments::secret_names(root, env)?,
        None => Vec::new(),
    };
    Ok(used
        .iter()
        .filter(|name| is_sensitive(name, &secret_names))
        .cloned()
        .collect())
}

/// Why the request's secrets and files would go to a host that isn't a URL of the environment,
/// if they would.
fn secrets_to_new_host(
    root: &Path,
    environment: Option<&str>,
    sensitive: &[String],
    destination: Option<&String>,
) -> Result<Option<String>> {
    if sensitive.is_empty() {
        return Ok(None);
    }
    let allowed = allowed_hosts(root, environment)?;
    if destination.is_some_and(|host| allowed.contains(host)) {
        return Ok(None);
    }
    Ok(Some(format!(
        "this request sends secret values ({}) to {}, which isn't the host of a URL in {} ({})",
        sensitive.join(", "),
        destination.map_or("an unknown host", String::as_str),
        environment.map_or("the workspace defaults".to_string(), |e| format!(
            "environment {e}"
        )),
        if allowed.is_empty() {
            "it has none".to_string()
        } else {
            allowed.into_iter().collect::<Vec<_>>().join(", ")
        }
    )))
}

/// The chosen environment when it is production, else the production environment that owns
/// the destination host.
fn production_target(
    root: &Path,
    environment: Option<&str>,
    destination: Option<&String>,
) -> Result<Option<String>> {
    if let Some(env) = environment
        && environments::tag(root, env)? == EnvironmentTag::Production
    {
        return Ok(Some(env.to_string()));
    }
    match destination {
        Some(host) => production_owner(root, host),
        None => Ok(None),
    }
}

/// What an agent's download from `url` (to import it) touches. The agent picks the URL, so it
/// asks the user unless an environment points at the host and it is a public, non-production
/// one.
pub fn assess_download(root: &Path, url: &str) -> Result<PermissionContext> {
    let Some(host) = url_host(url) else {
        return Ok(PermissionContext {
            unexpected_host: Some(format!("{url} has no host the app can check")),
            ..PermissionContext::default()
        });
    };
    let production = production_owner(root, &host)?.is_some();
    let why = match () {
        _ if is_private_host(&host) => Some("an address on this machine or the local network"),
        _ if production => Some("a host of a production environment"),
        _ if !known_hosts(root)?.contains(&host) => Some("a host no environment points at"),
        _ => None,
    };
    Ok(PermissionContext {
        production,
        unexpected_host: why.map(|why| format!("this downloads from {host}, {why}")),
        ..PermissionContext::default()
    })
}

/// Refuses an agent's send that no longer looks as the user approved it (`None`: they never
/// saw it): it changed while they were asked, or during a run, and could now go somewhere
/// riskier. Returns whether it is a production write, which their approval confirms.
pub fn check_approved(
    root: &Path,
    id: &str,
    request: &Request,
    environment: Option<&str>,
    approved: Option<&SendAssessment>,
) -> Result<bool> {
    let now = assess_agent_send(root, id, request, environment)?;
    if let Some(reason) = now.forbidden {
        return Err(Error::Invalid(reason));
    }
    if approved != Some(&now) {
        return Err(Error::Conflict(format!(
            "request {id} changed since the user approved it, or they never saw it; ask again"
        )));
    }
    Ok(!now.safe && now.production.is_some())
}

/// Why an agent's change of a plain variable needs the user, if it does: the host of an
/// existing URL changes, or a non-production environment would point at a production host.
pub fn assess_agent_variable(
    root: &Path,
    environment: &str,
    name: &str,
    value: &str,
) -> Result<Option<String>> {
    let Some(new_host) = url_host(value) else {
        return Ok(None);
    };
    let current = environments::own_variables(root, environment)?;
    if let Some(old_host) = current.get(name).and_then(|v| url_host(v))
        && old_host != new_host
    {
        return Ok(Some(format!(
            "changing {name} from host {old_host} to {new_host} sends this environment's secrets \
             somewhere else"
        )));
    }
    if environments::tag(root, environment)? != EnvironmentTag::Production
        && let Some(prod) = production_owner(root, &new_host)?
    {
        return Ok(Some(format!(
            "{new_host} is the host of production environment {prod}; a {} environment would \
             point at it",
            environment_tag_name(environments::tag(root, environment)?)
        )));
    }
    Ok(None)
}

fn environment_tag_name(tag: EnvironmentTag) -> &'static str {
    match tag {
        EnvironmentTag::Production => "production",
        EnvironmentTag::Staging => "staging",
        EnvironmentTag::Development => "development",
        EnvironmentTag::Local => "local",
    }
}

fn is_process_env(name: &str) -> bool {
    name.starts_with("$env.") || name.starts_with("$processEnv")
}

/// Values an agent mustn't be able to send anywhere: keychain secrets, `.env` and process
/// environment values, OAuth tokens and earlier responses.
fn is_sensitive(name: &str, secret_names: &[String]) -> bool {
    secret_names.iter().any(|s| s == name)
        || name.starts_with("$dotenv")
        || is_process_env(name)
        || name.starts_with("$auth.token")
        || chain::is_reference(name)
}

#[cfg(test)]
mod tests;
