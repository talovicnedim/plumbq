//! A workspace is any folder: directories are folders, `.http` files hold requests. Ids are
//! workspace-relative paths plus `#<index>` for later requests in a file (`legacy/all.http#2`).

mod name;
mod tree;

use std::path::{Component, Path, PathBuf};

pub use crate::folder_file::{
    FolderSettings, check_settings, child_names, insert_in_order, read_settings, rename_in_order,
    write_settings,
};
use crate::{Error, Result, fs_util};
pub use name::{WorkspaceSummary, name, rename, summary};
pub(crate) use tree::{SortKey, sort_entries};
pub use tree::{TreeNode, WorkspaceWarning, tree, tree_with_warnings};

pub const FOLDER_FILE: &str = "folder.toml";
/// Root-level directory holding environment files; never shown as a folder.
pub const ENVIRONMENTS_DIR: &str = "environments";
/// Root folders the tree never shows: environments, and dependency and build folders that
/// never hold requests but can be huge when a repo root is opened.
const HIDDEN_ROOT_DIRS: [&str; 5] = [
    ENVIRONMENTS_DIR,
    "node_modules",
    "target",
    "vendor",
    "build",
];

/// Creates a new workspace folder `name` inside `parent`, with a `local` environment and an
/// example request so there is something to send right away. Returns the new root.
pub fn create(parent: &Path, name: &str) -> Result<PathBuf> {
    let name = name.trim();
    let valid = !name.is_empty() && !name.contains(['/', '\\']) && name != "." && name != "..";
    if !valid {
        return Err(Error::Invalid(format!("invalid workspace name {name}")));
    }
    if !parent.is_dir() {
        return Err(Error::NotFound(format!("folder {}", parent.display())));
    }
    let root = parent.join(name);
    if root.exists() {
        return Err(Error::Invalid(format!("{} already exists", root.display())));
    }
    fs_util::write_atomic(
        &root.join(ENVIRONMENTS_DIR).join("local.toml"),
        "tag = \"local\"\n\n[variables]\nbaseUrl = \"http://localhost:8000\"\n",
    )?;
    fs_util::write_atomic(
        &root.join("example.http"),
        "### Example\nGET {{baseUrl}}/\n",
    )?;
    Ok(root)
}

/// The path that identifies a workspace: absolute and with symlinks resolved, so the desktop
/// app, MCP server and CLI agree on it. Keychain secrets and history are keyed by it. On Windows
/// the `\\?\` prefix `canonicalize` adds is dropped, because the app's file picker never has it.
pub fn canonical_root(path: &Path) -> Result<PathBuf> {
    let canonical = path
        .canonicalize()
        .map_err(|_| Error::NotFound(format!("workspace {}", path.display())))?;
    Ok(strip_verbatim(canonical))
}

fn strip_verbatim(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path,
    }
}

/// The id of the request at `index` (from 0) in `rel_path`: `a.http`, then `a.http#2`, ….
pub fn request_id(rel_path: &str, index: usize) -> String {
    match index {
        0 => rel_path.to_string(),
        _ => format!("{rel_path}#{}", index + 1),
    }
}

/// Splits an id into its relative file path and request index.
pub fn parse_request_id(id: &str) -> Result<(&str, usize)> {
    let Some((path, number)) = id.rsplit_once('#') else {
        return Ok((id, 0));
    };
    match number.parse::<usize>() {
        Ok(n) if n >= 2 => Ok((path, n - 1)),
        _ => Err(Error::Invalid(format!("invalid request id {id}"))),
    }
}

/// Resolves a workspace-relative path (`/`-separated, `""` for the root), rejecting anything
/// that could leave the workspace: `..` or absolute parts, and symlinks (the path itself or a
/// folder above it) that point outside. A path that doesn't exist yet is checked through its
/// deepest existing folder.
pub fn resolve(root: &Path, rel: &str) -> Result<PathBuf> {
    let rel_path = Path::new(rel);
    let safe = rel_path
        .components()
        .all(|c| matches!(c, Component::Normal(_)));
    if !safe {
        return Err(Error::Invalid(format!(
            "path {rel} is outside the workspace"
        )));
    }
    // Ids are built from these paths as text, so `users/` would make `users//list.http`.
    if !rel.is_empty() && rel.split('/').any(str::is_empty) {
        return Err(Error::Invalid(format!(
            "invalid path {rel}: remove the extra /"
        )));
    }
    let path = root.join(rel_path);
    // Without the root on disk nothing below it exists, so no link can lead out.
    let Ok(real_root) = root.canonicalize() else {
        return Ok(path);
    };
    let existing = path.ancestors().find(|p| p.exists()).unwrap_or(root);
    if !is_inside(&existing.canonicalize()?, &real_root) {
        return Err(Error::Invalid(format!(
            "{rel} goes through a link to outside the workspace"
        )));
    }
    Ok(path)
}

/// Like [`resolve`], for reading an existing file whose content may be sent somewhere (body
/// files, `.env`, certificates), so a shared repo can't point `data.json` at
/// `~/.aws/credentials`.
pub fn resolve_existing(root: &Path, rel: &str) -> Result<PathBuf> {
    let path = resolve(root, rel)?;
    if !path.exists() {
        return Err(Error::NotFound(format!("file {rel}")));
    }
    Ok(path)
}

/// Whether a real (canonical) path lies in the real workspace root.
pub(crate) fn is_inside(real: &Path, real_root: &Path) -> bool {
    strip_verbatim(real.to_path_buf()).starts_with(strip_verbatim(real_root.to_path_buf()))
}

/// Hidden folders anywhere; [`HIDDEN_ROOT_DIRS`] only at the root, where a repo keeps them (an
/// imported "Build" folder deeper down is a real folder). The tree skips these, so they are
/// never created either.
pub(crate) fn is_hidden_dir(name: &str, parent_rel: &str) -> bool {
    name.starts_with('.') || (parent_rel.is_empty() && HIDDEN_ROOT_DIRS.contains(&name))
}

pub fn join_rel(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    }
}

/// Splits `a/b/c.http` into (`a/b`, `c.http`).
pub fn split_rel(rel: &str) -> (&str, &str) {
    rel.rsplit_once('/').unwrap_or(("", rel))
}

/// Turns a request or folder name into a file-system friendly slug: `Create user` → `create-user`.
pub fn slug(name: &str) -> String {
    let mut slug = String::new();
    for c in name.trim().chars() {
        if c.is_alphanumeric() {
            slug.extend(c.to_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-').to_string();
    if slug.is_empty() {
        "untitled".to_string()
    } else {
        slug
    }
}

/// `base` + `ext` in `dir`, adding `-2`, `-3`, … until the name is free.
pub fn unique_name(dir: &Path, base: &str, ext: &str) -> String {
    let mut name = format!("{base}{ext}");
    let mut n = 2;
    while dir.join(&name).exists() {
        name = format!("{base}-{n}{ext}");
        n += 1;
    }
    name
}

#[cfg(test)]
pub(crate) mod tests;
