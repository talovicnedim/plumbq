//! Matches an edited request's values to the lines it was read from, so unchanged values keep
//! their line (see [`super::template`]).

use super::read;
use super::template::{Line, Template, one_line};
use crate::http_file::{FileVariable, Request};
use crate::request_settings::RequestSettings;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Variable,
    Setting,
    Comment,
    Assertion,
    Header,
    Other,
}

/// Which current values the template's lines match, and which have been written.
pub(crate) struct State<'a> {
    request: &'a Request,
    /// For each preamble / header template line: the matched index into the current list.
    preamble_matches: Vec<Option<usize>>,
    header_matches: Vec<Option<usize>>,
    written: [Vec<bool>; 4],
    /// Current values some template line matched; they are written at that line's place.
    matched: [Vec<bool>; 4],
    settings_written: Vec<&'static str>,
}

const VARIABLES: usize = 0;
const COMMENTS: usize = 1;
const ASSERTIONS: usize = 2;
const HEADERS: usize = 3;

impl<'a> State<'a> {
    pub(crate) fn new(request: &'a Request) -> Self {
        Self {
            request,
            preamble_matches: Vec::new(),
            header_matches: Vec::new(),
            written: lists(request),
            matched: lists(request),
            settings_written: Vec::new(),
        }
    }

    /// Matches template lines to current values in order, so reordering isn't silently undone.
    pub(crate) fn match_lines(&mut self, t: &Template) {
        let r = self.request;
        let mut next = [0usize; 4];
        let matched = &mut self.matched;
        let mut find = |list: usize, len: usize, same: &dyn Fn(usize) -> bool| -> Option<usize> {
            let found = (next[list]..len).find(|&i| same(i))?;
            next[list] = found + 1;
            matched[list][found] = true;
            Some(found)
        };
        let mut match_line = |line: &Line| -> Option<usize> {
            match line {
                Line::Variable { variable, .. } => find(VARIABLES, r.variables.len(), &|i| {
                    r.variables[i].name == variable.name
                }),
                Line::Comment { raw } => {
                    find(COMMENTS, r.comments.len(), &|i| r.comments[i] == raw.trim())
                }
                Line::Assertion { expr, .. } => find(ASSERTIONS, r.assertions.len(), &|i| {
                    r.assertions[i] == *expr
                }),
                Line::Header { header, .. } => {
                    find(HEADERS, r.headers.len(), &|i| r.headers[i] == *header)
                }
                Line::Blank | Line::Setting { .. } => None,
            }
        };
        let preamble_matches = t.preamble.iter().map(&mut match_line).collect();
        let header_matches = t
            .headers
            .iter()
            .map(|line| match line {
                // Comments among the headers aren't part of `comments`; they're always kept.
                Line::Comment { .. } => None,
                other => match_line(other),
            })
            .collect();
        self.preamble_matches = preamble_matches;
        self.header_matches = header_matches;
    }

    pub(crate) fn preamble(&mut self, t: &Template) -> Vec<(Kind, String)> {
        let mut out = Vec::new();
        for (line, matched) in t.preamble.iter().zip(self.preamble_matches.clone()) {
            match (line, matched) {
                (Line::Blank, _) => out.push((Kind::Other, String::new())),
                (Line::Setting { raw }, _) => {
                    if let Some(text) = self.setting(raw) {
                        out.push((Kind::Setting, text));
                    }
                }
                (Line::Variable { raw, .. }, Some(i)) => {
                    self.pending(VARIABLES, i, &mut out);
                    let current = &self.request.variables[i];
                    let unchanged =
                        matches!(line, Line::Variable { variable, .. } if variable == current);
                    let text = match unchanged {
                        true => raw.clone(),
                        false => variable_line(current),
                    };
                    self.written[VARIABLES][i] = true;
                    out.push((Kind::Variable, text));
                }
                (Line::Comment { raw }, Some(i)) => {
                    self.pending(COMMENTS, i, &mut out);
                    self.written[COMMENTS][i] = true;
                    out.push((Kind::Comment, raw.clone()));
                }
                (Line::Assertion { raw, .. }, Some(i)) => {
                    self.pending(ASSERTIONS, i, &mut out);
                    self.written[ASSERTIONS][i] = true;
                    out.push((Kind::Assertion, raw.clone()));
                }
                _ => {}
            }
        }
        out
    }

    pub(crate) fn headers(&mut self, t: &Template) -> Vec<String> {
        let mut out: Vec<(Kind, String)> = Vec::new();
        for (line, matched) in t.headers.iter().zip(self.header_matches.clone()) {
            match (line, matched) {
                (Line::Comment { raw }, _) => out.push((Kind::Other, raw.clone())),
                (Line::Header { raw, .. }, Some(i)) => {
                    self.pending(HEADERS, i, &mut out);
                    self.written[HEADERS][i] = true;
                    out.push((Kind::Header, raw.clone()));
                }
                (Line::Assertion { raw, .. }, Some(i)) => {
                    self.pending(ASSERTIONS, i, &mut out);
                    self.written[ASSERTIONS][i] = true;
                    out.push((Kind::Assertion, raw.clone()));
                }
                _ => {}
            }
        }
        let leftovers: Vec<(Kind, String)> = (0..self.request.headers.len())
            .filter(|&i| !self.written[HEADERS][i])
            .map(|i| (Kind::Header, self.text(HEADERS, i)))
            .collect();
        for i in 0..self.request.headers.len() {
            self.written[HEADERS][i] = true;
        }
        insert_after_last(&mut out, Kind::Header, leftovers);
        out.into_iter().map(|(_, line)| line).collect()
    }

    /// Writes current values of `list` before index `upto` that no template line matched, so a
    /// new value lands right before the existing one that follows it.
    fn pending(&mut self, list: usize, upto: usize, out: &mut Vec<(Kind, String)>) {
        let kind = kind_of(list);
        for i in 0..upto {
            if !self.written[list][i] && !self.is_matched(list, i) {
                self.written[list][i] = true;
                out.push((kind, self.text(list, i)));
            }
        }
    }

    fn is_matched(&self, list: usize, index: usize) -> bool {
        self.matched[list][index]
    }

    fn text(&self, list: usize, i: usize) -> String {
        let r = self.request;
        match list {
            VARIABLES => variable_line(&r.variables[i]),
            COMMENTS => one_line(&r.comments[i]),
            ASSERTIONS => format!("# @assert {}", one_line(&r.assertions[i])),
            _ => format!(
                "{}: {}",
                one_line(&r.headers[i].name),
                one_line(&r.headers[i].value)
            ),
        }
    }

    /// The setting line for a template setting line: as written while its value is unchanged,
    /// rewritten when changed, gone when removed.
    fn setting(&mut self, raw: &str) -> Option<String> {
        let directive = read::comment_text(raw.trim())?;
        let key = RequestSettings::key_of(directive)?;
        if self.settings_written.contains(&key) {
            return None;
        }
        self.settings_written.push(key);
        match self.request.settings.agrees(directive) {
            true => Some(raw.to_string()),
            false => self.request.settings.line_for(key),
        }
    }

    /// Values no template line placed: after the last line of their kind, or before the
    /// preamble's trailing blank lines.
    pub(crate) fn insert_leftovers(&mut self, out: &mut Vec<(Kind, String)>) {
        let settings: Vec<(Kind, String)> = self
            .request
            .settings
            .lines()
            .into_iter()
            .filter(|line| {
                read::comment_text(line)
                    .and_then(RequestSettings::key_of)
                    .is_some_and(|key| !self.settings_written.contains(&key))
            })
            .map(|line| (Kind::Setting, line))
            .collect();
        insert_after_last(out, Kind::Setting, settings);
        for list in [VARIABLES, COMMENTS, ASSERTIONS] {
            let kind = kind_of(list);
            let len = self.written[list].len();
            let rest: Vec<(Kind, String)> = (0..len)
                .filter(|&i| !self.written[list][i])
                .map(|i| (kind, self.text(list, i)))
                .collect();
            insert_after_last(out, kind, rest);
        }
    }
}

fn lists(request: &Request) -> [Vec<bool>; 4] {
    [
        vec![false; request.variables.len()],
        vec![false; request.comments.len()],
        vec![false; request.assertions.len()],
        vec![false; request.headers.len()],
    ]
}

fn kind_of(list: usize) -> Kind {
    match list {
        VARIABLES => Kind::Variable,
        COMMENTS => Kind::Comment,
        ASSERTIONS => Kind::Assertion,
        _ => Kind::Header,
    }
}

fn insert_after_last(out: &mut Vec<(Kind, String)>, kind: Kind, lines: Vec<(Kind, String)>) {
    if lines.is_empty() {
        return;
    }
    let at = match out.iter().rposition(|(k, _)| *k == kind) {
        Some(last) => last + 1,
        // Before trailing blank lines, which separate notes from the request line.
        None => out
            .iter()
            .rposition(|(k, line)| *k != Kind::Other || !line.is_empty())
            .map_or(0, |i| i + 1),
    };
    out.splice(at..at, lines);
}

fn variable_line(variable: &FileVariable) -> String {
    format!(
        "@{} = {}",
        one_line(&variable.name),
        one_line(&variable.value)
    )
}
