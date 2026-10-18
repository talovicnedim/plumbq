//! Imports Postman, OpenAPI 3, Swagger 2, Insomnia, HAR and Bruno collections (format detected
//! from the content) into a folder of plain `.http` files.

mod bruno;
pub(crate) mod credentials;
mod download;
mod har;
mod insomnia;
mod merge;
mod openapi;
mod postman;
mod schema;
mod write;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::http_file::Request;
use crate::{Error, Result};

pub use download::download;
pub use write::{ImportEnvironmentChange, environment_changes, write};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ImportFormat {
    Postman,
    OpenApi,
    Swagger,
    Insomnia,
    Har,
    Bruno,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub format: ImportFormat,
    /// Workspace-relative path of the folder holding the import.
    pub folder: String,
    /// The folder already held an earlier import of this collection and was updated in place.
    pub merged: bool,
    /// Requests in the source.
    pub requests: u32,
    /// New request files written.
    pub added: u32,
    /// Existing requests whose name changed in the source. Their headers and body are kept.
    pub updated: u32,
    /// Existing requests already matching the source.
    pub unchanged: u32,
    /// Ids of requests from an earlier import that are no longer in the source. Never deleted.
    pub stale: Vec<String>,
    /// Folders created below `folder`.
    pub folders: u32,
    /// Names of the environments created or extended at the workspace root.
    pub environments: Vec<String>,
    /// Things worth telling the user, e.g. "skipped 12 static assets".
    pub notes: Vec<String>,
}

/// A parsed collection, independent of its source format.
#[derive(Debug, Clone)]
pub struct ImportedCollection {
    pub format: ImportFormat,
    /// Notes from parsing, copied into the summary.
    pub notes: Vec<String>,
    pub name: String,
    /// Headers for every request (e.g. collection-level auth), stored in the folder's
    /// `folder.toml`.
    pub headers: BTreeMap<String, String>,
    pub items: Vec<ImportedItem>,
    pub environments: Vec<ImportedEnvironment>,
    /// Literal credentials the source declares in its auth settings; replaced by secrets
    /// wherever they appear (see [`credentials`]).
    pub credentials: Vec<String>,
}

// Items only live while an import runs; boxing every request would add noise to every
// importer for no measurable gain.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum ImportedItem {
    Folder {
        name: String,
        headers: BTreeMap<String, String>,
        items: Vec<ImportedItem>,
    },
    Request(Request),
}

#[derive(Debug, Clone)]
pub struct ImportedEnvironment {
    pub name: String,
    pub variables: BTreeMap<String, String>,
    /// Names only; values are never imported into files and must be set by the user.
    pub secrets: Vec<String>,
}

pub fn parse(text: &str) -> Result<ImportedCollection> {
    let doc = load(text)?;
    if doc.get("openapi").is_some() {
        return openapi::parse(&doc, ImportFormat::OpenApi);
    }
    if doc.get("swagger").is_some() {
        return openapi::parse(&doc, ImportFormat::Swagger);
    }
    if insomnia::detect(&doc) {
        return insomnia::parse(&doc);
    }
    if postman::detect(&doc) {
        return postman::parse(&doc);
    }
    if har::detect(&doc) {
        return Ok(har::parse(&doc));
    }
    if bruno::detect_yaml(&doc) {
        return bruno::parse_yaml_request(&doc);
    }
    Err(Error::Invalid(
        "unrecognized file: expected a Postman, OpenAPI, Swagger, Insomnia, HAR or Bruno export"
            .into(),
    ))
}

/// The file an AI agent may import: `path` (relative to `root`, or absolute) with links
/// resolved, when it lies in the workspace or one of `extra_dirs`. Agents can't read or probe
/// for other files on the user's machine.
pub fn allowed_path(root: &Path, path: &str, extra_dirs: &[&Path]) -> Result<PathBuf> {
    let given = Path::new(path);
    let full = if given.is_absolute() {
        given.to_path_buf()
    } else {
        root.join(given)
    };
    let resolved = crate::workspace::canonical_root(&full)
        .map_err(|_| Error::NotFound(full.display().to_string()))?;
    if std::iter::once(root)
        .chain(extra_dirs.iter().copied())
        .any(|dir| resolved.starts_with(dir))
    {
        return Ok(resolved);
    }
    Err(Error::Invalid(format!("{path} is outside the workspace")))
}

/// Imports a file, or a Bruno collection folder, into `folder`.
pub fn file(root: &Path, folder: &str, path: &Path) -> Result<ImportSummary> {
    write(root, folder, read_file(path)?)
}

pub fn read_file(path: &Path) -> Result<ImportedCollection> {
    if path.is_dir() {
        return bruno::dir::parse_dir(path);
    }
    let text = fs::read_to_string(path)
        .map_err(|err| Error::Invalid(format!("failed to read {}: {err}", path.display())))?;
    if path.extension().is_some_and(|ext| ext == "bru") {
        return bruno::parse_bru_file(path, &text);
    }
    parse(&text)
}

/// Downloads a spec (e.g. `https://petstore3.swagger.io/api/v3/openapi.json`) and imports it.
/// Importing the same URL again updates the earlier import instead of duplicating it.
pub async fn url(root: &Path, folder: &str, url: &str) -> Result<ImportSummary> {
    let collection = download(url, None).await?;
    let (root, folder) = (root.to_path_buf(), folder.to_string());
    crate::blocking::run(move || write(&root, &folder, collection)).await
}

/// JSON or YAML into one value type, so importers walk a single shape.
fn load(text: &str) -> Result<Value> {
    if text.trim_start().starts_with(['{', '[']) {
        return Ok(serde_json::from_str(text)?);
    }
    serde_saphyr::from_str(text).map_err(|err| Error::Invalid(format!("invalid yaml: {err}")))
}

/// A string field, or `""`.
fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Any JSON scalar as text; objects and arrays as compact JSON.
fn scalar(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// `api.example.com:8080` → `apiExampleCom8080`.
fn camel(text: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in text.chars() {
        if !c.is_ascii_alphanumeric() {
            upper = !out.is_empty();
            continue;
        }
        match upper {
            true => out.push(c.to_ascii_uppercase()),
            false => out.push(c.to_ascii_lowercase()),
        }
        upper = false;
    }
    out
}

#[cfg(test)]
mod tests;
