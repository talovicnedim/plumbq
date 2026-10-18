//! What the request editor shows about an unsaved draft: how each `{{variable}}` resolves, its
//! tags and docs, a file body, and body lines the `.http` format can't keep.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::dynamic::{self, DynamicKind};
use crate::environments::{self, Environment};
use crate::http_file::{self, Request};
use crate::{Result, chain, file_vars, folders, multipart, oauth, request_meta, send, variables};

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RequestView {
    /// Every `{{variable}}` in the URL, headers and body, in order of first use.
    pub variables: Vec<VariableHint>,
    pub tags: Vec<String>,
    /// Markdown from the request's plain comment lines.
    pub docs: String,
    /// The path of a `< ./path` file body.
    pub file_reference: Option<String>,
    /// Body lines that would turn into something else when the file is read again.
    pub body_warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct VariableHint {
    pub name: String,
    pub kind: VariableKind,
    /// Plain values, the OAuth client or the chained request; never a secret.
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum VariableKind {
    Value,
    Secret,
    /// `$uuid`, `$timestamp` and friends, made up at send time.
    Dynamic,
    /// `{{$prompt name}}`: asked when sending.
    Prompt,
    /// `{{$processEnv NAME}}` / `{{$env.NAME}}`: the app's OS environment.
    ProcessEnv,
    /// `{{$dotenv NAME}}`: the workspace's `.env` file.
    Dotenv,
    /// `{{login.response.body.$.token}}`: another request's latest response.
    Chain,
    /// `{{$auth.token("github")}}`: an OAuth client's access token.
    Oauth,
    /// A workspace-wide default from the root `folder.toml`.
    Workspace,
    /// `@name = value` in the request's `.http` file.
    File,
    Missing,
}

/// Reads secret states from the keychain, so call it off the async runtime.
pub fn request(
    root: &Path,
    id: &str,
    request: &Request,
    environment: Option<&str>,
) -> Result<RequestView> {
    let sources = Sources {
        environment: environment
            .map(|name| environments::get(root, name))
            .transpose()?,
        defaults: folders::get_settings(root, "")?.variables,
        file: file_vars::collect(root, id, request)?
            .into_iter()
            .map(|v| (v.name, v.value))
            .collect(),
    };
    let text = send::request_text(root, id, request)?;
    Ok(RequestView {
        variables: variables::used(&text)
            .into_iter()
            .map(|name| sources.hint(name))
            .collect(),
        tags: request_meta::tags(request),
        docs: request_meta::docs(request),
        file_reference: file_reference(&request.body),
        body_warnings: http_file::body_warnings(request),
    })
}

/// Where variable values can come from, in the order a send looks them up.
struct Sources {
    environment: Option<Environment>,
    defaults: BTreeMap<String, String>,
    file: BTreeMap<String, String>,
}

impl Sources {
    fn hint(&self, name: String) -> VariableHint {
        let (kind, value) = self.resolve(&name);
        VariableHint { name, kind, value }
    }

    fn resolve(&self, name: &str) -> (VariableKind, String) {
        if let Some(kind) = dynamic::kind(name) {
            let kind = match kind {
                DynamicKind::Prompt => VariableKind::Prompt,
                DynamicKind::ProcessEnv => VariableKind::ProcessEnv,
                DynamicKind::Dotenv => VariableKind::Dotenv,
            };
            return (kind, String::new());
        }
        if let Some(client) = oauth::client_name(name) {
            return (VariableKind::Oauth, client.to_string());
        }
        if let Some(request) = chain::source_request(name) {
            return (VariableKind::Chain, request.to_string());
        }
        if let Some(value) = self.file.get(name) {
            return (VariableKind::File, value.clone());
        }
        self.resolve_environment(name)
    }

    fn resolve_environment(&self, name: &str) -> (VariableKind, String) {
        let env = self.environment.as_ref();
        if let Some(secret) = env.and_then(|e| e.secrets.iter().find(|s| s.name == name)) {
            let kind = match secret.is_set {
                true => VariableKind::Secret,
                false => VariableKind::Missing,
            };
            return (kind, String::new());
        }
        if let Some(value) = env.and_then(|e| e.variables.get(name)) {
            return (VariableKind::Value, value.clone());
        }
        if let Some(value) = self.defaults.get(name) {
            return (VariableKind::Workspace, value.clone());
        }
        match variables::is_builtin(name) {
            true => (VariableKind::Dynamic, String::new()),
            false => (VariableKind::Missing, String::new()),
        }
    }
}

/// A body that is only a `< ./path` line sends that file.
fn file_reference(body: &str) -> Option<String> {
    let mut lines = body.lines().filter(|line| !line.trim().is_empty());
    let line = lines.next()?;
    if lines.next().is_some() {
        return None;
    }
    multipart::file_reference(line).map(str::to_string)
}

#[cfg(test)]
#[path = "inspect_tests.rs"]
mod tests;
