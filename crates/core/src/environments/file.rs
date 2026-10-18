//! Reading and writing environment files (`environments/<name>.toml`).
//!
//! Writes edit the TOML document in place, so the user's comments, formatting and keys the app
//! doesn't know survive a save. Variable values that aren't strings (`port = 8080`) are read as
//! text and keep their type when written back unchanged.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use toml_edit::{Array, DocumentMut, Item, Table, value};

use crate::environments::EnvironmentTag;
use crate::oauth::OAuthClientFile;
use crate::workspace::{ENVIRONMENTS_DIR, WorkspaceWarning};
use crate::{Error, Result, fs_util};

/// The parts of an environment file the app works with.
#[derive(Debug, Default)]
pub(crate) struct EnvironmentFile {
    pub tag: EnvironmentTag,
    pub secrets: Vec<String>,
    pub variables: BTreeMap<String, String>,
    pub auth: BTreeMap<String, OAuthClientFile>,
}

#[derive(Deserialize)]
struct Raw {
    #[serde(default)]
    tag: EnvironmentTag,
    #[serde(default)]
    secrets: Vec<String>,
    #[serde(default)]
    variables: BTreeMap<String, toml::Value>,
    #[serde(default)]
    auth: BTreeMap<String, OAuthClientFile>,
}

/// Most file systems cap a file name at 255 bytes.
const MAX_FILE_NAME: usize = 255;

/// Secret names are part of their keychain entry: `|` separates its parts and `$` marks the
/// app's own entries (OAuth tokens, the PFX password), so names with them are never used.
pub(crate) fn is_secret_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['$', '|'])
}

pub(crate) fn read(root: &Path, name: &str) -> Result<EnvironmentFile> {
    let path = existing_path(root, name)?;
    let raw: Raw = toml::from_str(&fs::read_to_string(path)?)?;
    Ok(EnvironmentFile {
        tag: raw.tag,
        secrets: raw.secrets,
        variables: raw
            .variables
            .into_iter()
            .map(|(key, value)| (key, as_text(&value)))
            .collect(),
        auth: raw.auth,
    })
}

/// A variable value as text: strings as they are, other values as TOML writes them.
fn as_text(value: &toml::Value) -> String {
    match value {
        toml::Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// Writes `file` over the existing document (or a new one), changing only what differs.
pub(crate) fn write(path: &Path, file: &EnvironmentFile) -> Result<()> {
    let invalid = |err: toml_edit::TomlError| {
        Error::Invalid(format!(
            "{} isn't valid TOML ({err}); fix it before changing it here",
            path.display()
        ))
    };
    let (mut doc, old) = match path.is_file() {
        true => {
            let text = fs::read_to_string(path)?;
            let doc: DocumentMut = text.parse().map_err(invalid)?;
            let old: Raw = toml::from_str(&text)?;
            (doc, Some(old))
        }
        false => (DocumentMut::new(), None),
    };
    if old.as_ref().is_none_or(|old| old.tag != file.tag) {
        doc["tag"] = value(tag_name(file.tag));
    }
    write_secrets(&mut doc, &file.secrets);
    write_variables(&mut doc, &file.variables, old.as_ref());
    write_auth(&mut doc, &file.auth, old.as_ref(), invalid)?;
    fs_util::write_atomic(path, doc.to_string())?;
    Ok(())
}

fn write_secrets(doc: &mut DocumentMut, secrets: &[String]) {
    if secrets.is_empty() {
        doc.remove("secrets");
        return;
    }
    let unchanged = doc
        .get("secrets")
        .and_then(Item::as_array)
        .is_some_and(|array| {
            array
                .iter()
                .map(|v| v.as_str())
                .eq(secrets.iter().map(|s| Some(s.as_str())))
        });
    if !unchanged {
        let array: Array = secrets.iter().map(String::as_str).collect();
        doc["secrets"] = value(array);
    }
}

fn write_variables(doc: &mut DocumentMut, variables: &BTreeMap<String, String>, old: Option<&Raw>) {
    if !doc.contains_key("variables") {
        doc["variables"] = Item::Table(Table::new());
    }
    let Some(table) = doc["variables"].as_table_like_mut() else {
        return;
    };
    let stale: Vec<String> = table
        .iter()
        .map(|(key, _)| key.to_string())
        .filter(|key| !variables.contains_key(key))
        .collect();
    for key in stale {
        table.remove(&key);
    }
    for (key, new) in variables {
        let before = old.and_then(|old| old.variables.get(key)).map(as_text);
        if before.as_deref() == Some(new.as_str()) {
            continue;
        }
        match table.get_mut(key) {
            // In place, so the key and the comments around it stay as written.
            Some(item) => {
                let decor = item.as_value().map(|v| v.decor().clone());
                *item = value(new.as_str());
                if let (Some(decor), Some(new)) = (decor, item.as_value_mut()) {
                    *new.decor_mut() = decor;
                }
            }
            None => {
                table.insert(key, value(new.as_str()));
            }
        }
    }
}

fn write_auth(
    doc: &mut DocumentMut,
    auth: &BTreeMap<String, OAuthClientFile>,
    old: Option<&Raw>,
    invalid: impl Fn(toml_edit::TomlError) -> Error,
) -> Result<()> {
    let serialized = |clients: &BTreeMap<String, OAuthClientFile>| -> Result<String> {
        #[derive(serde::Serialize)]
        struct Wrapper<'a> {
            auth: &'a BTreeMap<String, OAuthClientFile>,
        }
        Ok(toml::to_string(&Wrapper { auth: clients })?)
    };
    if let Some(old) = old
        && serialized(&old.auth)? == serialized(auth)?
    {
        return Ok(());
    }
    if auth.is_empty() {
        doc.remove("auth");
        return Ok(());
    }
    let fresh: DocumentMut = serialized(auth)?.parse().map_err(invalid)?;
    if let Some(item) = fresh.get("auth") {
        doc["auth"] = item.clone();
    }
    Ok(())
}

fn tag_name(tag: EnvironmentTag) -> &'static str {
    match tag {
        EnvironmentTag::Production => "production",
        EnvironmentTag::Staging => "staging",
        EnvironmentTag::Development => "development",
        EnvironmentTag::Local => "local",
    }
}

/// The file of environment `name`. Almost any name works (`my env`, `staging.eu`), so existing
/// files keep their keychain secrets; see [`name_problem`] for what doesn't. The name never
/// holds the `$` and `|` the keychain uses for the app's own entries.
pub(crate) fn path(root: &Path, name: &str) -> Result<PathBuf> {
    if let Some(problem) = name_problem(name) {
        return Err(Error::Invalid(format!(
            "invalid environment name {name}: it {problem}"
        )));
    }
    Ok(root.join(ENVIRONMENTS_DIR).join(format!("{name}.toml")))
}

/// Why `name` can't be an environment's file name, or `None` when it can. The check keeps the
/// file inside `environments/` and the name out of the keychain's own entries.
pub(crate) fn name_problem(name: &str) -> Option<&'static str> {
    if name.is_empty() {
        Some("is empty")
    } else if name.starts_with('.') {
        Some("starts with a dot (those files are hidden)")
    } else if name.starts_with('$') {
        Some("starts with $, which the app keeps for itself")
    } else if name.contains('|') {
        Some("has a | in it, which the app keeps for itself")
    } else if name.contains(['/', '\\']) {
        Some("has a slash in it")
    } else if name.chars().any(char::is_control) {
        Some("has a control character in it")
    } else if name.len() + ".toml".len() > MAX_FILE_NAME {
        Some("is too long for a file name")
    } else {
        None
    }
}

/// The file of environment `name`, when a file has exactly that name. On case-insensitive file
/// systems (macOS, Windows) `PROD.toml` also opens `prod.toml`, which would get around hidden
/// environments.
pub(crate) fn existing_path(root: &Path, name: &str) -> Result<PathBuf> {
    let path = path(root, name)?;
    let file_name = format!("{name}.toml");
    let listed = fs::read_dir(root.join(ENVIRONMENTS_DIR)).is_ok_and(|mut entries| {
        entries.any(|entry| entry.is_ok_and(|e| e.file_name() == file_name.as_str()))
    });
    if !listed || !path.is_file() {
        return Err(Error::NotFound(format!("environment {name}")));
    }
    Ok(path)
}

/// Environment names (file names without `.toml`), sorted.
pub(crate) fn names(root: &Path) -> Result<Vec<String>> {
    Ok(toml_names(root)?
        .into_iter()
        .filter(|name| name_problem(name).is_none())
        .collect())
}

/// Environment files left out because [`path`] refuses their name (`a|b.toml`), so the user
/// is told instead of the environment vanishing. Hidden files (`.draft.toml`) are left out
/// without a word.
pub fn warnings(root: &Path) -> Result<Vec<WorkspaceWarning>> {
    Ok(toml_names(root)?
        .into_iter()
        .filter(|name| !name.starts_with('.'))
        .filter_map(|name| {
            let problem = name_problem(&name)?;
            Some(WorkspaceWarning {
                message: format!(
                    "this environment is skipped: its name {problem}; rename the file to use it"
                ),
                path: format!("{ENVIRONMENTS_DIR}/{name}.toml"),
            })
        })
        .collect())
}

fn toml_names(root: &Path) -> Result<Vec<String>> {
    let dir = root.join(ENVIRONMENTS_DIR);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut names: Vec<String> = fs::read_dir(&dir)?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            name.strip_suffix(".toml").map(str::to_string)
        })
        .collect();
    names.sort();
    Ok(names)
}
