//! Runs the `# @assert` lines of a request against its response. Syntax: `docs/formats.md`
//! (Assertions).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::environments::Values;
use crate::send::Response;
use crate::{json_path, variables};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AssertionResult {
    pub expression: String,
    pub passed: bool,
    /// What was found, e.g. "got 404"; for failures also what was expected.
    pub message: String,
}

/// Evaluates every expression. Run it on the masked response, with the values the request was
/// sent with, so secret values never end up in a message.
pub fn run(expressions: &[String], response: &Response, values: &Values) -> Vec<AssertionResult> {
    let placeholders = Values {
        secrets: values
            .secrets
            .keys()
            .map(|name| (name.clone(), format!("{{{{{name}}}}}")))
            .collect(),
        variables: values.variables.clone(),
        tag: values.tag,
    };
    expressions
        .iter()
        .map(|expression| {
            let outcome = variables::substitute(expression, &placeholders)
                .map_err(|err| err.to_string())
                .and_then(|resolved| evaluate(&resolved, response));
            let (passed, message) = match outcome {
                Ok(outcome) => outcome,
                Err(problem) => (false, problem),
            };
            AssertionResult {
                expression: expression.clone(),
                passed,
                message,
            }
        })
        .collect()
}

fn evaluate(expression: &str, response: &Response) -> Result<(bool, String), String> {
    let (subject, rest) = split_subject(expression.trim())?;
    let (operator, expected) = split_operator(rest)?;
    let actual = subject_value(&subject, response)?;

    if operator == "exists" {
        return Ok(match actual {
            Some(value) => (true, format!("found {}", show(&value))),
            None => (false, format!("{subject} is missing")),
        });
    }
    let Some(actual) = actual else {
        return Ok((false, format!("{subject} is missing")));
    };
    let passed = compare(&actual, operator, expected)?;
    let message = if passed {
        format!("got {}", show(&actual))
    } else {
        format!(
            "expected {subject} {operator} {expected}, got {}",
            show(&actual)
        )
    };
    Ok((passed, message))
}

/// Splits off the subject: `status`, `body.$.a["b c"]`, or the two words `header Name`.
fn split_subject(expression: &str) -> Result<(String, &str), String> {
    let end = word_end(expression);
    let first = &expression[..end];
    let rest = expression[end..].trim_start();
    if first != "header" {
        return Ok((first.to_string(), rest));
    }
    let name_end = word_end(rest);
    if name_end == 0 {
        return Err("header needs a name, e.g. `header Content-Type exists`".into());
    }
    Ok((
        format!("header {}", &rest[..name_end]),
        rest[name_end..].trim_start(),
    ))
}

/// End of the first word, treating spaces inside `[...]` as part of it.
fn word_end(text: &str) -> usize {
    let mut depth = 0;
    for (i, c) in text.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            c if c.is_whitespace() && depth <= 0 => return i,
            _ => {}
        }
    }
    text.len()
}

const OPERATORS: [&str; 12] = [
    "==",
    "!=",
    "<=",
    ">=",
    "<",
    ">",
    "contains",
    "startsWith",
    "endsWith",
    "exists",
    "in",
    "matches",
];

fn split_operator(rest: &str) -> Result<(&'static str, &str), String> {
    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    let word = &rest[..end];
    let Some(operator) = OPERATORS.iter().copied().find(|op| *op == word) else {
        return Err(format!(
            "unknown operator {word:?}; use ==, !=, <, <=, >, >=, contains, startsWith, \
             endsWith, exists or in"
        ));
    };
    if operator == "matches" {
        return Err("matches isn't supported yet; use contains, startsWith or endsWith".into());
    }
    let expected = rest[end..].trim();
    if operator != "exists" && expected.is_empty() {
        return Err(format!("{operator} needs a value"));
    }
    Ok((operator, expected))
}

fn subject_value(subject: &str, response: &Response) -> Result<Option<Value>, String> {
    if let Some(name) = subject.strip_prefix("header ") {
        return Ok(response
            .headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(name))
            .map(|h| Value::from(h.value.clone())));
    }
    if let Some(path) = subject.strip_prefix("body.") {
        let json: Value = serde_json::from_str(&response.body)
            .map_err(|_| format!("{subject}: the body isn't JSON"))?;
        if !path.starts_with('$') {
            return Err(format!("{subject}: paths start with $, e.g. body.$.id"));
        }
        return Ok(json_path::get(&json, path));
    }
    match subject {
        "status" => Ok(Some(Value::from(response.status))),
        "duration" => Ok(Some(Value::from(response.duration_ms))),
        "size" => Ok(Some(Value::from(response.size))),
        "body" => Ok(Some(Value::from(response.body.clone()))),
        other => Err(format!(
            "unknown subject {other:?}; use status, duration, size, header <name>, body or \
             body.$.path"
        )),
    }
}

fn compare(actual: &Value, operator: &str, expected: &str) -> Result<bool, String> {
    if operator == "in" {
        let (low, high) = expected
            .split_once("..")
            .and_then(|(a, b)| Some((a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?)))
            .ok_or_else(|| format!("in needs a range like 200..299, got {expected}"))?;
        let value = number(actual).ok_or_else(|| "in needs a number".to_string())?;
        return Ok(value >= low && value <= high);
    }
    let expected = literal(expected);
    match operator {
        "==" => Ok(equal(actual, &expected)),
        "!=" => Ok(!equal(actual, &expected)),
        "<" | "<=" | ">" | ">=" => {
            let (a, b) = number(actual)
                .zip(number(&expected))
                .ok_or_else(|| format!("{operator} compares numbers"))?;
            Ok(match operator {
                "<" => a < b,
                "<=" => a <= b,
                ">" => a > b,
                _ => a >= b,
            })
        }
        "contains" => Ok(match actual {
            Value::Array(items) => items.iter().any(|item| equal(item, &expected)),
            other => text(other).contains(&text(&expected)),
        }),
        "startsWith" => Ok(text(actual).starts_with(&text(&expected))),
        _ => Ok(text(actual).ends_with(&text(&expected))),
    }
}

/// Parses a written value: number, quoted string, true/false/null, or a bare word.
fn literal(text: &str) -> Value {
    let quoted = text
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| text.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')));
    if let Some(inner) = quoted {
        return Value::from(inner);
    }
    match text {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        "null" => Value::Null,
        _ => text
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or_else(|| Value::from(text)),
    }
}

/// Numbers compare by value (so 1 == 1.0); a string and a number compare as text, so
/// `header X-Count == 3` works.
fn equal(actual: &Value, expected: &Value) -> bool {
    if let (Some(a), Some(b)) = (number(actual), number(expected)) {
        return a == b;
    }
    match (actual, expected) {
        (Value::String(_), _) | (_, Value::String(_)) => text(actual) == text(expected),
        _ => actual == expected,
    }
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Short display for messages: long strings and documents are cut.
fn show(value: &Value) -> String {
    let full = value.to_string();
    match full.char_indices().nth(80) {
        Some((cut, _)) => format!("{}…", &full[..cut]),
        None => full,
    }
}

#[cfg(test)]
#[path = "assertions_tests.rs"]
mod tests;
