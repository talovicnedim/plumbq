//! Compares two response bodies: JSON field by field, anything else line by line.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// More changes than this are summarized as "and N more".
const MAX_CHANGES: usize = 100;
/// Values longer than this are cut in change descriptions.
const MAX_VALUE_CHARS: usize = 80;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BodyComparison {
    /// Both bodies are JSON, so `changes` are per field.
    pub json: bool,
    pub changes: Vec<BodyChange>,
    /// Changes left out after the first [`MAX_CHANGES`].
    pub omitted: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BodyChange {
    /// JSON path (`$.items[2].name`) or `line 12`.
    pub path: String,
    pub kind: BodyChangeKind,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum BodyChangeKind {
    Added,
    Removed,
    Changed,
}

pub fn bodies(before: &str, after: &str) -> BodyComparison {
    let mut changes = Vec::new();
    let json = match (
        serde_json::from_str::<Value>(before),
        serde_json::from_str::<Value>(after),
    ) {
        (Ok(a), Ok(b)) => {
            diff_values("$", &a, &b, &mut changes);
            true
        }
        _ => {
            diff_lines(before, after, &mut changes);
            false
        }
    };
    let omitted = changes.len().saturating_sub(MAX_CHANGES);
    changes.truncate(MAX_CHANGES);
    BodyComparison {
        json,
        changes,
        omitted: u32::try_from(omitted).unwrap_or(u32::MAX),
    }
}

fn diff_values(path: &str, a: &Value, b: &Value, out: &mut Vec<BodyChange>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for (key, va) in x {
                let child = format!("{path}{}", key_segment(key));
                match y.get(key) {
                    Some(vb) => diff_values(&child, va, vb, out),
                    None => out.push(change(child, BodyChangeKind::Removed, Some(va), None)),
                }
            }
            for (key, vb) in y.iter().filter(|(k, _)| !x.contains_key(*k)) {
                let child = format!("{path}{}", key_segment(key));
                out.push(change(child, BodyChangeKind::Added, None, Some(vb)));
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            for i in 0..x.len().max(y.len()) {
                let child = format!("{path}[{i}]");
                match (x.get(i), y.get(i)) {
                    (Some(va), Some(vb)) => diff_values(&child, va, vb, out),
                    (Some(va), None) => {
                        out.push(change(child, BodyChangeKind::Removed, Some(va), None))
                    }
                    (None, Some(vb)) => {
                        out.push(change(child, BodyChangeKind::Added, None, Some(vb)))
                    }
                    (None, None) => {}
                }
            }
        }
        _ if a != b => out.push(change(
            path.to_string(),
            BodyChangeKind::Changed,
            Some(a),
            Some(b),
        )),
        _ => {}
    }
}

/// `.name`, or `["odd key"]` when the key isn't a plain identifier.
fn key_segment(key: &str) -> String {
    let plain = !key.is_empty()
        && key.chars().all(|c| c.is_alphanumeric() || c == '_')
        && !key.starts_with(|c: char| c.is_ascii_digit());
    if plain {
        format!(".{key}")
    } else {
        format!("[{}]", Value::String(key.to_string()))
    }
}

fn change(path: String, kind: BodyChangeKind, a: Option<&Value>, b: Option<&Value>) -> BodyChange {
    BodyChange {
        path,
        kind,
        before: a.map(summary),
        after: b.map(summary),
    }
}

fn summary(value: &Value) -> String {
    cut(&value.to_string())
}

fn cut(text: &str) -> String {
    match text.char_indices().nth(MAX_VALUE_CHARS) {
        Some((i, _)) => format!("{}…", &text[..i]),
        None => text.to_string(),
    }
}

/// Lines that differ at the same position, plus lines added or removed at the end. Simple on
/// purpose: the app shows a real side-by-side diff; this is a summary.
fn diff_lines(before: &str, after: &str, out: &mut Vec<BodyChange>) {
    let a: Vec<&str> = before.lines().collect();
    let b: Vec<&str> = after.lines().collect();
    for i in 0..a.len().max(b.len()) {
        let path = format!("line {}", i + 1);
        let (kind, x, y) = match (a.get(i), b.get(i)) {
            (Some(x), Some(y)) if x != y => (BodyChangeKind::Changed, Some(*x), Some(*y)),
            (Some(x), None) => (BodyChangeKind::Removed, Some(*x), None),
            (None, Some(y)) => (BodyChangeKind::Added, None, Some(*y)),
            _ => continue,
        };
        out.push(BodyChange {
            path,
            kind,
            before: x.map(cut),
            after: y.map(cut),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(comparison: &BodyComparison) -> Vec<(String, BodyChangeKind)> {
        comparison
            .changes
            .iter()
            .map(|c| (c.path.clone(), c.kind))
            .collect()
    }

    #[test]
    fn compares_json_by_field() {
        let before = r#"{"user":{"name":"Ada","age":36},"tags":["a","b"],"old":1}"#;
        let after = r#"{"user":{"name":"Grace","age":36},"tags":["a"],"new key":true}"#;
        let comparison = bodies(before, after);

        assert!(comparison.json);
        assert_eq!(
            paths(&comparison),
            vec![
                ("$.user.name".to_string(), BodyChangeKind::Changed),
                ("$.tags[1]".to_string(), BodyChangeKind::Removed),
                ("$.old".to_string(), BodyChangeKind::Removed),
                ("$[\"new key\"]".to_string(), BodyChangeKind::Added),
            ]
        );
        assert_eq!(comparison.changes[0].before.as_deref(), Some("\"Ada\""));
        assert_eq!(comparison.changes[0].after.as_deref(), Some("\"Grace\""));
    }

    #[test]
    fn identical_bodies_have_no_changes() {
        assert!(
            bodies(r#"{"a":[1,2]}"#, r#"{"a":[1,2]}"#)
                .changes
                .is_empty()
        );
    }

    #[test]
    fn compares_text_by_line() {
        let comparison = bodies("ok\nsame\n", "fail\nsame\nextra\n");
        assert!(!comparison.json);
        assert_eq!(
            paths(&comparison),
            vec![
                ("line 1".to_string(), BodyChangeKind::Changed),
                ("line 3".to_string(), BodyChangeKind::Added),
            ]
        );
    }

    #[test]
    fn caps_long_values_and_many_changes() {
        let long = "x".repeat(200);
        let comparison = bodies(&format!("\"{long}\""), "\"y\"");
        assert!(
            comparison.changes[0]
                .before
                .as_deref()
                .is_some_and(|v| v.ends_with('…'))
        );

        let before: Vec<u32> = (0..150).collect();
        let after: Vec<u32> = (1000..1150).collect();
        let many = bodies(
            &serde_json::to_string(&before).unwrap(),
            &serde_json::to_string(&after).unwrap(),
        );
        assert_eq!(many.changes.len(), MAX_CHANGES);
        assert_eq!(many.omitted, 50);
    }
}
