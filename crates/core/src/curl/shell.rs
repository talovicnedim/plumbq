//! Shell quoting for cURL commands: splitting a pasted command into arguments, and quoting
//! values for an exported one.

use crate::{Error, Result};

pub(crate) fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// As is when the shell reads it literally (`POST`, `PURGE`), quoted otherwise: a draft method
/// like `X;id` must not run as a command when the line is pasted.
pub(crate) fn word(text: &str) -> String {
    let literal = !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    match literal {
        true => text.to_string(),
        false => quote(text),
    }
}

/// Splits like a POSIX shell: single and double quotes, backslash escapes, and
/// backslash-newline continuations (as in multi-line copies).
pub(crate) fn tokenize(command: &str) -> Result<Vec<String>> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut chars = command.chars();

    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                in_token = true;
                single_quoted(&mut chars, &mut current)?;
            }
            '"' => {
                in_token = true;
                double_quoted(&mut chars, &mut current)?;
            }
            '\\' => match chars.next() {
                Some('\n') | Some('\r') | None => {}
                Some(c) => {
                    in_token = true;
                    current.push(c);
                }
            },
            c if c.is_whitespace() => {
                if in_token {
                    tokens.push(std::mem::take(&mut current));
                    in_token = false;
                }
            }
            c => {
                in_token = true;
                current.push(c);
            }
        }
    }
    if in_token {
        tokens.push(current);
    }
    Ok(tokens)
}

/// Everything up to the closing `'`, taken as written.
fn single_quoted(chars: &mut std::str::Chars, current: &mut String) -> Result<()> {
    loop {
        match chars.next() {
            Some('\'') => return Ok(()),
            Some(c) => current.push(c),
            None => return Err(Error::Invalid("unclosed ' in curl command".into())),
        }
    }
}

/// Everything up to the closing `"`; a backslash only escapes `"`, `\`, `$` and `` ` ``.
fn double_quoted(chars: &mut std::str::Chars, current: &mut String) -> Result<()> {
    loop {
        match chars.next() {
            Some('"') => return Ok(()),
            Some('\\') => match chars.next() {
                Some(c @ ('"' | '\\' | '$' | '`')) => current.push(c),
                Some(c) => {
                    current.push('\\');
                    current.push(c);
                }
                None => return Ok(()),
            },
            Some(c) => current.push(c),
            None => return Err(Error::Invalid("unclosed \" in curl command".into())),
        }
    }
}
