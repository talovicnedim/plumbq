//! Variables from outside the environment (`$prompt`, `$processEnv`, `$dotenv`), resolved at
//! send time. Syntax: `docs/formats.md`.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::environments::Values;
use crate::secret_names::looks_secret;
use crate::{Error, Result, variables};

const DOTENV_FILE: &str = ".env";

#[derive(Debug, PartialEq, Eq)]
enum Source<'a> {
    Prompt(&'a str),
    ProcessEnv(&'a str),
    Dotenv(&'a str),
}

fn parse(name: &str) -> Option<Source<'_>> {
    if let Some(var) = name.strip_prefix("$env.") {
        return Some(Source::ProcessEnv(var.trim()));
    }
    let (kind, arg) = name.split_once(char::is_whitespace)?;
    let arg = arg.trim();
    match kind {
        "$prompt" => Some(Source::Prompt(arg)),
        "$processEnv" => Some(Source::ProcessEnv(arg)),
        "$dotenv" => Some(Source::Dotenv(arg)),
        _ => None,
    }
}

fn is_dynamic(name: &str) -> bool {
    parse(name).is_some()
}

/// Where a dynamic variable comes from, for the editor's hints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DynamicKind {
    Prompt,
    ProcessEnv,
    Dotenv,
}

pub(crate) fn kind(name: &str) -> Option<DynamicKind> {
    match parse(name)? {
        Source::Prompt(_) => Some(DynamicKind::Prompt),
        Source::ProcessEnv(_) => Some(DynamicKind::ProcessEnv),
        Source::Dotenv(_) => Some(DynamicKind::Dotenv),
    }
}

/// Names of the `{{$prompt name}}` variables in `text`, in order of first use.
pub fn prompts(text: &str) -> Vec<String> {
    variables::used(text)
        .iter()
        .filter_map(|name| match parse(name) {
            Some(Source::Prompt(prompt)) => Some(prompt.to_string()),
            _ => None,
        })
        .collect()
}

/// Adds the value of every dynamic variable used in `text` to `values`. Prompt answers come
/// from `answers` (keyed by prompt name); a missing answer fails with
/// [`Error::NeedsInput`] before anything is sent. Unknown OS or `.env` variables are left out,
/// so substitution reports them as unknown. `env` reads the process environment (a parameter so
/// tests don't have to change it).
pub fn resolve(
    text: &str,
    root: &Path,
    answers: &BTreeMap<String, String>,
    env: &dyn Fn(&str) -> Option<String>,
    values: &mut Values,
) -> Result<()> {
    let missing: Vec<String> = prompts(text)
        .into_iter()
        .filter(|prompt| !answers.contains_key(prompt))
        .collect();
    if !missing.is_empty() {
        return Err(Error::NeedsInput(missing));
    }

    let mut dotenv: Option<BTreeMap<String, String>> = None;
    for name in variables::used(text) {
        let Some(source) = parse(&name) else { continue };
        // Prompt answers and `.env` values are usually credentials; mask them all.
        let (value, secret) = match source {
            Source::Prompt(prompt) => (answers.get(prompt).cloned(), true),
            Source::ProcessEnv(var) => {
                let value = env(var);
                let secret = looks_secret(var) || value.as_deref().is_some_and(has_userinfo);
                (value, secret)
            }
            Source::Dotenv(var) => {
                let file = match dotenv.take() {
                    Some(file) => file,
                    None => read_dotenv(root)?,
                };
                let value = file.get(var).cloned();
                dotenv = Some(file);
                (value, true)
            }
        };
        let Some(value) = value else { continue };
        let target = if secret {
            &mut values.secrets
        } else {
            &mut values.variables
        };
        target.insert(name, value);
    }
    Ok(())
}

/// Leaves every variable resolved at send time (dynamic, chained responses, OAuth tokens) in
/// `text` as `{{name}}`, e.g. for "copy as cURL" where nothing should be asked, read or fetched.
pub fn keep_as_placeholders(text: &str, values: &mut Values) {
    let resolved_at_send = |name: &String| {
        is_dynamic(name)
            || crate::chain::is_reference(name)
            || crate::oauth::client_name(name).is_some()
    };
    for name in variables::used(text).into_iter().filter(resolved_at_send) {
        values
            .secrets
            .insert(name.clone(), format!("{{{{{name}}}}}"));
    }
}

/// A URL with credentials in it, like `DATABASE_URL=postgres://user:pw@host/db`.
fn has_userinfo(value: &str) -> bool {
    value
        .split_once("://")
        .and_then(|(_, rest)| rest.split('/').next())
        .is_some_and(|authority| authority.contains('@'))
}

fn read_dotenv(root: &Path) -> Result<BTreeMap<String, String>> {
    if !root.join(DOTENV_FILE).exists() {
        return Ok(BTreeMap::new());
    }
    // Symlink-safe: a `.env` linking outside the workspace is refused.
    let path = crate::workspace::resolve_existing(root, DOTENV_FILE)?;
    Ok(parse_dotenv(&fs::read_to_string(path)?))
}

/// `KEY=VALUE` lines; `#` comments, blank lines and a leading `export ` are allowed.
fn parse_dotenv(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let line = line.strip_prefix("export ").unwrap_or(line);
            let (key, value) = line.split_once('=')?;
            Some((key.trim().to_string(), dotenv_value(value).to_string()))
        })
        .collect()
}

/// A quoted value ends at its closing quote; an unquoted one at a `#` after a space, which
/// starts a comment (`KEY=abc # note`), as in most dotenv tools.
fn dotenv_value(raw: &str) -> &str {
    let value = raw.trim_start();
    for quote in ['"', '\''] {
        let quoted = value
            .strip_prefix(quote)
            .and_then(|rest| rest.find(quote).map(|end| &rest[..end]));
        if let Some(inner) = quoted {
            return inner;
        }
    }
    let comment = raw
        .char_indices()
        .find(|(i, c)| c.is_whitespace() && raw[i + c.len_utf8()..].starts_with('#'))
        .map_or(raw.len(), |(i, _)| i);
    raw[..comment].trim()
}

#[cfg(test)]
#[path = "dynamic_tests.rs"]
mod tests;
