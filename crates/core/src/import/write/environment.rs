use std::collections::BTreeMap;
use std::path::Path;

use crate::Result;
use crate::environments::{self, Environment, EnvironmentTag, SecretVariable};
use crate::import::ImportedEnvironment;
use crate::secret_names::looks_secret;
use crate::workspace;

/// What an import does to one environment: creates it, or, when updating an earlier import,
/// adds the variables and secret names it doesn't have yet (the user's values win).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportEnvironmentChange {
    pub name: String,
    pub tag: EnvironmentTag,
    /// The environment is already there; only `variables` and `secrets` are added to it.
    pub existing: bool,
    pub variables: BTreeMap<String, String>,
    pub secrets: Vec<String>,
}

/// What importing `envs` does, in order, without writing anything.
pub(super) fn plan(
    root: &Path,
    envs: Vec<ImportedEnvironment>,
    merged: bool,
    notes: &mut Vec<String>,
) -> Result<Vec<ImportEnvironmentChange>> {
    let existing = environments::names(root)?;
    let mut taken = existing.clone();
    let mut changes = Vec::new();
    for mut env in envs {
        split_secret_variables(&mut env, notes);
        let change = match merged {
            true => added_to_existing(root, &env, &existing)?,
            false => None,
        };
        let change = change.unwrap_or_else(|| created(env, &taken));
        taken.push(change.name.clone());
        changes.push(change);
    }
    Ok(changes)
}

/// Writes one change from [`plan`]. Returns the environment's name.
pub(super) fn apply(root: &Path, change: ImportEnvironmentChange) -> Result<String> {
    if !change.existing {
        return create_environment(root, change);
    }
    let mut current = environments::get(root, &change.name)?;
    for (key, value) in change.variables {
        current.variables.entry(key).or_insert(value);
    }
    current
        .secrets
        .extend(change.secrets.into_iter().map(|name| SecretVariable {
            name,
            is_set: false,
        }));
    environments::update(root, current)?;
    Ok(change.name)
}

/// Moves secret-looking variables (`token`, `apiKey`, …) to secret names; their values are
/// dropped, never written to the workspace.
fn split_secret_variables(env: &mut ImportedEnvironment, notes: &mut Vec<String>) {
    let secret: Vec<String> = env
        .variables
        .keys()
        .filter(|name| looks_secret(name))
        .cloned()
        .collect();
    if secret.is_empty() {
        return;
    }
    for name in &secret {
        env.variables.remove(name);
        if !env.secrets.contains(name) {
            env.secrets.push(name.clone());
        }
    }
    notes.push(format!(
        "environment {}: {} look like secrets and were imported without their values; set them \
         in the app",
        env.name,
        secret.join(", ")
    ));
}

/// The variables and secret names an earlier import's environment doesn't have yet; `None`
/// when there is no such environment, or its file can't be read.
fn added_to_existing(
    root: &Path,
    env: &ImportedEnvironment,
    existing: &[String],
) -> Result<Option<ImportEnvironmentChange>> {
    let name = workspace::slug(&env.name);
    if !existing.contains(&name) {
        return Ok(None);
    }
    let Ok(current) = environments::read_file(root, &name) else {
        return Ok(None);
    };
    Ok(Some(ImportEnvironmentChange {
        tag: current.tag,
        existing: true,
        variables: env
            .variables
            .iter()
            .filter(|(key, _)| !current.variables.contains_key(*key))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
        secrets: env
            .secrets
            .iter()
            .filter(|secret| !current.secrets.contains(secret))
            .cloned()
            .collect(),
        name,
    }))
}

/// A new environment under a free name: `staging`, else `staging-2`, … Letter case doesn't
/// count, as on macOS and Windows `Staging.toml` is the same file.
fn created(env: ImportedEnvironment, taken: &[String]) -> ImportEnvironmentChange {
    let base = workspace::slug(&env.name);
    let free = |name: &str| !taken.iter().any(|t| t.to_lowercase() == name);
    let mut name = base.clone();
    let mut n = 2;
    while !free(&name) {
        name = format!("{base}-{n}");
        n += 1;
    }
    ImportEnvironmentChange {
        name,
        tag: infer_tag(&env.name),
        existing: false,
        variables: env.variables,
        secrets: env.secrets,
    }
}

fn create_environment(root: &Path, change: ImportEnvironmentChange) -> Result<String> {
    let name = environments::create(root, &change.name, change.tag)?;
    let secrets = change
        .secrets
        .into_iter()
        .map(|name| SecretVariable {
            name,
            is_set: false,
        })
        .collect();
    environments::update(
        root,
        Environment {
            name: name.clone(),
            tag: change.tag,
            variables: change.variables,
            secrets,
            auth: Vec::new(),
            warning: None,
        },
    )?;
    Ok(name)
}

/// `production` for names like prod / production / live, and so on; local otherwise.
pub(super) fn infer_tag(name: &str) -> EnvironmentTag {
    let lower = name.to_ascii_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let has = |options: &[&str]| words.iter().any(|w| options.contains(w));
    if has(&["prod", "production", "live", "prd"]) {
        EnvironmentTag::Production
    } else if has(&["staging", "stage", "stg", "preprod", "uat"]) {
        EnvironmentTag::Staging
    } else if has(&["dev", "development", "test", "qa"]) {
        EnvironmentTag::Development
    } else {
        EnvironmentTag::Local
    }
}

#[cfg(test)]
#[path = "environment_tests.rs"]
mod tests;
