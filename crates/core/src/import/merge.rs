//! Re-imports: finding the requests of an earlier import by their `# @source` markers and
//! updating them without undoing the user's own edits.

use std::collections::BTreeMap;
use std::path::Path;

use super::ImportSummary;
use super::write::SOURCE;
use crate::http_file::Request;
use crate::workspace::{self, TreeNode, resolve};
use crate::{Result, requests};

/// Separates the source key from the name the request was imported with.
const NAME_SEPARATOR: &str = " | ";

/// A request of an earlier import.
pub(super) struct Earlier {
    pub(super) id: String,
    /// Its name now (the user may have renamed it).
    name: String,
    /// The name it was imported with; `None` for markers written before names were recorded.
    imported_as: Option<String>,
}

/// Earlier imported requests by source key, in file order. Several requests can share a key
/// (`GET /items?page=1` and `?page=2`); they are told apart by name.
pub(super) type EarlierImport = BTreeMap<String, Vec<Earlier>>;

/// The earlier request for `key`: the one imported (or now named) as `name` first, otherwise
/// the first left in file order.
pub(super) fn take_earlier(existing: &mut EarlierImport, key: &str, name: &str) -> Option<Earlier> {
    let candidates = existing.get_mut(key)?;
    let named = candidates
        .iter()
        .position(|e| e.imported_as.as_deref().unwrap_or(&e.name) == name);
    let index = named.unwrap_or(0);
    let earlier = (index < candidates.len()).then(|| candidates.remove(index));
    if candidates.is_empty() {
        existing.remove(key);
    }
    earlier
}

/// Applies a source rename to an earlier request, unless the user renamed it since: then the
/// user's name stays and only the marker learns the new imported name.
pub(super) fn update_earlier(
    root: &Path,
    earlier: Earlier,
    key: &str,
    source_name: &str,
    summary: &mut ImportSummary,
) -> Result<String> {
    let imported_as = earlier.imported_as.as_deref();
    if imported_as == Some(source_name) {
        summary.unchanged += 1;
        return Ok(earlier.id);
    }
    // Older markers don't say what the source called it: never guess, so a user rename stays.
    let user_renamed = imported_as.is_none_or(|imported| imported != earlier.name);
    let current = requests::get(root, &earlier.id)?;
    let mut edited = current.clone();
    edited.comments.retain(|c| !c.starts_with(SOURCE));
    edited.comments.push(marker(key, source_name));
    if user_renamed {
        summary.unchanged += 1;
    } else {
        edited.name = source_name.to_string();
        summary.updated += 1;
    }
    requests::update(root, &earlier.id, edited, Some(&current.source))?;
    Ok(earlier.id)
}

/// The `# @source` marker for a request imported as `name`.
pub(super) fn marker(key: &str, name: &str) -> String {
    format!(
        "{SOURCE}{key}{NAME_SEPARATOR}{}",
        name.replace(NAME_SEPARATOR, " - ")
    )
}

/// The key and imported name in a marker comment (`GET /x | List`, or just `GET /x`).
fn parse_marker(comment: &str) -> Option<(String, Option<String>)> {
    let rest = comment.strip_prefix(SOURCE)?.trim();
    Some(match rest.rsplit_once(NAME_SEPARATOR) {
        Some((key, name)) => (key.trim().to_string(), Some(name.trim().to_string())),
        None => (rest.to_string(), None),
    })
}

/// `METHOD url-without-query`: stable across re-imports of the same spec, independent of
/// example values in the query string. An importer may set the marker itself (HAR keeps the
/// recorded URL there while the request uses `{{baseUrl}}`).
pub(super) fn source_key(request: &Request) -> String {
    if let Some((key, _)) = request.comments.iter().find_map(|c| parse_marker(c)) {
        return key;
    }
    let url = request.url.split('?').next().unwrap_or_default().trim();
    format!("{} {url}", request.method.to_uppercase())
}

/// Requests below `path` that came from an import, by source key.
pub(super) fn earlier_import(root: &Path, path: &str) -> Result<EarlierImport> {
    let mut found = EarlierImport::new();
    let dir = resolve(root, path)?;
    if !dir.is_dir() {
        return Ok(found);
    }
    let tree = workspace::tree(root)?;
    let Some(children) = find_folder(&tree, path) else {
        return Ok(found);
    };
    collect_sources(root, children, &mut found)?;
    Ok(found)
}

fn find_folder<'a>(nodes: &'a [TreeNode], path: &str) -> Option<&'a [TreeNode]> {
    nodes.iter().find_map(|node| match node {
        TreeNode::Folder {
            path: p, children, ..
        } if p == path => Some(children.as_slice()),
        TreeNode::Folder { children, .. } => find_folder(children, path),
        TreeNode::Request { .. } => None,
    })
}

fn collect_sources(root: &Path, nodes: &[TreeNode], found: &mut EarlierImport) -> Result<()> {
    for node in nodes {
        match node {
            TreeNode::Folder { children, .. } => collect_sources(root, children, found)?,
            TreeNode::Request { id, .. } => {
                let request = requests::get(root, id)?;
                if let Some((key, imported_as)) =
                    request.comments.iter().find_map(|c| parse_marker(c))
                {
                    found.entry(key).or_default().push(Earlier {
                        id: id.clone(),
                        name: request.name,
                        imported_as,
                    });
                }
            }
        }
    }
    Ok(())
}
