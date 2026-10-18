//! `folder.toml`: order, shared headers and (in the root) workspace defaults. Writes edit the
//! TOML in place, so comments and unknown keys survive.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use toml_edit::{DocumentMut, Item};

use crate::http_client::HttpSettings;
use crate::workspace::{FOLDER_FILE, SortKey, sort_entries};
use crate::{Error, Result, fs_util};

/// The `folder.toml` keys [`FolderSettings`] owns; anything else in the file is the user's.
const SETTINGS_KEYS: [&str; 4] = ["order", "headers", "variables", "settings"];

/// Per-folder settings from `folder.toml`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FolderSettings {
    /// File and directory names in display order. Unlisted children follow, folders first,
    /// then alphabetically.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<String>,
    /// Sent with every request in this folder and below; closer folders win.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    /// Only read from the root `folder.toml`: workspace-wide defaults for `{{variables}}`,
    /// used by every environment (and by "no environment"); environments override them.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "text_values"
    )]
    #[specta(type = BTreeMap<String, String>)]
    pub variables: BTreeMap<String, String>,
    /// Only read from the root `folder.toml`: timeouts, redirects, TLS and proxy for every
    /// request (see [`crate::http_client`]).
    #[serde(default, skip_serializing_if = "HttpSettings::is_default")]
    pub settings: HttpSettings,
}

/// Values that aren't strings (`port = 8080`) are read as text, like environment variables.
fn text_values<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<BTreeMap<String, String>, D::Error> {
    let values = BTreeMap::<String, toml::Value>::deserialize(deserializer)?;
    Ok(values
        .into_iter()
        .map(|(key, value)| match value {
            toml::Value::String(text) => (key, text),
            other => (key, other.to_string()),
        })
        .collect())
}

pub fn read_settings(dir: &Path) -> Result<FolderSettings> {
    let path = dir.join(FOLDER_FILE);
    if !path.exists() {
        return Ok(FolderSettings::default());
    }
    Ok(toml::from_str(&fs::read_to_string(path)?)?)
}

/// Fails when `dir`'s `folder.toml` can't be read, so a change that has to update its order
/// stops before it touches any file.
pub fn check_settings(dir: &Path) -> Result<()> {
    read_settings(dir).map(|_| ()).map_err(|err| {
        Error::Invalid(format!(
            "{} can't be read ({err}); fix it before changing this folder",
            dir.join(FOLDER_FILE).display()
        ))
    })
}

/// Writes the settings into `folder.toml`, changing only the keys whose value changed, so the
/// user's comments and any keys we don't know stay as written. Refuses to touch a
/// `folder.toml` that isn't valid TOML, rather than replacing it.
pub fn write_settings(dir: &Path, settings: &FolderSettings) -> Result<()> {
    let path = dir.join(FOLDER_FILE);
    let invalid = |err: toml_edit::TomlError| {
        Error::Invalid(format!(
            "{} isn't valid TOML ({err}); fix it before changing it here",
            path.display()
        ))
    };
    let (mut doc, old) = match path.exists() {
        true => {
            let text = fs::read_to_string(&path)?;
            let doc: DocumentMut = text.parse().map_err(invalid)?;
            (doc, toml::from_str(&text)?)
        }
        false => (DocumentMut::new(), FolderSettings::default()),
    };
    let before: DocumentMut = toml::to_string(&old)?.parse().map_err(invalid)?;
    let after: DocumentMut = toml::to_string(settings)?.parse().map_err(invalid)?;
    for key in SETTINGS_KEYS {
        update_key(&mut doc, key, before.get(key), after.get(key));
    }
    fs_util::write_atomic(&path, doc.to_string())?;
    Ok(())
}

/// Changes `doc[key]` from `before` to `after`. Tables change key by key, so the comments and
/// unknown keys inside them stay.
fn update_key(doc: &mut DocumentMut, key: &str, before: Option<&Item>, after: Option<&Item>) {
    if same(before, after) {
        return;
    }
    let old_table = before.and_then(Item::as_table);
    let empty = toml_edit::Table::new();
    let new_table = match after {
        Some(item) => item.as_table(),
        None => old_table.map(|_| &empty),
    };
    match (
        new_table,
        doc.get_mut(key).and_then(Item::as_table_like_mut),
    ) {
        (Some(new), Some(existing)) => {
            update_table(existing, old_table, new);
            if after.is_none() && existing.is_empty() {
                doc.remove(key);
            }
        }
        _ => replace(doc, key, after),
    }
}

fn update_table(
    existing: &mut dyn toml_edit::TableLike,
    before: Option<&toml_edit::Table>,
    after: &toml_edit::Table,
) {
    for (key, _) in before.iter().flat_map(|table| table.iter()) {
        if !after.contains_key(key) {
            existing.remove(key);
        }
    }
    for (key, item) in after.iter() {
        if same(before.and_then(|table| table.get(key)), Some(item)) {
            continue;
        }
        match existing.get_mut(key) {
            Some(old) => {
                let mut item = item.clone();
                keep_decor(old, &mut item);
                *old = item;
            }
            None => {
                existing.insert(key, item.clone());
            }
        }
    }
}

fn replace(doc: &mut DocumentMut, key: &str, after: Option<&Item>) {
    let Some(item) = after else {
        doc.remove(key);
        return;
    };
    let mut item = item.clone();
    match doc.get_mut(key) {
        // Replacing in place keeps the key, and the comments above it, as written.
        Some(existing) => {
            keep_decor(existing, &mut item);
            *existing = item;
        }
        None => {
            doc.insert(key, item);
        }
    }
}

/// Both from a fresh serialization, so equal values print the same.
fn same(a: Option<&Item>, b: Option<&Item>) -> bool {
    a.map(Item::to_string) == b.map(Item::to_string)
}

/// Copies the comments around an existing table or value onto its replacement.
fn keep_decor(old: &Item, new: &mut Item) {
    if let (Some(old), Some(new)) = (old.as_table(), new.as_table_mut()) {
        *new.decor_mut() = old.decor().clone();
    }
    if let (Some(old), Some(new)) = (old.as_value(), new.as_value_mut()) {
        *new.decor_mut() = old.decor().clone();
    }
}

/// Replaces `old` with `new` in a folder's order, or drops it when `new` is `None`. Only
/// touches `folder.toml` when the name was listed.
pub fn rename_in_order(dir: &Path, old: &str, new: Option<&str>) -> Result<()> {
    let mut settings = read_settings(dir)?;
    let Some(position) = settings.order.iter().position(|n| n == old) else {
        return Ok(());
    };
    match new {
        Some(name) => settings.order[position] = name.to_string(),
        None => {
            settings.order.remove(position);
        }
    }
    write_settings(dir, &settings)
}

/// Inserts a name into a folder's order at `position` (end when `None`), if the folder has
/// an explicit order. Unordered folders stay alphabetical.
pub fn insert_in_order(dir: &Path, name: &str, position: Option<usize>) -> Result<()> {
    let mut settings = read_settings(dir)?;
    if settings.order.is_empty() && position.is_none() {
        return Ok(());
    }
    if settings.order.is_empty() {
        settings.order = child_names(dir)?;
    }
    settings.order.retain(|n| n != name);
    let at = position
        .unwrap_or(settings.order.len())
        .min(settings.order.len());
    settings.order.insert(at, name.to_string());
    write_settings(dir, &settings)
}

/// Names of a directory's children in their current display order.
pub fn child_names(dir: &Path) -> Result<Vec<String>> {
    let settings = read_settings(dir)?;
    let mut entries: Vec<(SortKey, ())> = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.path().is_dir();
        if (is_dir && !name.starts_with('.')) || name.ends_with(".http") {
            entries.push(((name, !is_dir), ()));
        }
    }
    sort_entries(&mut entries, &settings.order);
    Ok(entries.into_iter().map(|((name, _), _)| name).collect())
}

#[cfg(test)]
#[path = "folder_file_tests.rs"]
mod tests;
