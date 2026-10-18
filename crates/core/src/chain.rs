//! Request chaining (`{{login.response.body.$.token}}`): a value from another request's latest
//! response. Syntax and rules: `docs/formats.md` (Chaining).

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::path::Path;

use serde_json::Value;

use crate::environments::Values;
use crate::secret_names::looks_secret;
use crate::send::Response;
use crate::workspace::{self, TreeNode};
use crate::{Error, Result, history, json_path, request_meta, requests, variables};

/// A parsed `{{<request>.response.<part>.<path>}}` reference.
#[derive(Debug, PartialEq, Eq)]
struct Reference<'a> {
    request: &'a str,
    part: Part<'a>,
}

#[derive(Debug, PartialEq, Eq)]
enum Part<'a> {
    /// A JSON path starting with `$`, or `*` for the whole body.
    Body(&'a str),
    Header(&'a str),
}

fn parse(name: &str) -> Option<Reference<'_>> {
    let (request, rest) = name.split_once(".response.")?;
    if request.is_empty() || request.starts_with('$') {
        return None;
    }
    let part = if let Some(path) = rest.strip_prefix("body.") {
        Part::Body(path)
    } else {
        Part::Header(rest.strip_prefix("headers.")?)
    };
    Some(Reference { request, part })
}

pub fn is_reference(name: &str) -> bool {
    parse(name).is_some()
}

/// The request a reference reads from: `login` in `login.response.body.$.token`.
pub(crate) fn source_request(name: &str) -> Option<&str> {
    parse(name).map(|reference| reference.request)
}

/// Whether the request or the picked field is named like a credential. Only the request's own
/// name counts, not its folders (`auth/login.http`).
fn names_a_secret(reference: &Reference<'_>) -> bool {
    let request = reference.request.rsplit('/').next().unwrap_or_default();
    let (Part::Body(path) | Part::Header(path)) = reference.part;
    looks_secret(request) || looks_secret(path)
}

/// Adds the value of every chain reference in `text` to `values`, under the full variable name.
/// Fails naming the request when it was never sent, or the path when it isn't in the response.
pub fn resolve(
    text: &str,
    root: &Path,
    history_dir: Option<&Path>,
    environment: Option<&str>,
    values: &mut Values,
) -> Result<()> {
    let names: Vec<String> = variables::used(text)
        .into_iter()
        .filter(|name| is_reference(name))
        .collect();
    if names.is_empty() {
        return Ok(());
    }
    let Some(history_dir) = history_dir else {
        return Err(Error::Invalid(
            "request chaining needs response history, which isn't available here".into(),
        ));
    };
    // Read once per send, however many references there are.
    let tree = workspace::tree(root)?;
    let named = reference_names(root, &tree);
    let mut responses: BTreeMap<String, Response> = BTreeMap::new();
    for name in names {
        let Some(reference) = parse(&name) else {
            continue;
        };
        let id = find_request(&named, &tree, reference.request)?;
        let response = match responses.entry(id.clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                entry.insert(last_response(&name, root, history_dir, &id, environment)?)
            }
        };
        let (value, has_secret) = chained_value(&name, &reference, &id, response, values)?;
        let target = if has_secret || names_a_secret(&reference) {
            &mut values.secrets
        } else {
            &mut values.variables
        };
        target.insert(name, value);
    }
    Ok(())
}

/// The referenced part of `response`, with the secrets history masked put back.
fn chained_value(
    name: &str,
    reference: &Reference<'_>,
    id: &str,
    response: &Response,
    values: &Values,
) -> Result<(String, bool)> {
    let value = extract(&reference.part, &response.body, &response.headers).ok_or_else(|| {
        let cut = match response.truncated {
            true => " (history keeps only the start of a large body)",
            false => "",
        };
        Error::Invalid(format!(
            "{{{{{name}}}}}: not in the last response of {id}{cut}"
        ))
    })?;
    restore_masked(value, &response.masked_secrets, values).map_err(|placeholder| {
        Error::Invalid(format!(
            "{{{{{name}}}}}: the last response of {id} has a masked value {placeholder}; \
             send {id} again with the environment that has it"
        ))
    })
}

/// The request's last response in `environment`.
fn last_response(
    name: &str,
    root: &Path,
    history_dir: &Path,
    id: &str,
    environment: Option<&str>,
) -> Result<Response> {
    let entry = history::latest(history_dir, root, id, environment)?.ok_or_else(|| {
        Error::Invalid(format!(
            "{{{{{name}}}}} needs a response from {id}{}; send that request first",
            environment
                .map(|env| format!(" in environment {env}"))
                .unwrap_or_default()
        ))
    })?;
    Ok(entry.response)
}

/// History is stored masked, so a chained value can be `{{token}}` instead of the token. Only
/// the secrets the response lists as `masked` are put back: a `{{name}}` the server wrote is
/// text and stays text. A value that is only a placeholder it can't fill is an error, never
/// sent as is. Returns the value and whether it contains a secret.
fn restore_masked(
    value: String,
    masked: &[String],
    values: &Values,
) -> std::result::Result<(String, bool), String> {
    let mut restored = value.clone();
    let mut has_secret = false;
    for name in variables::used(&value) {
        let placeholder = format!("{{{{{name}}}}}");
        match values.secrets.get(&name).filter(|_| masked.contains(&name)) {
            Some(secret) => {
                restored = restored.replace(&placeholder, secret);
                has_secret = true;
            }
            None if value.trim() == placeholder => return Err(placeholder),
            None => {}
        }
    }
    Ok((restored, has_secret))
}

fn extract(part: &Part<'_>, body: &str, headers: &[crate::http_file::Header]) -> Option<String> {
    match part {
        Part::Header(name) => headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(name))
            .map(|h| h.value.clone()),
        Part::Body("*") => Some(body.to_string()),
        Part::Body(path) => {
            let json: Value = serde_json::from_str(body).ok()?;
            match json_path::get(&json, path)? {
                Value::String(text) => Some(text),
                other => Some(other.to_string()),
            }
        }
    }
}

/// The id of the request `reference` names: an id, a file stem, or a request name, compared
/// as slugs (`Log in` matches `log-in`). `named` holds the `# @name` of every request.
fn find_request(named: &[(String, String)], tree: &[TreeNode], reference: &str) -> Result<String> {
    // A REST Client `# @name login` is an explicit reference name, so it wins outright.
    let mut matches: Vec<String> = named
        .iter()
        .filter(|(_, name)| name == reference)
        .map(|(id, _)| id.clone())
        .collect();
    if matches.is_empty() {
        let wanted = workspace::slug(reference);
        collect_matches(tree, reference, &wanted, &mut matches);
    }
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => Err(Error::Invalid(format!(
            "no request named {reference} for {{{{{reference}.response…}}}}"
        ))),
        _ => Err(Error::Invalid(format!(
            "{reference} matches several requests ({}); use a unique name",
            matches.join(", ")
        ))),
    }
}

fn collect_matches(nodes: &[TreeNode], reference: &str, wanted: &str, out: &mut Vec<String>) {
    for node in nodes {
        match node {
            TreeNode::Folder { children, .. } => collect_matches(children, reference, wanted, out),
            TreeNode::Request { id, name, .. } => {
                let (_, file) = workspace::split_rel(id);
                let stem = file.trim_end_matches(".http");
                let hit = id == reference || stem == wanted || workspace::slug(name) == wanted;
                if hit && !out.contains(id) {
                    out.push(id.clone());
                }
            }
        }
    }
}

/// The `# @name` of every request in the tree, as (id, name).
fn reference_names(root: &Path, nodes: &[TreeNode]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for node in nodes {
        match node {
            TreeNode::Folder { children, .. } => out.extend(reference_names(root, children)),
            TreeNode::Request { id, .. } => {
                // An unreadable request can't be the one meant; the name matching still runs.
                let named = requests::get(root, id)
                    .ok()
                    .and_then(|request| request_meta::reference_name(&request));
                if let Some(name) = named {
                    out.push((id.clone(), name));
                }
            }
        }
    }
    out
}

#[cfg(test)]
#[path = "chain_tests.rs"]
mod tests;
