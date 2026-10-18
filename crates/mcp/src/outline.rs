//! Compact text about the workspace's structure: the tree, search results, folder settings and
//! request previews. Kept short for agents: big workspaces are capped with a hint.

use plumbq_core::send::SentRequest;
use plumbq_core::workspace::{FolderSettings, TreeNode};

use crate::format;

/// The tree is cut after this many lines; pass `folder` or `depth` to see more of a part.
pub const MAX_TREE_LINES: usize = 300;

/// An indented outline: folders with their path, requests with id, method and URL. Folders
/// below `depth` are summarized as a request count.
pub fn tree(nodes: &[TreeNode], depth: Option<u32>) -> String {
    let mut lines = Vec::new();
    write_tree(&mut lines, nodes, 0, depth);
    if lines.is_empty() {
        return "(no requests here yet)\n".into();
    }
    let total = lines.len();
    let mut out: String = lines
        .into_iter()
        .take(MAX_TREE_LINES)
        .map(|line| line + "\n")
        .collect();
    if total > MAX_TREE_LINES {
        out.push_str(&format!(
            "… {} more lines. Pass `folder` and/or `depth`, or use requests_search.\n",
            total - MAX_TREE_LINES
        ));
    }
    out
}

fn write_tree(out: &mut Vec<String>, nodes: &[TreeNode], level: u32, depth: Option<u32>) {
    let indent = "  ".repeat(level as usize);
    for node in nodes {
        match node {
            TreeNode::Folder {
                name,
                path,
                children,
            } => {
                if depth.is_some_and(|d| level >= d) {
                    out.push(format!(
                        "{indent}📁 {name}/  (folder path: {path}, {} requests)",
                        count_requests(children)
                    ));
                    continue;
                }
                out.push(format!("{indent}📁 {name}/  (folder path: {path})"));
                write_tree(out, children, level + 1, depth);
            }
            TreeNode::Request { .. } => out.push(format!("{indent}{}", request_line(node))),
        }
    }
}

fn request_line(node: &TreeNode) -> String {
    let TreeNode::Request {
        id,
        name,
        method,
        url,
        tags,
    } = node
    else {
        return String::new();
    };
    let tags = match tags.is_empty() {
        true => String::new(),
        false => format!("  #{}", tags.join(" #")),
    };
    format!("{method} {name}  [id: {id}]  {url}{tags}")
}

fn count_requests(nodes: &[TreeNode]) -> usize {
    nodes
        .iter()
        .map(|node| match node {
            TreeNode::Folder { children, .. } => count_requests(children),
            TreeNode::Request { .. } => 1,
        })
        .sum()
}

/// The children of the folder at `path` (`""` for the whole tree).
pub fn subtree<'a>(nodes: &'a [TreeNode], path: &str) -> Option<&'a [TreeNode]> {
    if path.is_empty() {
        return Some(nodes);
    }
    nodes.iter().find_map(|node| match node {
        TreeNode::Folder {
            path: p, children, ..
        } if p == path => Some(children.as_slice()),
        TreeNode::Folder { children, .. } => subtree(children, path),
        TreeNode::Request { .. } => None,
    })
}

pub fn search_results(found: &[&TreeNode], max: usize) -> String {
    if found.is_empty() {
        return "No matching requests.\n".into();
    }
    let lines: Vec<String> = found.iter().map(|node| request_line(node)).collect();
    capped_lines(&lines, max)
}

/// Lines, cut after `max` with a count of the rest.
pub fn capped_lines(lines: &[String], max: usize) -> String {
    let mut out: String = lines.iter().take(max).map(|l| format!("{l}\n")).collect();
    if lines.len() > max {
        out.push_str(&format!("… and {} more\n", lines.len() - max));
    }
    out
}

pub fn moves(done: &[(String, String)]) -> String {
    if done.is_empty() {
        return "(nothing moved)\n".into();
    }
    done.iter()
        .map(|(from, to)| format!("{from} -> {to}\n"))
        .collect()
}

pub fn folder(path: &str, settings: &FolderSettings) -> String {
    let label = if path.is_empty() {
        "workspace root"
    } else {
        path
    };
    let mut out = format!("Folder {label}\n");
    out.push_str("Shared headers:\n");
    out.push_str(&pairs(settings.headers.iter()));
    if !settings.order.is_empty() {
        out.push_str(&format!("Order: {}\n", settings.order.join(", ")));
    }
    if path.is_empty() {
        out.push_str("Variable defaults (every environment, which can override them):\n");
        out.push_str(&pairs(settings.variables.iter()));
        if let Ok(http) = toml::to_string(&settings.settings)
            && !http.trim().is_empty()
        {
            out.push_str(&format!("HTTP settings:\n{http}"));
        }
    }
    out
}

pub fn headers_saved(headers: &std::collections::BTreeMap<String, String>) -> String {
    format!("Folder headers saved. Now:\n{}", pairs(headers.iter()))
}

fn pairs<'a>(items: impl Iterator<Item = (&'a String, &'a String)>) -> String {
    let lines: String = items.map(|(k, v)| format!("  {k}: {v}\n")).collect();
    if lines.is_empty() {
        "  (none)\n".into()
    } else {
        lines
    }
}

/// What a request would send. `prepared` is an error when it can't be prepared (e.g. unknown
/// variables); `verdict` is whether this server may send it.
pub fn preview(
    prepared: &Result<SentRequest, String>,
    prompts: &[String],
    verdict: &Result<(), String>,
) -> String {
    let mut out = match prepared {
        Ok(sent) => {
            let mut text = format!("{} {}\n", sent.method, sent.url);
            for header in &sent.headers {
                text.push_str(&format!("{}: {}\n", header.name, header.value));
            }
            if !sent.body.is_empty() {
                text.push('\n');
                text.push_str(&format::cap(&sent.body));
            }
            text
        }
        Err(problem) => format!("Can't be sent as is: {problem}\n"),
    };
    if !prompts.is_empty() {
        out.push_str(&format!(
            "\nAsks for: {} (pass them in `prompts`)\n",
            prompts.join(", ")
        ));
    }
    match verdict {
        Ok(()) => out.push_str("\nThis server may send it.\n"),
        Err(reason) => out.push_str(&format!("\nThis server won't send it: {reason}\n")),
    }
    out
}

#[cfg(test)]
#[path = "outline_tests.rs"]
mod tests;
