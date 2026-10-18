//! Writes an [`ImportedCollection`] as `.http` files, and updates an earlier import in place:
//! see `docs/formats.md` (Re-importing).

mod environment;
mod prepare;

use std::collections::BTreeMap;
use std::path::Path;

use super::merge::{
    EarlierImport, earlier_import, marker, source_key, take_earlier, update_earlier,
};
use super::{
    ImportFormat, ImportSummary, ImportedCollection, ImportedEnvironment, ImportedItem, credentials,
};
use crate::http_file::{self, Request};
use crate::workspace::{self, FolderSettings, join_rel, resolve, split_rel};
use crate::{Error, Result, folders, requests};
pub use environment::ImportEnvironmentChange;
use prepare::prepare;

pub(super) const SOURCE: &str = "# @source ";
/// Notes and stale ids in a summary; the rest is counted in one last note, so a huge import
/// doesn't flood the UI or the AI.
const MAX_LISTED: usize = 20;

/// Writes a collection into a folder named after it inside `folder`, plus its environments at
/// the workspace root. An earlier import of the same collection is updated, never duplicated.
pub fn write(
    root: &Path,
    folder: &str,
    mut collection: ImportedCollection,
) -> Result<ImportSummary> {
    scrub_credentials(&mut collection);
    prepare(&mut collection.items, &mut collection.notes)?;
    let parent_dir = resolve(root, folder)?;
    if !parent_dir.is_dir() {
        return Err(Error::NotFound(format!("folder {folder}")));
    }
    let base = folder_name(folder, &collection.name);
    let candidate = join_rel(folder, &base);
    let mut existing = earlier_import(root, &candidate)?;
    let merged = !existing.is_empty();

    let target = match merged {
        true => candidate,
        false => {
            let name = workspace::unique_name(&parent_dir, &base, "");
            folders::create(root, folder, &name)?
        }
    };
    let mut summary = empty_summary(collection.format, target, merged, collection.notes);
    let order = write_items(
        root,
        &summary.folder.clone(),
        collection.items,
        &mut existing,
        &mut summary,
    )?;
    if !merged {
        let settings = FolderSettings {
            order,
            headers: collection.headers,
            ..FolderSettings::default()
        };
        folders::update_settings(root, &summary.folder, settings)?;
    }
    summary.stale = existing.into_values().flatten().map(|e| e.id).collect();
    summary.stale.sort();

    let changes = environment::plan(root, collection.environments, merged, &mut summary.notes)?;
    for change in changes {
        summary.environments.push(environment::apply(root, change)?);
    }
    cap_lists(&mut summary);
    Ok(summary)
}

/// What [`write`] would do to the workspace's environments, without writing anything: for the
/// approval of an agent's import.
pub fn environment_changes(
    root: &Path,
    folder: &str,
    collection: &ImportedCollection,
) -> Result<Vec<ImportEnvironmentChange>> {
    let mut collection = collection.clone();
    scrub_credentials(&mut collection);
    let candidate = join_rel(folder, &folder_name(folder, &collection.name));
    let merged = !earlier_import(root, &candidate)?.is_empty();
    environment::plan(root, collection.environments, merged, &mut Vec::new())
}

/// The collection's name as a folder name; one the tree would hide at the top (`build`,
/// `environments`) gets `-requests` added, the same each time so a re-import finds it.
fn folder_name(parent: &str, collection: &str) -> String {
    let name = workspace::slug(collection);
    match workspace::is_hidden_dir(&name, parent) {
        true => format!("{name}-requests"),
        false => name,
    }
}

fn cap_lists(summary: &mut ImportSummary) {
    let more_stale = summary.stale.len().saturating_sub(MAX_LISTED);
    summary.stale.truncate(MAX_LISTED);
    let more_notes = summary.notes.len().saturating_sub(MAX_LISTED);
    summary.notes.truncate(MAX_LISTED);
    if more_notes > 0 {
        summary.notes.push(format!("and {more_notes} more notes"));
    }
    if more_stale > 0 {
        summary.notes.push(format!(
            "{more_stale} more requests are no longer in the source"
        ));
    }
}

fn empty_summary(
    format: ImportFormat,
    folder: String,
    merged: bool,
    notes: Vec<String>,
) -> ImportSummary {
    ImportSummary {
        format,
        folder,
        merged,
        requests: 0,
        added: 0,
        updated: 0,
        unchanged: 0,
        stale: Vec::new(),
        folders: 0,
        environments: Vec::new(),
        notes,
    }
}

/// Replaces literal credentials in headers with `{{secret}}` placeholders and adds the secret
/// names to every environment of the import (one named after the collection if it has none).
fn scrub_credentials(collection: &mut ImportedCollection) {
    let declared = std::mem::take(&mut collection.credentials);
    let mut found = credentials::Found::with_declared(declared);
    credentials::headers(&mut collection.headers, &mut found);
    scrub_items(&mut collection.items, &mut found);
    if found.is_empty() {
        return;
    }
    let names = found.names();
    if collection.environments.is_empty() {
        collection.environments.push(ImportedEnvironment {
            name: collection.name.clone(),
            variables: BTreeMap::new(),
            secrets: Vec::new(),
        });
    }
    for env in &mut collection.environments {
        for name in &names {
            if !env.secrets.contains(name) {
                env.secrets.push(name.clone());
            }
        }
    }
    collection.notes.push(format!(
        "kept credentials out of the files: they are now the secrets {}; set their values in \
         the app",
        names.join(", ")
    ));
}

fn scrub_items(items: &mut [ImportedItem], found: &mut credentials::Found) {
    for item in items {
        match item {
            ImportedItem::Request(request) => credentials::request(request, found),
            ImportedItem::Folder { headers, items, .. } => {
                credentials::headers(headers, found);
                scrub_items(items, found);
            }
        }
    }
}

/// Writes items into `path` and returns the file/folder names in source order.
fn write_items(
    root: &Path,
    path: &str,
    items: Vec<ImportedItem>,
    existing: &mut EarlierImport,
    summary: &mut ImportSummary,
) -> Result<Vec<String>> {
    let mut order = Vec::new();
    for item in items {
        let child = match item {
            ImportedItem::Request(request) => {
                summary.requests += 1;
                write_request(root, path, request, existing, summary)?
            }
            ImportedItem::Folder {
                name,
                headers,
                items,
            } => {
                let sub = join_rel(path, &workspace::slug(&name));
                let created = !resolve(root, &sub)?.is_dir();
                if created {
                    summary.folders += 1;
                    folders::create(root, path, &name)?;
                }
                let sub_order = write_items(root, &sub, items, existing, summary)?;
                if created {
                    let settings = FolderSettings {
                        order: sub_order,
                        headers,
                        ..FolderSettings::default()
                    };
                    folders::update_settings(root, &sub, settings)?;
                }
                sub
            }
        };
        order.push(split_rel(&child).1.to_string());
    }
    Ok(order)
}

fn write_request(
    root: &Path,
    path: &str,
    mut request: Request,
    existing: &mut EarlierImport,
    summary: &mut ImportSummary,
) -> Result<String> {
    let key = source_key(&request);
    for warning in http_file::body_warnings(&request) {
        summary.notes.push(format!("{}: {warning}", request.name));
    }
    if let Some(earlier) = take_earlier(existing, &key, &request.name) {
        return update_earlier(root, earlier, &key, &request.name, summary);
    }
    request.comments.retain(|c| !c.starts_with(SOURCE));
    request.comments.push(marker(&key, &request.name));
    summary.added += 1;
    requests::create(root, path, request)
}

#[cfg(test)]
mod prepare_tests;
#[cfg(test)]
mod tests;
