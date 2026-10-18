//! Exports a workspace for other tools and readers: a Postman v2.1 collection (plus Postman
//! environment files) and a self-contained HTML documentation page.

pub mod docs;
pub mod postman;

use std::path::Path;

use crate::http_file::Header;
use crate::workspace::{self, TreeNode};
use crate::{Error, Result};

/// The children of the folder at `path` (`""` for the whole workspace).
fn subtree(root: &Path, path: &str) -> Result<Vec<TreeNode>> {
    let tree = workspace::tree(root)?;
    if path.is_empty() {
        return Ok(tree);
    }
    find(&tree, path).ok_or_else(|| Error::NotFound(format!("folder {path}")))
}

fn find(nodes: &[TreeNode], path: &str) -> Option<Vec<TreeNode>> {
    nodes.iter().find_map(|node| match node {
        TreeNode::Folder {
            path: p, children, ..
        } if p == path => Some(children.clone()),
        TreeNode::Folder { children, .. } => find(children, path),
        TreeNode::Request { .. } => None,
    })
}

/// A request's headers with the shared `folder.toml` headers of its folders folded in, the way
/// `send` merges them: closer folders override, the request's own headers win.
fn effective_headers(root: &Path, id: &str, own: &[Header]) -> Result<Vec<Header>> {
    let (rel, _) = workspace::parse_request_id(id)?;
    let (folder, _) = workspace::split_rel(rel);
    let mut dirs = vec![String::new()];
    for part in folder.split('/').filter(|p| !p.is_empty()) {
        let next = workspace::join_rel(dirs.last().map_or("", String::as_str), part);
        dirs.push(next);
    }
    let mut headers: Vec<Header> = Vec::new();
    let shared = dirs
        .iter()
        .map(|dir| workspace::read_settings(&workspace::resolve(root, dir)?).map(|s| s.headers));
    for settings in shared {
        for (name, value) in settings? {
            push_header(&mut headers, Header { name, value });
        }
    }
    for header in own {
        push_header(&mut headers, header.clone());
    }
    Ok(headers)
}

fn push_header(headers: &mut Vec<Header>, header: Header) {
    headers.retain(|h| !h.name.eq_ignore_ascii_case(&header.name));
    headers.push(header);
}

#[cfg(test)]
mod tests;
