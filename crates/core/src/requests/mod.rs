//! Requests in a workspace: read, create, update, move and remove. How ids shift and why writes
//! carry the text they read: `docs/formats.md` (`.http` files).

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::http_file::{self, Request};
use crate::workspace::{self, join_rel, parse_request_id, request_id, resolve, split_rel};
use crate::{Error, Result, examples, file_vars, fs_util};

mod search;

pub use search::search;

/// A request id that changed because of an edit. `to` is `None` for a removed request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IdChange {
    pub from: String,
    pub to: Option<String>,
}

/// Where a moved request ended up, and every id the move changed (its own included).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RequestMove {
    pub id: String,
    pub changes: Vec<IdChange>,
}

pub fn get(root: &Path, id: &str) -> Result<Request> {
    let (mut requests, index) = read_file_of(root, id)?;
    Ok(requests.swap_remove(index))
}

/// A short token for the text a request was read from, for clients that can't send the whole
/// [`Request::source`] back (MCP agents get it with the request and pass it to writes).
pub fn version(request: &Request) -> String {
    let hash = Sha256::digest(request.source.as_bytes());
    hash.iter().take(6).map(|b| format!("{b:02x}")).collect()
}

/// Creates a request in its own file in `folder` (`""` for the root), named after the request.
/// Returns the new id.
pub fn create(root: &Path, folder: &str, request: Request) -> Result<String> {
    http_file::validate(&request)?;
    let dir = resolve(root, folder)?;
    if !dir.is_dir() {
        return Err(Error::NotFound(format!("folder {folder}")));
    }
    workspace::check_settings(&dir)?;
    let file_name = create_unique(&dir, &workspace::slug(&request.name), &request)?;
    workspace::insert_in_order(&dir, &file_name, None)?;
    Ok(join_rel(folder, &file_name))
}

/// Writes the request to a new file named after `base`, never over an existing one: two
/// creates with the same name at the same time (e.g. parallel agent calls) get `-2`, `-3`, ….
fn create_unique(dir: &Path, base: &str, request: &Request) -> Result<String> {
    let text = serialize_checked(std::slice::from_ref(request))?;
    let mut n = 1;
    loop {
        let name = match n {
            1 => format!("{base}.http"),
            _ => format!("{base}-{n}.http"),
        };
        match fs_util::write_new(&dir.join(&name), &text) {
            Ok(()) => return Ok(name),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => n += 1,
            Err(err) => return Err(err.into()),
        }
    }
}

/// Replaces a request, keeping its file name and any other requests in the same file. The
/// file's prefix (notes, file variables) stays unless the new request brings its own.
/// Returns the request as now on disk, with its fresh [`Request::source`] for the next write.
pub fn update(
    root: &Path,
    id: &str,
    mut request: Request,
    expected: Option<&str>,
) -> Result<Request> {
    http_file::validate(&request)?;
    let (mut requests, index) = read_file_of(root, id)?;
    check_expected(&requests[index], id, expected)?;
    if request.prefix.is_empty() {
        request.prefix = std::mem::take(&mut requests[index].prefix);
    }
    requests[index] = request;
    write_file_of(root, id, &requests)?;
    get(root, id)
}

/// Removes a request; deletes its file when it was the only one in it. Returns the ids that
/// changed: the removed one, and later requests of the same file moving up one place.
pub fn remove(root: &Path, id: &str, expected: Option<&str>) -> Result<Vec<IdChange>> {
    let (requests, index) = read_file_of(root, id)?;
    check_expected(&requests[index], id, expected)?;
    remove_unchecked(root, id)
}

fn remove_unchecked(root: &Path, id: &str) -> Result<Vec<IdChange>> {
    let (mut requests, index) = read_file_of(root, id)?;
    let (rel, _) = parse_request_id(id)?;
    let count = requests.len();
    let removed = requests.remove(index);
    let mut changes = vec![IdChange {
        from: id.to_string(),
        to: None,
    }];
    changes.extend((index + 1..count).map(|i| IdChange {
        from: request_id(rel, i),
        to: Some(request_id(rel, i - 1)),
    }));
    // The file's prefix (notes, file variables) belongs to the file, not the first request.
    if let Some(first) = requests.first_mut()
        && index == 0
    {
        first.prefix = removed.prefix;
    }
    if !requests.is_empty() {
        write_file_of(root, id, &requests)?;
        examples::removed(root, id)?;
        return Ok(changes);
    }
    let (folder, file_name) = split_rel(rel);
    let dir = resolve(root, folder)?;
    workspace::check_settings(&dir)?;
    fs::remove_file(resolve(root, rel)?)?;
    examples::removed(root, id)?;
    workspace::rename_in_order(&dir, file_name, None)?;
    Ok(changes)
}

/// Moves a request to `folder` at `position` among its children (the end when `None`). A
/// request from a multi-request file is split out into its own file.
pub fn move_to(
    root: &Path,
    id: &str,
    folder: &str,
    position: Option<u32>,
    expected: Option<&str>,
) -> Result<RequestMove> {
    let (rel, index) = parse_request_id(id)?;
    let (from_folder, file_name) = split_rel(rel);
    let to_dir = resolve(root, folder)?;
    if !to_dir.is_dir() {
        return Err(Error::NotFound(format!("folder {folder}")));
    }
    let position = position.map(|p| p as usize);
    let (requests, _) = read_file_of(root, id)?;
    check_expected(&requests[index], id, expected)?;
    if requests.len() > 1 {
        return split_out(root, id, &requests, index, folder, position);
    }
    let from_dir = resolve(root, from_folder)?;
    workspace::check_settings(&from_dir)?;
    workspace::check_settings(&to_dir)?;
    let new_name = if from_folder == folder {
        file_name.to_string()
    } else {
        let stem = file_name.trim_end_matches(".http");
        let new_name = workspace::unique_name(&to_dir, stem, ".http");
        fs::rename(from_dir.join(file_name), to_dir.join(&new_name))?;
        workspace::rename_in_order(&from_dir, file_name, None)?;
        new_name
    };
    workspace::insert_in_order(&to_dir, &new_name, position)?;
    let new_id = join_rel(folder, &new_name);
    let mut changes = Vec::new();
    if new_id != id {
        // Only after the file moved, so a failed rename keeps the examples where they were.
        examples::put(root, &new_id, examples::take(root, id)?)?;
        changes.push(IdChange {
            from: id.to_string(),
            to: Some(new_id.clone()),
        });
    }
    Ok(RequestMove {
        id: new_id,
        changes,
    })
}

/// Moves one request of a multi-request file into its own file in `folder`. The new file gets
/// the file variables the request relies on, so it still sends the same. It's written before
/// the original is removed: a failure leaves a duplicate, never a lost request.
fn split_out(
    root: &Path,
    id: &str,
    requests: &[Request],
    index: usize,
    folder: &str,
    position: Option<usize>,
) -> Result<RequestMove> {
    let request = Request {
        prefix: String::new(),
        variables: file_vars::needed_by(&http_file::file_variables(requests), &requests[index]),
        ..requests[index].clone()
    };
    let new_id = create(root, folder, request)?;
    let (_, new_name) = split_rel(&new_id);
    workspace::insert_in_order(&resolve(root, folder)?, new_name, position)?;
    examples::put(root, &new_id, examples::take(root, id)?)?;
    let mut changes = remove_unchecked(root, id)?;
    changes[0].to = Some(new_id.clone());
    Ok(RequestMove {
        id: new_id,
        changes,
    })
}

fn check_expected(current: &Request, id: &str, expected: Option<&str>) -> Result<()> {
    match expected {
        Some(source) if source != current.source => Err(Error::Conflict(format!(
            "request {id} changed on disk since it was read; reload it and try again"
        ))),
        _ => Ok(()),
    }
}

fn read_file_of(root: &Path, id: &str) -> Result<(Vec<Request>, usize)> {
    let (rel, index) = parse_request_id(id)?;
    let path = resolve(root, rel)?;
    if !rel.ends_with(".http") || !path.is_file() {
        return Err(Error::NotFound(format!("request {id}")));
    }
    let requests = http_file::parse(&fs::read_to_string(path)?);
    if index >= requests.len() {
        return Err(Error::NotFound(format!("request {id}")));
    }
    Ok((requests, index))
}

/// Atomic, so a crash or full disk never leaves a half-written `.http` file behind.
fn write_file_of(root: &Path, id: &str, requests: &[Request]) -> Result<()> {
    let (rel, _) = parse_request_id(id)?;
    fs_util::write_atomic(&resolve(root, rel)?, serialize_checked(requests)?)?;
    Ok(())
}

/// The file text, refused when it would read back as other requests than the ones written
/// (two requests joined into one, a method or URL read differently).
fn serialize_checked(requests: &[Request]) -> Result<String> {
    let text = http_file::serialize(requests);
    // A body line starting with `###` starts a new request by design; the editor warns about it.
    let split_on_purpose = requests
        .iter()
        .any(|r| r.body.lines().any(|line| line.starts_with("###")));
    let back = http_file::parse(&text);
    let same = back.len() == requests.len()
        && back
            .iter()
            .zip(requests)
            .all(|(read, written)| read.method == written.method && read.url == written.url.trim());
    if !same && !split_on_purpose {
        return Err(Error::Invalid(
            "this change can't be saved: the file would not read back the same; check the \
             method and URL"
                .into(),
        ));
    }
    Ok(text)
}

#[cfg(test)]
mod safety_tests;
#[cfg(test)]
mod tests;
