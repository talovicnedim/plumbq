use std::path::Path;

use serde::Serialize;

use super::KEEP_OUTCOMES;
use crate::environments::{self, EnvironmentTag};
use crate::http_file::Request;
use crate::send;
use crate::workspace::{self, TreeNode};
use crate::{Result, request_meta, requests};

/// A request picked for a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selected {
    pub id: String,
    pub name: String,
    pub method: String,
}

/// What a run would need before it starts, for the app's Runner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RunPreview {
    /// Requests sent per iteration.
    pub requests: u32,
    /// `{{$prompt}}` names to ask for first.
    pub prompts: Vec<String>,
    /// Requests that would change data in the environment, a production one. They are skipped
    /// unless the user types its name.
    pub production_writes: u32,
    /// Outcomes a report keeps; later ones are only counted.
    pub keep_outcomes: u32,
}

/// Requests under `target` (`""` for the whole workspace, a folder path, or a request id) in
/// display order, keeping only those with one of `tags` when given.
pub fn select(root: &Path, target: &str, tags: &[String]) -> Result<Vec<Selected>> {
    let mut all = Vec::new();
    flatten(&workspace::tree(root)?, &mut all);
    let in_target = |id: &str| {
        target.is_empty()
            || id == target
            || id.starts_with(&format!("{target}#"))
            || id.starts_with(&format!("{target}/"))
    };
    let mut selected = Vec::new();
    for item in all.into_iter().filter(|s| in_target(&s.id)) {
        if !tags.is_empty() {
            let request = requests::get(root, &item.id)?;
            let own = request_meta::tags(&request);
            if !own
                .iter()
                .any(|t| tags.iter().any(|w| w.eq_ignore_ascii_case(t)))
            {
                continue;
            }
        }
        selected.push(item);
    }
    Ok(selected)
}

/// Names of the `{{$prompt name}}` variables used anywhere in `selection` (folder headers
/// included), each once in first-use order, so a client can ask for all of them before a run.
pub fn prompts_needed(root: &Path, selection: &[Selected]) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for item in selection {
        add_prompts(root, item, &requests::get(root, &item.id)?, &mut names)?;
    }
    Ok(names)
}

/// The run of [`select`] with `environment`, without sending anything. Reads each request
/// once.
pub fn preview(
    root: &Path,
    target: &str,
    tags: &[String],
    environment: Option<&str>,
) -> Result<RunPreview> {
    let selection = select(root, target, tags)?;
    let production = match environment {
        Some(env) => environments::tag(root, env)? == EnvironmentTag::Production,
        None => false,
    };
    let mut prompts = Vec::new();
    let mut production_writes = 0;
    for item in &selection {
        let request = requests::get(root, &item.id)?;
        add_prompts(root, item, &request, &mut prompts)?;
        if production && !is_safe(root, item, &request)? {
            production_writes += 1;
        }
    }
    Ok(RunPreview {
        requests: count(selection.len()),
        prompts,
        production_writes,
        keep_outcomes: count(KEEP_OUTCOMES),
    })
}

fn add_prompts(
    root: &Path,
    item: &Selected,
    request: &Request,
    names: &mut Vec<String>,
) -> Result<()> {
    for name in send::prompts_needed(root, &item.id, request)? {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    Ok(())
}

/// The method the server acts on is GET, HEAD or OPTIONS, as the production check sees it.
fn is_safe(root: &Path, item: &Selected, request: &Request) -> Result<bool> {
    let method = send::effective_method(root, &item.id, request)?;
    Ok(send::SAFE_METHODS.contains(&method.as_str()))
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn flatten(nodes: &[TreeNode], out: &mut Vec<Selected>) {
    for node in nodes {
        match node {
            TreeNode::Folder { children, .. } => flatten(children, out),
            TreeNode::Request {
                id, name, method, ..
            } => out.push(Selected {
                id: id.clone(),
                name: name.clone(),
                method: method.clone(),
            }),
        }
    }
}
