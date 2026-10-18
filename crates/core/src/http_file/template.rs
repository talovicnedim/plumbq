//! Rewrites an edited request against the lines it was read from, so everything the user
//! didn't touch keeps its place and exact text.

use super::merge::State;
use super::read;
use crate::http_file::{FileVariable, Header, Request};
use crate::request_meta;

const BOM: char = '\u{feff}';

/// One line of a request's text, classified, with its exact original text.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Line {
    Blank,
    Variable {
        variable: FileVariable,
        raw: String,
    },
    Setting {
        raw: String,
    },
    Assertion {
        expr: String,
        raw: String,
    },
    /// A comment or a directive we keep verbatim; among headers also unparsable lines.
    Comment {
        raw: String,
    },
    Header {
        header: Header,
        raw: String,
    },
}

/// How a request was written, from its source.
#[derive(Debug, Clone, Default)]
pub(crate) struct Template {
    /// The `###` line as written; `None` when the request had none (start of the file).
    pub title_line: Option<String>,
    pub title: String,
    pub preamble: Vec<Line>,
    /// The request line plus multi-line query continuation lines, as written.
    pub request_lines: Vec<String>,
    pub method: String,
    pub url: String,
    pub version: Option<String>,
    pub headers: Vec<Line>,
    /// Blank lines that end the block, usually the gap before the next `###`.
    pub trailing_blanks: usize,
}

/// A request's source split into the parts that are rewritten and the ones kept verbatim.
struct Source {
    bom: bool,
    /// The text before the file's first `###` when it isn't a request, and what it means.
    prefix_raw: String,
    prefix: String,
    /// Note blocks (`###` blocks without a request) before and after the request.
    before: String,
    after: String,
    template: Template,
}

fn read_source(source: &str) -> Option<Source> {
    let (bom, text) = match source.strip_prefix(BOM) {
        Some(rest) => (true, rest),
        None => (false, source),
    };
    let blocks = read::split_blocks(text);
    let (index, template) = blocks.iter().enumerate().find_map(|(i, block)| {
        read::parse_block(block.title_line, &block.lines).map(|(_, t)| (i, t))
    })?;
    let starts_file = blocks.first().is_some_and(|b| b.title_line.is_none()) && index > 0;
    let (prefix_raw, prefix) = match starts_file {
        true => (
            blocks[0].raw.to_string(),
            blocks[0].lines.join("\n").trim().to_string(),
        ),
        false => (String::new(), String::new()),
    };
    let first_note = usize::from(starts_file);
    Some(Source {
        bom,
        prefix_raw,
        prefix,
        before: blocks[first_note.min(index)..index]
            .iter()
            .map(|b| b.raw)
            .collect(),
        after: blocks[index + 1..].iter().map(|b| b.raw).collect(),
        template,
    })
}

/// The text of an edited (or new) request, ending with a line ending.
pub(crate) fn write(request: &Request, ending: &str, is_first: bool) -> String {
    let source = read_source(&request.source);
    let mut out = String::new();
    if let Some(source) = &source
        && is_first
    {
        if source.bom {
            out.push(BOM);
        }
        if source.prefix == request.prefix {
            out.push_str(&source.prefix_raw);
        }
    }
    let prefix_rewritten = source.as_ref().is_none_or(|s| s.prefix != request.prefix);
    if is_first && prefix_rewritten && !request.prefix.is_empty() {
        for line in request.prefix.lines() {
            out.push_str(line);
            out.push_str(ending);
        }
        out.push_str(ending);
    }
    if let Some(source) = &source {
        out.push_str(&source.before);
    }
    if !out.trim_start_matches(BOM).is_empty() && !out.ends_with('\n') {
        out.push_str(ending);
    }
    let template = source.as_ref().map(|s| &s.template);
    for line in block_lines(request, template, is_first) {
        out.push_str(&line);
        out.push_str(ending);
    }
    if let Some(source) = source
        && !source.after.is_empty()
    {
        out.push_str(&source.after);
    }
    out
}

fn block_lines(request: &Request, template: Option<&Template>, is_first: bool) -> Vec<String> {
    let empty = Template::default();
    let t = template.unwrap_or(&empty);
    let mut lines = Vec::new();
    if let Some(title) = title_line(request, template, is_first) {
        lines.push(title);
    }

    let mut state = State::new(request);
    state.match_lines(t);
    let mut preamble = state.preamble(t);
    let headers = state.headers(t);
    state.insert_leftovers(&mut preamble);
    lines.extend(preamble.into_iter().map(|(_, line)| line));

    if template.is_some_and(|t| t.method == request.method && t.url == request.url) {
        lines.extend(t.request_lines.iter().cloned());
    } else {
        let mut line = one_line(&request.method);
        if !request.url.is_empty() {
            line = format!("{line} {}", one_line(&request.url));
        }
        if let Some(version) = &t.version {
            line = format!("{line} {version}");
        }
        lines.push(line);
    }
    lines.extend(headers);

    if !request.body.is_empty() {
        lines.push(String::new());
        lines.extend(request.body.split('\n').map(str::to_string));
    }
    if !request.trailer.is_empty() {
        lines.push(String::new());
        lines.extend(request.trailer.split('\n').map(str::to_string));
    }
    let blanks_written = lines.iter().rev().take_while(|l| l.is_empty()).count();
    let blanks = t.trailing_blanks.saturating_sub(blanks_written);
    lines.extend(std::iter::repeat_n(String::new(), blanks));
    lines
}

/// The `###` line: the original one while the name is unchanged, so `###` + `# @name login`
/// stays that way. Only the first request may go without one; anywhere else it would join the
/// request before it.
fn title_line(request: &Request, template: Option<&Template>, is_first: bool) -> Option<String> {
    if let Some(t) = template {
        let implied = match t.title.is_empty() {
            true => request_meta::reference_name(request).unwrap_or_default(),
            false => t.title.clone(),
        };
        if implied == request.name && (is_first || t.title_line.is_some()) {
            return t.title_line.clone();
        }
    }
    let name = one_line(&request.name);
    Some(match name.is_empty() {
        true => "###".to_string(),
        false => format!("### {name}"),
    })
}

/// Lines that must stay single lines never get a line break, whatever a caller passes.
pub(crate) fn one_line(text: &str) -> String {
    text.replace(['\r', '\n'], " ")
}
