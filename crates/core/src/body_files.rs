//! Bodies that include files (`< ./data.json`), read at send time relative to the `.http` file
//! and only from inside the workspace.

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use crate::environments::Values;
use crate::workspace::{self, parse_request_id};
use crate::{Error, Result, multipart, variables};

/// The bytes to send, and how the body is shown in history (`<file ./photo.png, 12.0 KB>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedBody {
    pub bytes: Vec<u8>,
    pub description: String,
}

/// A file the user picked to send, as request `id` refers to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BodyFile {
    /// Relative to the `.http` file: `./data/new.json`, `../files/photo.png`.
    pub path: String,
    /// A body that sends just this file: `< ./data/new.json`.
    pub body: String,
    /// From the file's extension.
    pub content_type: String,
}

/// `picked` (an absolute path) as request `id`'s body file. Files outside the workspace are
/// refused: the path is stored relative to the `.http` file, so it works on every machine.
pub fn reference(root: &Path, id: &str, picked: &Path) -> Result<BodyFile> {
    let outside = || {
        Error::Invalid(
            "pick a file inside the workspace: body files are stored relative to the .http file"
                .into(),
        )
    };
    let picked = fs::canonicalize(picked)?;
    let inside = picked
        .strip_prefix(fs::canonicalize(root)?)
        .map_err(|_| outside())?;
    let target: Vec<String> = inside
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_string())
        .collect();
    let dir = request_dir(id)?;
    let common = dir.iter().zip(&target).take_while(|(a, b)| a == b).count();
    let rest = target[common..].join("/");
    let path = match dir.len() - common {
        0 => format!("./{rest}"),
        ups => format!("{}{rest}", "../".repeat(ups)),
    };
    Ok(BodyFile {
        body: format!("< {path}"),
        content_type: multipart::content_type_for(&path).to_string(),
        path,
    })
}

/// Fills in the `{{variables}}` of a body as written and reads its `< ./path` lines. Only lines
/// written that way load a file: a value that looks like one (from a response, a prompt, …) is
/// sent as text, so it can't make the app send a workspace file. `<@ ./path` files are text
/// templates: their `{{variables}}` are resolved with `values` first. Multipart bodies get CRLF
/// line endings, as the format requires; other bodies keep their text as written.
pub fn load(
    root: &Path,
    id: &str,
    body: &str,
    content_type: Option<&str>,
    values: &Values,
) -> Result<LoadedBody> {
    let is_multipart = content_type.and_then(multipart::boundary).is_some();
    let has_files = body
        .lines()
        .any(|line| multipart::file_reference(line).is_some());
    if !is_multipart && !has_files {
        let text = variables::substitute(body, values)?;
        return Ok(LoadedBody {
            bytes: text.as_bytes().to_vec(),
            description: text,
        });
    }
    let pieces = pieces(root, id, body, values)?;
    let newline: &[u8] = if is_multipart { b"\r\n" } else { b"\n" };
    let mut bytes = pieces
        .iter()
        .map(|(bytes, _)| bytes.as_slice())
        .collect::<Vec<_>>()
        .join(newline);
    if is_multipart {
        bytes.extend_from_slice(newline);
    }
    Ok(LoadedBody {
        bytes,
        description: pieces
            .iter()
            .map(|(_, shown)| shown.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    })
}

/// Each line of `body` as bytes to send and as shown in history: a referenced file's content,
/// or the line with its variables filled in (a value with line breaks gives several lines).
fn pieces(root: &Path, id: &str, body: &str, values: &Values) -> Result<Vec<(Vec<u8>, String)>> {
    let dir = request_dir(id)?;
    let mut pieces = Vec::new();
    for line in body.lines() {
        match multipart::file_reference(line) {
            Some(path) => {
                let path = variables::substitute(path, values)?;
                pieces.push(read_file(root, &dir, &path, line, values)?);
            }
            None => {
                for text in variables::substitute(line, values)?.split('\n') {
                    let text = text.trim_end_matches('\r');
                    pieces.push((text.as_bytes().to_vec(), text.to_string()));
                }
            }
        }
    }
    Ok(pieces)
}

/// The text of every `<@ ./path` template in `body`, so the variables they use are resolved
/// (and `$prompt`s asked) like those in the request itself. Unreadable files are skipped here;
/// [`load`] reports them.
pub fn templates(root: &Path, id: &str, body: &str) -> Vec<String> {
    let Ok(dir) = request_dir(id) else {
        return Vec::new();
    };
    body.lines()
        .filter(|line| multipart::is_template_reference(line))
        .filter_map(multipart::file_reference)
        .filter_map(|path| resolve_file(root, &dir, path).ok())
        .filter_map(|file| fs::read_to_string(file).ok())
        .collect()
}

/// One referenced file's bytes (a `<@` template with its variables resolved) and how it is
/// shown in history.
fn read_file(
    root: &Path,
    dir: &[String],
    path: &str,
    line: &str,
    values: &Values,
) -> Result<(Vec<u8>, String)> {
    let file = resolve_file(root, dir, path)?;
    let content = fs::read(&file)
        .map_err(|err| Error::Invalid(format!("failed to read body file {path}: {err}")))?;
    if !multipart::is_template_reference(line) {
        let shown = format!("<file {path}, {}>", size(content.len()));
        return Ok((content, shown));
    }
    let text = String::from_utf8(content).map_err(|_| {
        Error::Invalid(format!(
            "{path} isn't text, so `<@` can't resolve variables in it; use `<`"
        ))
    })?;
    let content = variables::substitute(&text, values)?.into_bytes();
    let shown = format!("<file {path} with variables, {}>", size(content.len()));
    Ok((content, shown))
}

/// The request file's folder, workspace-relative, as path components.
fn request_dir(id: &str) -> Result<Vec<String>> {
    let (rel, _) = parse_request_id(id)?;
    let (folder, _) = workspace::split_rel(rel);
    Ok(folder
        .split('/')
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect())
}

/// The workspace-relative path of the file a body line of request `id` loads.
pub(crate) fn workspace_path(id: &str, path: &str) -> Result<String> {
    relative_to(&request_dir(id)?, path)
}

/// `path` relative to the request's folder, kept inside the workspace.
fn resolve_file(root: &Path, dir: &[String], path: &str) -> Result<PathBuf> {
    workspace::resolve_existing(root, &relative_to(dir, path)?).map_err(|err| match err {
        Error::NotFound(_) => Error::Invalid(format!("failed to read body file {path}: not found")),
        other => other,
    })
}

fn relative_to(dir: &[String], path: &str) -> Result<String> {
    let outside = || {
        Error::Invalid(format!(
            "body file {path} must be inside the workspace, relative to the .http file"
        ))
    };
    let mut parts = dir.to_vec();
    for component in Path::new(path).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            Component::ParentDir => {
                parts.pop().ok_or_else(outside)?;
            }
            Component::RootDir | Component::Prefix(_) => return Err(outside()),
        }
    }
    Ok(parts.join("/"))
}

fn size(bytes: usize) -> String {
    let kb = bytes as f64 / 1024.0;
    match bytes {
        0..1024 => format!("{bytes} B"),
        _ if kb < 1024.0 => format!("{kb:.1} KB"),
        _ => format!("{:.1} MB", kb / 1024.0),
    }
}

#[cfg(test)]
#[path = "body_files_tests.rs"]
mod tests;
