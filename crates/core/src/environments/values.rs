//! The values a send resolves `{{variables}}` with: workspace defaults, the environment's own
//! variables, and secrets from `PLUMBQ_SECRET_<NAME>` or the keychain.

use std::collections::BTreeMap;
use std::path::Path;

use super::file as file_io;
use crate::environments::Values;
use crate::{Result, secrets, workspace};

/// Everything needed to resolve `{{variables}}`. `None` means no environment: no variables,
/// treated like local.
///
/// A secret's value comes from the process environment variable `PLUMBQ_SECRET_<NAME>`
/// when set (for CI, where there is no keychain), otherwise from the keychain. A keychain that
/// can't be read counts as "not set" with a warning, so requests that don't use secrets still
/// work; requests that do fail with "unknown variables".
pub fn values(root: &Path, name: Option<&str>) -> Result<Values> {
    values_with(root, name, &|var| std::env::var(var).ok())
}

/// [`values`] with the process environment passed in, so tests don't depend on it.
pub fn values_with(
    root: &Path,
    name: Option<&str>,
    process_env: &dyn Fn(&str) -> Option<String>,
) -> Result<Values> {
    // Workspace-wide defaults from the root folder.toml; the environment overrides them.
    let mut variables = workspace::read_settings(root)?.variables;
    let Some(name) = name else {
        return Ok(Values {
            variables,
            ..Values::default()
        });
    };
    let file = file_io::read(root, name)?;
    let mut resolved = BTreeMap::new();
    for secret in file.secrets.iter().filter(|s| file_io::is_secret_name(s)) {
        if let Some(value) = process_env(&secret_env_var(secret)) {
            resolved.insert(secret.clone(), value);
            continue;
        }
        match secrets::get(root, name, secret) {
            Ok(Some(value)) => {
                resolved.insert(secret.clone(), value);
            }
            Ok(None) => {}
            Err(err) => tracing::warn!("secret {secret} is unavailable: {err}"),
        }
    }
    variables.extend(file.variables);
    Ok(Values {
        tag: file.tag,
        variables,
        secrets: resolved,
    })
}

/// `PLUMBQ_SECRET_<NAME>`: the name upper-cased, anything but letters and digits as `_`.
pub fn secret_env_var(secret: &str) -> String {
    let name: String = secret
        .chars()
        .map(|c| match c.is_ascii_alphanumeric() {
            true => c.to_ascii_uppercase(),
            false => '_',
        })
        .collect();
    format!("PLUMBQ_SECRET_{name}")
}

/// Plain variables as a send sees them: workspace defaults overridden by the environment's.
/// Never reads the keychain.
pub fn plain_variables(root: &Path, name: Option<&str>) -> Result<BTreeMap<String, String>> {
    let mut variables = workspace::read_settings(root)?.variables;
    if let Some(name) = name {
        variables.extend(file_io::read(root, name)?.variables);
    }
    Ok(variables)
}
