//! Splits `.http` text into `###` blocks and reads one block into a [`Request`] plus the
//! [`Template`] of how it was written.

use super::template::{Line, Template};
use crate::http_file::{FileVariable, Header, Request, is_variable_name};

/// A block of the file between `###` lines, with its exact text.
pub(crate) struct RawBlock<'a> {
    /// The `###` line without its line ending; `None` for the text before the first one.
    pub title_line: Option<&'a str>,
    /// Lines without line endings (`\r` removed).
    pub lines: Vec<&'a str>,
    pub raw: &'a str,
}

pub(crate) fn split_blocks(text: &str) -> Vec<RawBlock<'_>> {
    let mut blocks = vec![RawBlock {
        title_line: None,
        lines: Vec::new(),
        raw: "",
    }];
    let mut start = 0;
    let mut offset = 0;
    for raw_line in text.split_inclusive('\n') {
        let line = content(raw_line);
        if line.starts_with("###") {
            if let Some(block) = blocks.last_mut() {
                block.raw = &text[start..offset];
            }
            start = offset;
            blocks.push(RawBlock {
                title_line: Some(line),
                lines: Vec::new(),
                raw: "",
            });
        } else if let Some(block) = blocks.last_mut() {
            block.lines.push(line);
        }
        offset += raw_line.len();
    }
    if let Some(block) = blocks.last_mut() {
        block.raw = &text[start..];
    }
    blocks
}

/// A line without its `\n` / `\r\n` ending.
fn content(raw_line: &str) -> &str {
    let line = raw_line.strip_suffix('\n').unwrap_or(raw_line);
    line.strip_suffix('\r').unwrap_or(line)
}

/// A request and the line template it was read from; `None` without a request line.
pub(crate) fn parse_block(title_line: Option<&str>, lines: &[&str]) -> Option<(Request, Template)> {
    let title = title_line
        .and_then(|line| line.strip_prefix("###"))
        .unwrap_or_default()
        .trim();
    let mut request = Request::new(title, "GET", "");
    let mut template = Template {
        title_line: title_line.map(str::to_string),
        title: title.to_string(),
        ..Template::default()
    };

    let (request_line, mut i) = read_preamble(&mut request, &mut template, lines)?;
    template.request_lines.push(request_line.to_string());
    let (method, url, version) = split_request_line(request_line.trim());
    request.method = method;
    request.url = url;
    template.version = version;

    // Multi-line query strings: indented lines starting with `?` or `&`.
    while let Some(line) = lines.get(i) {
        let trimmed = line.trim_start();
        let indented = line.len() > trimmed.len();
        if !(indented && (trimmed.starts_with('?') || trimmed.starts_with('&'))) {
            break;
        }
        request.url.push_str(trimmed.trim_end());
        template.request_lines.push(line.to_string());
        i += 1;
    }
    template.method = request.method.clone();
    template.url = request.url.clone();

    i = read_headers(&mut request, &mut template, lines, i);

    template.trailing_blanks = lines
        .iter()
        .rev()
        .take_while(|l| l.trim().is_empty())
        .count();
    let remaining = &lines[i.min(lines.len())..];
    let trailer_start = remaining
        .iter()
        .position(|line| is_trailer(line))
        .unwrap_or(remaining.len());
    request.body = remaining[..trailer_start].join("\n").trim_end().to_string();
    request.trailer = remaining[trailer_start..].join("\n").trim_end().to_string();
    Some((request, template))
}

/// Comments, directives and file variables until the request line. Returns the request line
/// and the index of the line after it.
fn read_preamble<'a>(
    request: &mut Request,
    template: &mut Template,
    lines: &[&'a str],
) -> Option<(&'a str, usize)> {
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            template.preamble.push(Line::Blank);
            continue;
        }
        if let Some(variable) = file_variable(trimmed) {
            template.preamble.push(Line::Variable {
                variable: variable.clone(),
                raw: line.to_string(),
            });
            request.variables.push(variable);
            continue;
        }
        match comment_text(trimmed) {
            Some(comment) => {
                let item = read_directive(request, comment, line);
                template.preamble.push(item);
            }
            None => return Some((line, i + 1)),
        }
    }
    None
}

/// Headers until a blank line. Comments here stay in place; a response handler right after
/// the headers ends them. Returns the index of the first body line.
fn read_headers(
    request: &mut Request,
    template: &mut Template,
    lines: &[&str],
    mut i: usize,
) -> usize {
    while let Some(line) = lines.get(i) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return i + 1;
        }
        if is_trailer(line) {
            return i;
        }
        i += 1;
        if let Some(comment) = comment_text(trimmed) {
            let item = match assertion(comment) {
                Some(expr) => {
                    request.assertions.push(expr.clone());
                    Line::Assertion {
                        expr,
                        raw: line.to_string(),
                    }
                }
                None => Line::Comment {
                    raw: line.to_string(),
                },
            };
            template.headers.push(item);
            continue;
        }
        let Some((name, value)) = trimmed.split_once(':') else {
            // Not a header; kept verbatim so saving doesn't drop it.
            template.headers.push(Line::Comment {
                raw: line.to_string(),
            });
            continue;
        };
        let header = Header {
            name: name.trim().to_string(),
            value: value.trim().to_string(),
        };
        template.headers.push(Line::Header {
            header: header.clone(),
            raw: line.to_string(),
        });
        request.headers.push(header);
    }
    i
}

pub(crate) fn comment_text(trimmed: &str) -> Option<&str> {
    trimmed
        .strip_prefix('#')
        .or_else(|| trimmed.strip_prefix("//"))
        .map(str::trim)
}

/// The expression of an `@assert` directive.
fn assertion(comment: &str) -> Option<String> {
    comment
        .strip_prefix("@assert ")
        .map(|expr| expr.trim().to_string())
}

/// The value of an `@name` directive: `@name login`, `@name=login`. `@namespace` isn't one.
pub(crate) fn name_directive(comment: &str) -> Option<&str> {
    let rest = comment.strip_prefix("@name")?;
    let valid = rest.is_empty() || rest.starts_with(char::is_whitespace) || rest.starts_with('=');
    valid.then(|| rest.trim().trim_start_matches('=').trim())
}

fn read_directive(request: &mut Request, comment: &str, line: &str) -> Line {
    if let Some(expr) = assertion(comment) {
        request.assertions.push(expr.clone());
        return Line::Assertion {
            expr,
            raw: line.to_string(),
        };
    }
    if request.settings.read(comment) {
        return Line::Setting {
            raw: line.to_string(),
        };
    }
    // `# @name login` stays a comment line (REST Client needs it for chaining); without a
    // `###` title it is also the request's display name.
    if let Some(name) = name_directive(comment)
        && request.name.is_empty()
    {
        request.name = name.to_string();
    }
    request.comments.push(line.trim().to_string());
    Line::Comment {
        raw: line.to_string(),
    }
}

/// `@name = value` (spaces optional).
pub(crate) fn file_variable(line: &str) -> Option<FileVariable> {
    let (name, value) = line.strip_prefix('@')?.split_once('=')?;
    let name = name.trim();
    is_variable_name(name).then(|| FileVariable {
        name: name.to_string(),
        value: value.trim().to_string(),
    })
}

/// Method, URL and HTTP version (`HTTP/1.1`, kept so it is written back).
fn split_request_line(line: &str) -> (String, String, Option<String>) {
    let (line, version) = match line.rsplit_once(char::is_whitespace) {
        Some((before, version)) if version.starts_with("HTTP/") => {
            (before.trim_end(), Some(version.to_string()))
        }
        _ => (line, None),
    };
    if let Some((first, rest)) = line.split_once(char::is_whitespace)
        && is_method(first)
    {
        return (first.to_string(), rest.trim().to_string(), version);
    }
    // A lone method (`POST`) is a request without a URL yet, as created in the app.
    if is_lone_method(line) {
        return (line.to_string(), String::new(), version);
    }
    // A bare URL means GET.
    ("GET".to_string(), line.to_string(), version)
}

/// Any uppercase token is a method, so `PURGE` and `PROPFIND` work like `GET`.
pub(crate) fn is_method(token: &str) -> bool {
    !token.is_empty()
        && token
            .chars()
            .all(|c| c.is_ascii_uppercase() || c == '-' || c == '_')
}

/// A method on its own line: letters A-Z only, so bare hosts like `localhost` stay URLs.
pub(crate) fn is_lone_method(token: &str) -> bool {
    token.len() >= 3 && token.chars().all(|c| c.is_ascii_uppercase())
}

/// Response handlers (`> {% … %}`, `> ./handler.js`) and response redirects (`>> file`,
/// `>>! file`, `<> file`). A body line that merely starts with `> ` (a quote) isn't one.
pub(crate) fn is_trailer(line: &str) -> bool {
    if line.starts_with(">>") || line.starts_with("<> ") {
        return true;
    }
    let Some(rest) = line.strip_prefix("> ") else {
        return false;
    };
    let rest = rest.trim();
    rest.starts_with("{%")
        || rest.starts_with("./")
        || rest.starts_with("../")
        || rest.starts_with('/')
        || (rest.ends_with(".js") && !rest.contains(char::is_whitespace))
}
