//! Environments: one TOML file per environment in the workspace's `environments/` folder.
//!
//! ```toml
//! tag = "staging"
//! secrets = ["token"]
//!
//! [variables]
//! baseUrl = "https://staging.example.com"
//! ```
//!
//! Secret values live in the OS keychain (see [`crate::secrets`]); the file only names them.
//! Reading and writing the file itself is in [`file`].

mod file;
mod values;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::oauth::{self, OAuthClient};
use crate::workspace::{ENVIRONMENTS_DIR, slug};
use crate::{Error, Result, secrets};
pub use file::warnings;
use file::{self as file_io, EnvironmentFile};
pub use values::{plain_variables, secret_env_var, values, values_with};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
    /// The file name without `.toml`.
    pub name: String,
    pub tag: EnvironmentTag,
    pub variables: BTreeMap<String, String>,
    pub secrets: Vec<SecretVariable>,
    /// OAuth 2.0 clients for `{{$auth.token("name")}}`.
    pub auth: Vec<OAuthClient>,
    /// Why the file couldn't be read (e.g. invalid TOML); the other fields are then empty and
    /// the environment can't be saved until the file is fixed. Ignored when saving.
    #[serde(default)]
    pub warning: Option<String>,
}

/// How careful to be. Production asks before anything that changes data.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum EnvironmentTag {
    Production,
    Staging,
    Development,
    #[default]
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SecretVariable {
    pub name: String,
    /// Whether a value is stored in this machine's keychain. The value itself never leaves core.
    pub is_set: bool,
}

/// Resolved values for sending: plain variables and secrets, kept apart so secrets can be
/// masked in everything that is returned or stored.
#[derive(Debug, Clone, Default)]
pub struct Values {
    pub tag: EnvironmentTag,
    pub variables: BTreeMap<String, String>,
    pub secrets: BTreeMap<String, String>,
}

/// Every environment. A file that can't be read is listed with a [`Environment::warning`]
/// instead of failing the whole list.
pub fn list(root: &Path) -> Result<Vec<Environment>> {
    Ok(names(root)?
        .into_iter()
        .map(|name| {
            get(root, &name).unwrap_or_else(|err| Environment {
                name: name.clone(),
                tag: EnvironmentTag::Local,
                variables: BTreeMap::new(),
                secrets: Vec::new(),
                auth: Vec::new(),
                warning: Some(format!("environments/{name}.toml can't be read: {err}")),
            })
        })
        .collect())
}

pub fn get(root: &Path, name: &str) -> Result<Environment> {
    let file = read_file(root, name)?;
    let secrets = file
        .secrets
        .iter()
        .map(|secret| {
            // An unreadable keychain (e.g. no secret service on Linux) shows the secret as
            // unset instead of breaking the listing; sending still reports the real error.
            let is_set = file_io::is_secret_name(secret)
                && match secrets::get(root, name, secret) {
                    Ok(value) => value.is_some(),
                    Err(err) => {
                        tracing::warn!("failed to read secret {secret}: {err}");
                        false
                    }
                };
            Ok(SecretVariable {
                name: secret.clone(),
                is_set,
            })
        })
        .collect::<Result<_>>()?;
    let auth = file
        .auth
        .into_iter()
        .map(|(client, config)| {
            let signed_in = oauth::has_token(root, name, &client)?;
            Ok(OAuthClient::from_file(&client, config, signed_in))
        })
        .collect::<Result<_>>()?;
    Ok(Environment {
        name: name.to_string(),
        tag: file.tag,
        variables: file.variables,
        secrets,
        auth,
        warning: None,
    })
}

/// One OAuth client of an environment, for getting or refreshing its token.
pub fn auth_client(root: &Path, environment: &str, name: &str) -> Result<OAuthClient> {
    let file = read_file(root, environment)?;
    let config = file.auth.get(name).cloned().ok_or_else(|| {
        Error::NotFound(format!("OAuth client {name} in environment {environment}"))
    })?;
    Ok(OAuthClient::from_file(name, config, false))
}

/// Creates an empty environment. Returns its name (slugged for the file name).
pub fn create(root: &Path, name: &str, tag: EnvironmentTag) -> Result<String> {
    let name = slug(name);
    let path = file_path(root, &name)?;
    if path.exists() {
        return Err(Error::Invalid(format!("environment {name} already exists")));
    }
    fs::create_dir_all(root.join(ENVIRONMENTS_DIR))?;
    let file = EnvironmentFile {
        tag,
        ..Default::default()
    };
    file_io::write(&path, &file)?;
    Ok(name)
}

/// Saves tag, variables, the secret names and OAuth clients. Secret values are set separately
/// with [`set_secret`]. Secrets are never removed by leaving them out (a stale editor would
/// otherwise delete a secret someone just added): the file keeps every secret it has, plus new
/// names; [`remove_secret`] removes one. OAuth clients left out are signed out after the file
/// is written.
pub fn update(root: &Path, environment: Environment) -> Result<()> {
    let path = file_path(root, &environment.name)?;
    let old = read_file(root, &environment.name)?;
    let mut secret_names = old.secrets.clone();
    for secret in environment.secrets {
        if !secret_names.contains(&secret.name) {
            check_secret_name(&secret.name)?;
            secret_names.push(secret.name);
        }
    }
    let dropped_clients: Vec<String> = old
        .auth
        .keys()
        .filter(|c| !environment.auth.iter().any(|a| &a.name == *c))
        .cloned()
        .collect();
    let file = EnvironmentFile {
        tag: environment.tag,
        secrets: secret_names,
        variables: environment.variables,
        auth: environment
            .auth
            .iter()
            .map(|client| (client.name.clone(), client.to_file()))
            .collect(),
    };
    file_io::write(&path, &file)?;
    for client in dropped_clients {
        oauth::sign_out(root, &environment.name, &client)?;
    }
    Ok(())
}

/// Sets a plain variable. Secrets are refused: only the user sets them, in the app.
pub fn set_variable(root: &Path, environment: &str, name: &str, value: &str) -> Result<()> {
    let mut file = read_file(root, environment)?;
    if file.secrets.iter().any(|s| s == name) {
        return Err(Error::Invalid(format!(
            "{name} is a secret; the user sets it in the app"
        )));
    }
    file.variables.insert(name.to_string(), value.to_string());
    file_io::write(&file_path(root, environment)?, &file)
}

pub fn remove_variable(root: &Path, environment: &str, name: &str) -> Result<()> {
    let mut file = read_file(root, environment)?;
    if file.variables.remove(name).is_none() {
        return Err(Error::NotFound(format!(
            "variable {name} in environment {environment}"
        )));
    }
    file_io::write(&file_path(root, environment)?, &file)
}

/// Removes a secret: from the file first, then its value from the keychain.
pub fn remove_secret(root: &Path, environment: &str, name: &str) -> Result<()> {
    let mut file = read_file(root, environment)?;
    let before = file.secrets.len();
    file.secrets.retain(|s| s != name);
    if file.secrets.len() == before {
        return Err(Error::NotFound(format!(
            "secret {name} in environment {environment}"
        )));
    }
    file_io::write(&file_path(root, environment)?, &file)?;
    secrets::remove(root, environment, name)
}

/// Deletes the environment file, then its secrets and OAuth tokens from the keychain.
pub fn remove(root: &Path, name: &str) -> Result<()> {
    let file = read_file(root, name)?;
    fs::remove_file(file_path(root, name)?)?;
    for secret in &file.secrets {
        secrets::remove(root, name, secret)?;
    }
    for client in file.auth.keys() {
        oauth::sign_out(root, name, client)?;
    }
    Ok(())
}

/// Stores a secret's value in the keychain and adds its name to the environment if needed.
pub fn set_secret(root: &Path, environment: &str, name: &str, value: &str) -> Result<()> {
    check_secret_name(name)?;
    let mut file = read_file(root, environment)?;
    secrets::set(root, environment, name, value)?;
    if !file.secrets.iter().any(|s| s == name) {
        file.secrets.push(name.to_string());
        file_io::write(&file_path(root, environment)?, &file)?;
    }
    Ok(())
}

/// Environment names (file names without `.toml`), sorted.
pub fn names(root: &Path) -> Result<Vec<String>> {
    file_io::names(root)
}

pub fn exists(root: &Path, name: &str) -> bool {
    file_io::existing_path(root, name).is_ok()
}

/// A hint for "unknown environment `name`", in brackets for the end of that message: why the name
/// can't be used, or the `known` environment that differs only in letter case. Callers pass only
/// the environments the reader may see.
pub fn unknown_hint(name: &str, known: &[String]) -> String {
    if let Some(problem) = file_io::name_problem(name) {
        return format!(" (the name {problem})");
    }
    match known.iter().find(|known| known.eq_ignore_ascii_case(name)) {
        Some(close) => format!(" (did you mean {close}?)"),
        None => String::new(),
    }
}

/// The environment's tag, without touching the keychain.
pub fn tag(root: &Path, name: &str) -> Result<EnvironmentTag> {
    Ok(read_file(root, name)?.tag)
}

/// Names of the environment's secrets, without reading their values.
pub fn secret_names(root: &Path, name: &str) -> Result<Vec<String>> {
    Ok(read_file(root, name)?.secrets)
}

/// The environment file's own plain variables (no workspace defaults, no secrets).
pub fn own_variables(root: &Path, name: &str) -> Result<BTreeMap<String, String>> {
    Ok(read_file(root, name)?.variables)
}

fn check_secret_name(name: &str) -> Result<()> {
    if file_io::is_secret_name(name) {
        return Ok(());
    }
    Err(Error::Invalid(format!(
        "invalid secret name {name}: it can't be empty or contain $ or |"
    )))
}

/// The file's tag, secret names, variables and OAuth clients, without touching the keychain.
pub(crate) fn read_file(root: &Path, name: &str) -> Result<EnvironmentFile> {
    file_io::read(root, name)
}

fn file_path(root: &Path, name: &str) -> Result<PathBuf> {
    file_io::path(root, name)
}

#[cfg(test)]
mod tests;
