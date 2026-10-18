//! Folders in a workspace: plain directories plus an optional `folder.toml`.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::requests::IdChange;
use crate::workspace::{
    self, ENVIRONMENTS_DIR, FolderSettings, TreeNode, join_rel, resolve, split_rel,
};
use crate::{Error, Result, fs_util};

/// Files that only organize a folder; a folder holding nothing else counts as empty.
const ORGANIZING_FILE: &str = workspace::FOLDER_FILE;
const EXAMPLES_SUFFIX: &str = ".examples.json";

/// A renamed folder's new path, and the ids of the requests inside it that changed with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RenamedFolder {
    pub path: String,
    pub changes: Vec<IdChange>,
}

/// Creates `name` inside `parent` (`""` for the root). Writes an empty `folder.toml` so the
/// folder shows up before it has requests. Returns the new folder's path.
pub fn create(root: &Path, parent: &str, name: &str) -> Result<String> {
    let parent_dir = resolve(root, parent)?;
    let dir_name = visible_name(parent, name)?;
    let path = join_rel(parent, &dir_name);
    let dir = resolve(root, &path)?;
    if dir.exists() {
        return Err(Error::Invalid(format!("folder {path} already exists")));
    }
    workspace::check_settings(&parent_dir)?;
    fs::create_dir_all(&dir)?;
    workspace::write_settings(&dir, &FolderSettings::default())?;
    workspace::insert_in_order(&parent_dir, &dir_name, None)?;
    Ok(path)
}

/// Renames a folder in place.
pub fn rename(root: &Path, path: &str, name: &str) -> Result<RenamedFolder> {
    if path.is_empty() {
        return Err(Error::Invalid("the workspace root can't be renamed".into()));
    }
    let dir = existing_dir(root, path)?;
    let (parent, old_name) = split_rel(path);
    let new_name = visible_name(parent, name)?;
    let new_path = join_rel(parent, &new_name);
    let new_dir = resolve(root, &new_path)?;
    if new_dir.exists() {
        return Err(Error::Invalid(format!("folder {new_path} already exists")));
    }
    let ids = request_ids(root, path)?;
    let parent_dir = resolve(root, parent)?;
    workspace::check_settings(&parent_dir)?;
    fs::rename(&dir, &new_dir)?;
    workspace::rename_in_order(&parent_dir, old_name, Some(&new_name))?;
    let changes = ids
        .into_iter()
        .map(|id| IdChange {
            to: Some(format!("{new_path}{}", &id[path.len()..])),
            from: id,
        })
        .collect();
    Ok(RenamedFolder {
        path: new_path,
        changes,
    })
}

/// Copies a folder with everything in it (requests, `folder.toml`, saved examples) to
/// `<name>-copy` next to it. When the parent has an explicit order, the copy goes right after
/// the original. Returns the copy's path.
pub fn duplicate(root: &Path, path: &str) -> Result<String> {
    if path.is_empty() {
        return Err(Error::Invalid(
            "the workspace root can't be duplicated".into(),
        ));
    }
    let dir = existing_dir(root, path)?;
    let (parent, name) = split_rel(path);
    let parent_dir = resolve(root, parent)?;
    let settings = workspace::read_settings(&parent_dir)?;
    let copy_name = workspace::unique_name(&parent_dir, &format!("{name}-copy"), "");
    copy_dir(&root.canonicalize()?, &dir, &parent_dir.join(&copy_name))?;

    if let Some(at) = settings.order.iter().position(|n| n == name) {
        workspace::insert_in_order(&parent_dir, &copy_name, Some(at + 1))?;
    }
    Ok(join_rel(parent, &copy_name))
}

/// Copies files and real subfolders. A symlinked file is copied only when it points into the
/// workspace; symlinked folders are left out (they could loop or lead outside).
fn copy_dir(real_root: &Path, from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir(real_root, &source, &target)?;
            continue;
        }
        if file_type.is_symlink() {
            let inside = source
                .canonicalize()
                .is_ok_and(|real| real.is_file() && workspace::is_inside(&real, real_root));
            if !inside {
                continue;
            }
        }
        fs_util::write_atomic(&target, fs::read(&source)?)?;
    }
    Ok(())
}

/// Removes a folder. Without `recursive` it may only hold `folder.toml` and saved-example files
/// (in empty subfolders too): nothing else is ever deleted by accident, e.g. by an agent
/// reorganizing a workspace that lives in a repo. Hidden folders and the root `environments`
/// folder are never removed. Returns the ids of the requests that were removed.
pub fn remove(root: &Path, path: &str, recursive: bool) -> Result<Vec<IdChange>> {
    // Removing "" would delete the whole workspace, `.git` included.
    if path.is_empty() {
        return Err(Error::Invalid("the workspace root can't be removed".into()));
    }
    if path.split('/').any(|part| part.starts_with('.')) || path == ENVIRONMENTS_DIR {
        return Err(Error::Invalid(format!("{path} isn't a request folder")));
    }
    let dir = existing_dir(root, path)?;
    if !recursive {
        let mut kept = files_below(&dir, path)?;
        kept.retain(|file| !file.ends_with(EXAMPLES_SUFFIX));
        if !kept.is_empty() {
            let more = kept.len().saturating_sub(5);
            kept.truncate(5);
            let and_more = if more > 0 {
                format!(" and {more} more")
            } else {
                String::new()
            };
            return Err(Error::Invalid(format!(
                "folder {path} isn't empty ({}{and_more}); move or delete those first",
                kept.join(", ")
            )));
        }
    }
    let changes = request_ids(root, path)?
        .into_iter()
        .map(|id| IdChange { from: id, to: None })
        .collect();
    let (parent, name) = split_rel(path);
    let parent_dir = resolve(root, parent)?;
    workspace::check_settings(&parent_dir)?;
    fs::remove_dir_all(&dir)?;
    workspace::rename_in_order(&parent_dir, name, None)?;
    Ok(changes)
}

/// Every file below a folder except `folder.toml`, as sorted workspace-relative paths: what
/// removing the folder would delete.
pub fn content(root: &Path, path: &str) -> Result<Vec<String>> {
    files_below(&existing_dir(root, path)?, path)
}

fn files_below(dir: &Path, rel: &str) -> Result<Vec<String>> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        let child = join_rel(rel, &name);
        // Don't follow symlinked folders; a link itself counts as content.
        if entry.file_type()?.is_dir() {
            found.extend(files_below(&entry.path(), &child)?);
        } else if name != ORGANIZING_FILE {
            found.push(child);
        }
    }
    found.sort();
    Ok(found)
}

/// Ids of every request below a folder, as the tree shows them.
fn request_ids(root: &Path, path: &str) -> Result<Vec<String>> {
    fn collect(nodes: &[TreeNode], out: &mut Vec<String>) {
        for node in nodes {
            match node {
                TreeNode::Folder { children, .. } => collect(children, out),
                TreeNode::Request { id, .. } => out.push(id.clone()),
            }
        }
    }
    let prefix = format!("{path}/");
    let mut ids = Vec::new();
    collect(&workspace::tree(root)?, &mut ids);
    ids.retain(|id| id.starts_with(&prefix));
    Ok(ids)
}

/// Sets the display order of a folder's children by file or folder name. Names not listed
/// keep their default place after the listed ones.
pub fn reorder(root: &Path, path: &str, order: Vec<String>) -> Result<()> {
    let dir = existing_dir(root, path)?;
    let children = workspace::child_names(&dir)?;
    if let Some(unknown) = order.iter().find(|name| !children.contains(name)) {
        return Err(Error::Invalid(format!("{unknown} is not in folder {path}")));
    }
    let settings = FolderSettings {
        order,
        ..workspace::read_settings(&dir)?
    };
    workspace::write_settings(&dir, &settings)
}

pub fn get_settings(root: &Path, path: &str) -> Result<FolderSettings> {
    workspace::read_settings(&existing_dir(root, path)?)
}

pub fn update_settings(root: &Path, path: &str, settings: FolderSettings) -> Result<()> {
    workspace::write_settings(&existing_dir(root, path)?, &settings)
}

/// A change to a folder's shared headers: `set` adds or changes, `remove` drops by name, and
/// `replace` starts from no headers instead of the current ones.
#[derive(Debug, Clone, Default)]
pub struct FolderHeaderChange {
    pub set: BTreeMap<String, String>,
    pub remove: Vec<String>,
    pub replace: bool,
}

/// Applies `change` to the folder's shared headers in `folder.toml`. Returns the headers after.
pub fn set_headers(
    root: &Path,
    path: &str,
    change: &FolderHeaderChange,
) -> Result<BTreeMap<String, String>> {
    let mut settings = get_settings(root, path)?;
    settings.headers = merged_headers(settings.headers, change);
    let headers = settings.headers.clone();
    update_settings(root, path, settings)?;
    Ok(headers)
}

/// The headers after `change`. Names compare case-insensitively: setting `accept` replaces an
/// existing `Accept`.
pub fn merged_headers(
    current: BTreeMap<String, String>,
    change: &FolderHeaderChange,
) -> BTreeMap<String, String> {
    let mut headers = if change.replace {
        BTreeMap::new()
    } else {
        current
    };
    for (name, value) in &change.set {
        headers.retain(|k, _| !k.eq_ignore_ascii_case(name));
        headers.insert(name.clone(), value.clone());
    }
    headers.retain(|k, _| !change.remove.iter().any(|r| r.eq_ignore_ascii_case(k)));
    headers
}

/// The folder name for `name` in `parent`, refused when the tree would hide it.
fn visible_name(parent: &str, name: &str) -> Result<String> {
    let dir_name = workspace::slug(name);
    if workspace::is_hidden_dir(&dir_name, parent) {
        return Err(Error::Invalid(format!(
            "a top-level folder named {dir_name} is hidden in the app; pick another name"
        )));
    }
    Ok(dir_name)
}

fn existing_dir(root: &Path, path: &str) -> Result<std::path::PathBuf> {
    let dir = resolve(root, path)?;
    if !dir.is_dir() {
        return Err(Error::NotFound(format!("folder {path}")));
    }
    Ok(dir)
}

#[cfg(test)]
#[path = "folders_tests.rs"]
mod tests;
