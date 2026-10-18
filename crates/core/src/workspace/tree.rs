//! The workspace tree in display order. Hidden and build folders are skipped; unreadable files
//! become a [`WorkspaceWarning`] instead of breaking the tree.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::workspace::{
    FOLDER_FILE, FolderSettings, is_hidden_dir, is_inside, join_rel, read_settings, request_id,
};
use crate::{Error, Result, http_file, request_meta};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TreeNode {
    #[serde(rename_all = "camelCase")]
    Folder {
        name: String,
        /// Workspace-relative path, `/`-separated.
        path: String,
        children: Vec<TreeNode>,
    },
    #[serde(rename_all = "camelCase")]
    Request {
        id: String,
        name: String,
        method: String,
        url: String,
        /// From `# @tag` lines (see [`crate::request_meta`]).
        tags: Vec<String>,
    },
}

/// The folder tree under `root`. Directories without any requests below them are hidden,
/// unless they have a `folder.toml` (so new, empty folders show up). Unreadable parts are left
/// out and logged; [`tree_with_warnings`] returns them.
pub fn tree(root: &Path) -> Result<Vec<TreeNode>> {
    let (nodes, warnings) = tree_with_warnings(root)?;
    for warning in warnings {
        tracing::warn!("{}: {}", warning.path, warning.message);
    }
    Ok(nodes)
}

/// A part of the workspace the tree had to leave out or read with defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceWarning {
    /// Workspace-relative path of the file or folder.
    pub path: String,
    pub message: String,
}

/// Like [`tree`], plus what was skipped: a file that isn't UTF-8, a folder that can't be read,
/// a symlinked folder (not followed, so links can't loop or leave the workspace), a request
/// file linking outside the workspace, a name with `#` (ids use it for the request number), or
/// a broken `folder.toml` (its folder is shown with default order).
pub fn tree_with_warnings(root: &Path) -> Result<(Vec<TreeNode>, Vec<WorkspaceWarning>)> {
    if !root.is_dir() {
        return Err(Error::NotFound(format!("workspace {}", root.display())));
    }
    let mut walk = Walk {
        real_root: root.canonicalize()?,
        warnings: Vec::new(),
    };
    let nodes = read_dir_nodes(&mut walk, root, "")?;
    Ok((nodes, walk.warnings))
}

/// What the walk through the workspace carries along.
struct Walk {
    real_root: PathBuf,
    warnings: Vec<WorkspaceWarning>,
}

impl Walk {
    fn warn(&mut self, path: &str, message: &str) {
        self.warnings.push(WorkspaceWarning {
            path: path.to_string(),
            message: message.to_string(),
        });
    }
}

fn read_dir_nodes(walk: &mut Walk, dir: &Path, rel: &str) -> Result<Vec<TreeNode>> {
    let settings = match read_settings(dir) {
        Ok(settings) => settings,
        Err(err) => {
            walk.warn(&join_rel(rel, FOLDER_FILE), &err.to_string());
            FolderSettings::default()
        }
    };
    let mut entries: Vec<(SortKey, TreeNode)> = Vec::new();
    for entry in fs::read_dir(dir)?.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        let child_rel = join_rel(rel, &name);
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if name.contains('#') && (path.is_dir() || name.ends_with(".http")) {
            walk.warn(
                &child_rel,
                "has # in its name, which request ids use; rename it",
            );
            continue;
        }
        if path.is_dir() {
            let folder = folder_node(walk, &path, rel, child_rel, file_type)?;
            entries.extend(folder.map(|node| ((name, false), node)));
        } else if name.ends_with(".http") {
            if file_type.is_symlink() && !links_inside(&path, &walk.real_root) {
                walk.warn(
                    &child_rel,
                    "links to a file outside the workspace, not shown",
                );
                continue;
            }
            match file_nodes(&path, &child_rel) {
                Ok(nodes) => entries.extend(nodes.into_iter().map(|n| ((name.clone(), true), n))),
                Err(err) => walk.warn(&child_rel, &format!("can't be read: {err}")),
            }
        }
    }
    sort_entries(&mut entries, &settings.order);
    Ok(entries.into_iter().map(|(_, node)| node).collect())
}

fn links_inside(path: &Path, real_root: &Path) -> bool {
    path.canonicalize()
        .is_ok_and(|real| is_inside(&real, real_root))
}

/// A subfolder and everything under it; `None` when it is hidden, a symlink, unreadable, or
/// empty without its own `folder.toml`.
fn folder_node(
    walk: &mut Walk,
    path: &Path,
    rel: &str,
    child_rel: String,
    file_type: fs::FileType,
) -> Result<Option<TreeNode>> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if is_hidden_dir(&name, rel) {
        return Ok(None);
    }
    if file_type.is_symlink() {
        walk.warn(&child_rel, "symlinked folder, not shown");
        return Ok(None);
    }
    if let Err(err) = fs::read_dir(path) {
        walk.warn(&child_rel, &format!("can't be read: {err}"));
        return Ok(None);
    }
    let children = read_dir_nodes(walk, path, &child_rel)?;
    if children.is_empty() && !path.join(FOLDER_FILE).exists() {
        return Ok(None);
    }
    Ok(Some(TreeNode::Folder {
        name,
        path: child_rel,
        children,
    }))
}

fn file_nodes(path: &Path, rel: &str) -> Result<Vec<TreeNode>> {
    let requests = http_file::parse(&fs::read_to_string(path)?);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let nodes = requests
        .into_iter()
        .enumerate()
        .map(|(index, request)| TreeNode::Request {
            id: request_id(rel, index),
            name: display_name(&request.name, &stem, index),
            tags: request_meta::tags(&request),
            method: request.method,
            url: request.url,
        })
        .collect();
    Ok(nodes)
}

fn display_name(name: &str, stem: &str, index: usize) -> String {
    match (name.is_empty(), index) {
        (false, _) => name.to_string(),
        (true, 0) => stem.to_string(),
        (true, _) => format!("{stem} #{}", index + 1),
    }
}

/// A directory entry's name and whether it is a file.
pub(crate) type SortKey = (String, bool);

/// Listed names first in their listed order, then folders, then the rest alphabetically.
/// The sort is stable, so requests of one multi-request file keep their file order.
pub(crate) fn sort_entries<T>(entries: &mut [(SortKey, T)], order: &[String]) {
    entries.sort_by_key(|((name, is_file), _)| {
        let position = order.iter().position(|o| o == name).unwrap_or(usize::MAX);
        (position, *is_file, natural_key(name))
    });
}

/// A sort key that orders digit runs by value, so `request 2` comes before `request 10`.
fn natural_key(name: &str) -> Vec<(String, u64)> {
    let mut key = Vec::new();
    let mut text = String::new();
    let mut chars = name.chars().peekable();
    while let Some(c) = chars.next() {
        if !c.is_ascii_digit() {
            text.extend(c.to_lowercase());
            continue;
        }
        let mut digits = c.to_string();
        while let Some(d) = chars.next_if(char::is_ascii_digit) {
            digits.push(d);
        }
        // Absurdly long numbers saturate; they only need to sort after normal ones.
        key.push((
            std::mem::take(&mut text),
            digits.parse().unwrap_or(u64::MAX),
        ));
    }
    key.push((text, 0));
    key
}
