//! REST Client file variables (`@name = value`) at send time: they win over environment
//! variables and may use them.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::environments::Values;
use crate::http_file::{self, FileVariable, Request};
use crate::workspace::{parse_request_id, resolve};
use crate::{Result, variables};

/// The file variables visible to `request` (an unsaved edit of the request at `id`): the
/// saved file's, with the request's own lines (and the prefix, for the first request) taken
/// from the edit.
pub fn collect(root: &Path, id: &str, request: &Request) -> Result<Vec<FileVariable>> {
    let (rel, index) = parse_request_id(id)?;
    let path = resolve(root, rel)?;
    let mut requests = match path.is_file() {
        true => http_file::parse(&fs::read_to_string(path)?),
        false => Vec::new(),
    };
    match requests.get_mut(index) {
        Some(saved) => {
            saved.variables = request.variables.clone();
            if index == 0 {
                saved.prefix = request.prefix.clone();
            }
        }
        None => requests.push(request.clone()),
    }
    Ok(http_file::file_variables(&requests))
}

/// Raw values, so variables used inside them (`{{$prompt otp}}`, chained responses) are found
/// and resolved like the request's own.
pub fn raw_values(root: &Path, id: &str, request: &Request) -> Result<Vec<String>> {
    Ok(collect(root, id, request)?
        .into_iter()
        .map(|variable| variable.value)
        .collect())
}

/// Adds the file variables to `values`, after environment and dynamic values are in. Names in
/// `keep` (e.g. a run's data-file row) are left alone. A variable whose value can't be resolved
/// only fails the send when the request uses it: `text` is the request's own text.
pub fn resolve_into(
    root: &Path,
    id: &str,
    request: &Request,
    text: &str,
    keep: &BTreeMap<String, String>,
    values: &mut Values,
) -> Result<()> {
    let mut failed = BTreeMap::new();
    for variable in collect(root, id, request)? {
        if keep.contains_key(&variable.name) {
            continue;
        }
        match variables::substitute(&variable.value, values) {
            Ok(value) => {
                failed.remove(&variable.name);
                // `@token = {{token}}` (or anything built from a secret) stays a secret, so
                // it is still masked in what is returned and stored.
                let from_secret = variables::used(&variable.value)
                    .iter()
                    .any(|name| values.secrets.contains_key(name));
                values.secrets.remove(&variable.name);
                values.variables.remove(&variable.name);
                match from_secret {
                    true => values.secrets.insert(variable.name, value),
                    false => values.variables.insert(variable.name, value),
                };
            }
            Err(err) => {
                failed.insert(variable.name, err);
            }
        }
    }
    let used = variables::used(text);
    match failed.into_iter().find(|(name, _)| used.contains(name)) {
        Some((_, err)) => Err(err),
        None => Ok(()),
    }
}

/// The file variables `request` needs when it leaves its file: the ones it uses, the ones those
/// use (transitively), and its own, in file order.
pub fn needed_by(all: &[FileVariable], request: &Request) -> Vec<FileVariable> {
    let mut text: Vec<&str> = vec![&request.url, &request.body];
    text.extend(request.headers.iter().map(|h| h.value.as_str()));
    text.extend(request.assertions.iter().map(String::as_str));
    let mut needed: Vec<String> = variables::used(&text.join("\n"));
    needed.extend(request.variables.iter().map(|v| v.name.clone()));
    let mut i = 0;
    while i < needed.len() {
        let current = needed[i].clone();
        for variable in all.iter().filter(|v| v.name == current) {
            for name in variables::used(&variable.value) {
                if !needed.contains(&name) {
                    needed.push(name);
                }
            }
        }
        i += 1;
    }
    all.iter()
        .filter(|v| needed.contains(&v.name))
        .cloned()
        .collect()
}

#[cfg(test)]
#[path = "file_vars_tests.rs"]
mod tests;
