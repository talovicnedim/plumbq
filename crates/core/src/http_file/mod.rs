//! Reads and writes `.http` files (JetBrains HTTP Client / VS Code REST Client), losslessly.
//! Format and saving rules: `docs/formats.md` (`.http` files).

mod merge;
pub(crate) mod read;
mod template;

use serde::{Deserialize, Serialize};

use crate::request_settings::RequestSettings;
use crate::{Error, Result};
use read::{file_variable, is_trailer, parse_block, split_blocks};

const BOM: char = '\u{feff}';

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    /// From the `###` separator line or `# @name`; empty when the file gives none.
    pub name: String,
    pub method: String,
    /// As written, with `{{variables}}` unresolved.
    pub url: String,
    pub headers: Vec<Header>,
    pub body: String,
    /// Expressions from `# @assert` lines, e.g. `status == 200`.
    pub assertions: Vec<String>,
    /// Comment and unknown directive lines before the request line (including `# @name`),
    /// trimmed but otherwise verbatim.
    pub comments: Vec<String>,
    /// Response handlers and redirects (`> {% … %}`, `<> file`), kept verbatim but not run.
    pub trailer: String,
    /// `@name = value` lines written before this request's request line.
    #[serde(default)]
    pub variables: Vec<FileVariable>,
    /// From `# @no-redirect`, `# @timeout 60`, … lines.
    #[serde(default)]
    pub settings: RequestSettings,
    /// Text before the file's first `###` that isn't a request (notes, file variables). Only
    /// ever set on the first request of a file, and written back before it.
    #[serde(default)]
    pub prefix: String,
    /// The exact text this request was read from: its own lines plus notes around it (and, for
    /// the first request, the file's start). Used to write unchanged requests back byte for
    /// byte and edited ones line by line. Empty for new requests.
    #[serde(default)]
    pub source: String,
}

/// A REST Client file variable: `@name = value`, usable as `{{name}}` in the whole file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FileVariable {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    pub name: String,
    pub value: String,
}

impl Request {
    pub fn new(name: &str, method: &str, url: &str) -> Self {
        Self {
            name: name.to_string(),
            method: method.to_string(),
            url: url.to_string(),
            headers: Vec::new(),
            body: String::new(),
            assertions: Vec::new(),
            comments: Vec::new(),
            trailer: String::new(),
            variables: Vec::new(),
            settings: RequestSettings::default(),
            prefix: String::new(),
            source: String::new(),
        }
    }

    /// Everything but [`Request::source`]: what the request means, for change detection.
    fn same_content(&self, other: &Request) -> bool {
        let without_source = |r: &Request| Request {
            source: String::new(),
            ..r.clone()
        };
        without_source(self) == without_source(other)
    }
}

/// Parses every request in a file. Blocks without a request line are skipped (their text
/// stays in a neighbour's [`Request::source`]); the text before the first `###` is also the
/// first request's [`Request::prefix`].
pub fn parse(text: &str) -> Vec<Request> {
    let (bom, text) = match text.strip_prefix(BOM) {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let mut prefix = String::new();
    let mut pending = String::new();
    let mut requests: Vec<Request> = Vec::new();
    for (index, block) in split_blocks(text).into_iter().enumerate() {
        match parse_block(block.title_line, &block.lines) {
            Some((mut request, _)) => {
                request.source = std::mem::take(&mut pending) + block.raw;
                requests.push(request);
            }
            None => {
                if index == 0 {
                    prefix = block.lines.join("\n").trim().to_string();
                }
                pending.push_str(block.raw);
            }
        }
    }
    if let Some(last) = requests.last_mut() {
        last.source.push_str(&pending);
    }
    if let Some(first) = requests.first_mut() {
        first.prefix = prefix;
        if bom {
            first.source.insert(0, BOM);
        }
    }
    requests
}

/// Writes requests back as one file. Unchanged requests come out exactly as they were read.
pub fn serialize(requests: &[Request]) -> String {
    let ending = match requests.iter().any(|r| r.source.contains("\r\n")) {
        true => "\r\n",
        false => "\n",
    };
    let blank = format!("{ending}{ending}");
    let mut out = String::new();
    for (index, request) in requests.iter().enumerate() {
        // Text from the start of a file (notes, file variables, a BOM) can only go first;
        // anywhere else it would become the previous request's body.
        let starts_block = request.source.starts_with("###");
        let unchanged = (index == 0 || starts_block) && is_unchanged(request);
        let piece = match unchanged {
            true => request.source.clone(),
            false => template::write(request, ending, index == 0),
        };
        if !out.is_empty() && !out.ends_with('\n') {
            out.push_str(ending);
        }
        // Rewritten requests get a blank line before them, like the requests we create.
        if !unchanged && !out.is_empty() && !out.ends_with(&blank) {
            out.push_str(ending);
        }
        out.push_str(&piece);
    }
    out
}

/// Whether `request` still means exactly what its [`Request::source`] says, so the source can
/// be written back as is.
fn is_unchanged(request: &Request) -> bool {
    if request.source.is_empty() {
        return false;
    }
    match parse(&request.source).as_slice() {
        [parsed] => parsed.same_content(request),
        _ => false,
    }
}

/// Fails when a request can't be written without changing its meaning: line breaks in fields
/// that must stay on one line (they would inject lines, even whole requests, into the file), or
/// a method, header, file variable or comment that would read back as something else. Call
/// before creating or updating a request.
pub fn validate(request: &Request) -> Result<()> {
    single_line("name", &request.name)?;
    single_line("url", &request.url)?;
    validate_method(&request.method, &request.url)?;
    for header in &request.headers {
        single_line("header", &format!("{}{}", header.name, header.value))?;
        if !is_header_name(&header.name) {
            return Err(invalid("header name", &header.name));
        }
    }
    for variable in &request.variables {
        let line = format!("{}{}", variable.name, variable.value);
        single_line("file variable", &line)?;
        if !is_variable_name(&variable.name) {
            return Err(invalid("file variable name", &variable.name));
        }
    }
    for line in &request.assertions {
        single_line("assertion", line)?;
    }
    for line in &request.comments {
        single_line("comment", line)?;
        if read::comment_text(line.trim()).is_none() {
            return Err(Error::Invalid(format!(
                "comment {line:?} must start with # or //"
            )));
        }
    }
    Ok(())
}

/// A name a header line reads back with: not empty, no `:`, not a comment.
pub(crate) fn is_header_name(name: &str) -> bool {
    let name = name.trim();
    !name.is_empty() && !name.contains(':') && read::comment_text(name).is_none()
}

/// A file variable name: letters, digits, `_`, `-` and `.`.
pub(crate) fn is_variable_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

fn single_line(field: &str, value: &str) -> Result<()> {
    match value.contains(['\r', '\n']) {
        true => Err(Error::Invalid(format!("{field} must be a single line"))),
        false => Ok(()),
    }
}

/// Methods are tokens in capitals (`POST`, `PROPFIND`); without a URL only letters, at least
/// three, so the line isn't read as a bare URL.
fn validate_method(method: &str, url: &str) -> Result<()> {
    if !read::is_method(method) {
        return Err(Error::Invalid(format!(
            "invalid method {method:?}: use capital letters, like POST"
        )));
    }
    if url.is_empty() && !read::is_lone_method(method) {
        return Err(Error::Invalid(format!("method {method} needs a url")));
    }
    Ok(())
}

fn invalid(what: &str, value: &str) -> Error {
    Error::Invalid(format!("invalid {what} {value:?}"))
}

/// Body lines the format can't keep as body: a line starting with `###` starts a new request
/// and a `> {%` / `> ./file` line starts a response handler when the file is read again.
pub fn body_warnings(request: &Request) -> Vec<String> {
    request
        .body
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let n = i + 1;
            if line.starts_with("###") {
                Some(format!(
                    "body line {n} starts with ### and will be read as a new request"
                ))
            } else if is_trailer(line) {
                Some(format!(
                    "body line {n} looks like a response handler and will be cut from the body"
                ))
            } else {
                None
            }
        })
        .collect()
}

/// Every file variable of a parsed file, in file order: the prefix's, then each request's.
pub fn file_variables(requests: &[Request]) -> Vec<FileVariable> {
    let mut all = Vec::new();
    if let Some(first) = requests.first() {
        all.extend(
            first
                .prefix
                .lines()
                .filter_map(|line| file_variable(line.trim())),
        );
    }
    for request in requests {
        all.extend(request.variables.iter().cloned());
    }
    all
}

#[cfg(test)]
mod edit_tests;

#[cfg(test)]
mod tests;
