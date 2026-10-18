//! Bruno collection folders: `bruno.json` (or `opencollection.yml` since Bruno 3), one `.bru` or
//! `.yml` file per request, `folder.bru`/`folder.yml` for folder settings and `environments/`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::lang::{self, dict_of, get, list_of};
use super::yaml::{yaml_auth, yaml_headers, yaml_request};
use super::{bru_auth, bru_request, convert, detect_yaml, seq};
use crate::import::{
    ImportFormat, ImportedCollection, ImportedEnvironment, ImportedItem, load, scalar, text,
};
use crate::{Error, Result};

const SKIPPED: [&str; 4] = ["environments", "node_modules", ".git", ".bruno"];
const SETTINGS_FILES: [&str; 4] = [
    "folder.bru",
    "collection.bru",
    "folder.yml",
    "opencollection.yml",
];

pub fn parse_dir(dir: &Path) -> Result<ImportedCollection> {
    let name = collection_name(dir)?;
    let (headers, _) = settings(dir, "collection")?;
    let mut environments = Vec::new();
    let env_dir = dir.join("environments");
    if env_dir.is_dir() {
        for entry in sorted_entries(&env_dir)? {
            if let Some(env) = environment(&entry)? {
                environments.push(env);
            }
        }
    }
    Ok(ImportedCollection {
        format: ImportFormat::Bruno,
        notes: vec!["scripts and tests are not imported".into()],
        name,
        headers,
        items: items(dir)?,
        environments,
        credentials: Vec::new(),
    })
}

fn collection_name(dir: &Path) -> Result<String> {
    let json = dir.join("bruno.json");
    let yaml = dir.join("opencollection.yml");
    let doc = match () {
        _ if json.is_file() => load(&fs::read_to_string(json)?)?,
        _ if yaml.is_file() => load(&fs::read_to_string(yaml)?)?,
        _ => {
            return Err(Error::Invalid(format!(
                "{} is not a Bruno collection (no bruno.json or opencollection.yml)",
                dir.display()
            )));
        }
    };
    let name = doc
        .pointer("/info/name")
        .map(scalar)
        .unwrap_or_else(|| text(&doc, "name"));
    Ok(match name.is_empty() {
        true => dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        false => name,
    })
}

/// Requests and subfolders of `dir`, in Bruno's `seq` order.
fn items(dir: &Path) -> Result<Vec<ImportedItem>> {
    let mut found: Vec<(u32, String, ImportedItem)> = Vec::new();
    for path in sorted_entries(dir)? {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let item = match path.is_dir() {
            true => folder_item(&path, &file_name)?,
            false => request_item(&path, &file_name)?,
        };
        found.extend(item);
    }
    found.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    Ok(found.into_iter().map(|(_, _, item)| item).collect())
}

/// A subfolder with its `seq` and name; `None` when skipped or empty.
fn folder_item(path: &Path, file_name: &str) -> Result<Option<(u32, String, ImportedItem)>> {
    if SKIPPED.contains(&file_name) {
        return Ok(None);
    }
    let (headers, (name, seq)) = settings(path, "folder")?;
    let name = if name.is_empty() {
        file_name.to_string()
    } else {
        name
    };
    let items = items(path)?;
    if items.is_empty() {
        return Ok(None);
    }
    let folder = ImportedItem::Folder {
        name: name.clone(),
        headers,
        items,
    };
    Ok(Some((seq, name, folder)))
}

/// A request file with its `seq` and name; `None` for settings files and other files.
fn request_item(path: &Path, file_name: &str) -> Result<Option<(u32, String, ImportedItem)>> {
    if SETTINGS_FILES.contains(&file_name) {
        return Ok(None);
    }
    let parsed = match path.extension().and_then(|e| e.to_str()) {
        Some("bru") => bru_request(&lang::parse(&fs::read_to_string(path)?)),
        Some("yml" | "yaml") => {
            let doc = load(&fs::read_to_string(path)?)?;
            detect_yaml(&doc).then(|| yaml_request(&doc))
        }
        _ => None,
    };
    let Some((seq, mut request)) = parsed else {
        return Ok(None);
    };
    if request.name.is_empty() {
        request.name = file_name
            .rsplit_once('.')
            .map_or(file_name.to_string(), |(stem, _)| stem.to_string());
    }
    Ok(Some((
        seq,
        request.name.clone(),
        ImportedItem::Request(request),
    )))
}

/// Headers (with auth) and `(name, seq)` from `folder.bru` / `collection.bru` or `folder.yml`.
fn settings(dir: &Path, kind: &str) -> Result<(BTreeMap<String, String>, (String, u32))> {
    let bru = dir.join(format!("{kind}.bru"));
    if bru.is_file() {
        let blocks = lang::parse(&fs::read_to_string(bru)?);
        let meta = dict_of(&blocks, "meta");
        let mut headers: BTreeMap<String, String> = dict_of(&blocks, "headers")
            .iter()
            .map(|(k, v)| (k.clone(), convert(v)))
            .collect();
        let mode = get(dict_of(&blocks, "auth"), "mode");
        headers.extend(
            bru_auth(&blocks, &mode)
                .into_iter()
                .map(|h| (h.name, h.value)),
        );
        return Ok((headers, (get(meta, "name"), seq(&get(meta, "seq")))));
    }
    let yml = dir.join(format!("{kind}.yml"));
    if yml.is_file() {
        let doc = load(&fs::read_to_string(yml)?)?;
        let mut headers: BTreeMap<String, String> = yaml_headers(&doc)
            .into_iter()
            .map(|h| (h.name, h.value))
            .collect();
        headers.extend(yaml_auth(&doc).into_iter().map(|h| (h.name, h.value)));
        let name = doc.pointer("/info/name").map(scalar).unwrap_or_default();
        let order = doc.pointer("/info/seq").map(scalar).unwrap_or_default();
        return Ok((headers, (name, seq(&order))));
    }
    Ok((BTreeMap::new(), (String::new(), u32::MAX)))
}

fn environment(path: &Path) -> Result<Option<ImportedEnvironment>> {
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    match path.extension().and_then(|e| e.to_str()) {
        Some("bru") => {
            let blocks = lang::parse(&fs::read_to_string(path)?);
            let variables = dict_of(&blocks, "vars")
                .iter()
                .map(|(k, v)| (k.clone(), convert(v)))
                .collect();
            Ok(Some(ImportedEnvironment {
                name,
                variables,
                secrets: list_of(&blocks, "vars:secret"),
            }))
        }
        Some("yml" | "yaml") => {
            let doc = load(&fs::read_to_string(path)?)?;
            let mut env = ImportedEnvironment {
                name,
                variables: BTreeMap::new(),
                secrets: Vec::new(),
            };
            for var in doc
                .get("variables")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let var_name = text(var, "name");
                if var.get("secret").and_then(Value::as_bool).unwrap_or(false) {
                    env.secrets.push(var_name);
                } else if !var
                    .get("disabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    env.variables.insert(
                        var_name,
                        convert(&scalar(var.get("value").unwrap_or(&Value::Null))),
                    );
                }
            }
            Ok(Some(env))
        }
        _ => Ok(None),
    }
}

/// Symlinks are left out, so a link can't loop the walk or pull in files from outside the
/// collection.
fn sorted_entries(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paths: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| !t.is_symlink()))
        .map(|e| e.path())
        .collect();
    paths.sort();
    Ok(paths)
}
