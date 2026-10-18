use std::path::{Path, PathBuf};

use super::hosts::{plain_values, resolve_plain};
use crate::environments::Values;
use crate::http_file::Request;
use crate::workspace::ENVIRONMENTS_DIR;
use crate::{Result, body_files, multipart};

/// The workspace files a request's body sends (`< ./data.json`, `<@ ./template.json`).
#[derive(Debug, Default)]
pub(super) struct BodyFiles {
    /// As written, with plain variables filled in.
    pub paths: Vec<String>,
    /// Why an agent may never send one of them, if so.
    pub forbidden: Option<String>,
}

pub(super) fn body_files(
    root: &Path,
    id: &str,
    request: &Request,
    environment: Option<&str>,
) -> Result<BodyFiles> {
    let written: Vec<&str> = request
        .body
        .lines()
        .filter_map(multipart::file_reference)
        .collect();
    let mut files = BodyFiles::default();
    if written.is_empty() && !request.body.contains("{{") {
        return Ok(files);
    }
    let values = plain_values(root, id, request, environment)?;
    for path in written {
        let path = resolve_plain(path, &values)?;
        if files.forbidden.is_none() {
            files.forbidden = forbidden(root, id, &path);
        }
        files.paths.push(path);
    }
    if files.forbidden.is_none() && added_by_variables(&request.body, &values, &files.paths)? {
        files.forbidden = Some(
            "a variable in the body adds a file to it (`< ./path`); AI agents can only send \
             files named in the request"
                .to_string(),
        );
    }
    Ok(files)
}

/// Whether filling in the body adds file references to the ones written in it. The send never
/// loads those, but an agent mustn't try.
fn added_by_variables(body: &str, values: &Values, written: &[String]) -> Result<bool> {
    let filled = resolve_plain(body, values)?;
    let shown: Vec<&str> = filled
        .lines()
        .filter_map(multipart::file_reference)
        .collect();
    Ok(shown != written)
}

/// Environment files hold what the user may hide from AI, and `.env` files hold secrets. A path
/// that still has a `{{placeholder}}` comes from a secret or a response, so nobody can tell
/// which file it is.
fn forbidden(root: &Path, id: &str, path: &str) -> Option<String> {
    if path.contains("{{") {
        return Some(format!(
            "the body sends a file whose name comes from a secret or a response ({path}); AI \
             agents can only send files named in the request"
        ));
    }
    // A path outside the workspace is refused when the request is sent.
    let rel = PathBuf::from(body_files::workspace_path(id, path).ok()?);
    let real = real_path(root, &rel);
    if [Some(rel), real]
        .into_iter()
        .flatten()
        .any(|p| is_private(&p))
    {
        return Some(format!(
            "the body sends {path}; AI agents can't send environment or .env files"
        ));
    }
    None
}

/// Why an agent may never read `path` (absolute, links resolved), if so: environment and
/// `.env` files, as for request bodies.
pub fn private_file(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).unwrap_or(path);
    is_private(rel).then(|| {
        format!(
            "{} is an environment or .env file; AI agents can't read those",
            rel.display()
        )
    })
}

/// Where a link inside the workspace really points, relative to the workspace.
fn real_path(root: &Path, rel: &Path) -> Option<PathBuf> {
    let real = root.join(rel).canonicalize().ok()?;
    let root = root.canonicalize().ok()?;
    Some(real.strip_prefix(root).ok()?.to_path_buf())
}

/// Letter case is ignored: macOS and Windows open `Environments/` and `.ENV` too.
fn is_private(rel: &Path) -> bool {
    let in_environments = rel
        .components()
        .next()
        .is_some_and(|first| first.as_os_str().eq_ignore_ascii_case(ENVIRONMENTS_DIR));
    let dotenv = rel
        .file_name()
        .is_some_and(|name| name.to_string_lossy().to_lowercase().starts_with(".env"));
    in_environments || dotenv
}
