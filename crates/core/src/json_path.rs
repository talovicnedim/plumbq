//! The small JSON path subset used by assertions, chaining and the response filter: see
//! `docs/formats.md` (JSON paths).

use serde_json::Value;

/// The value at `path`, or `None` when the path is malformed or doesn't exist.
/// `.length` gives the length of an array, string or object (unless the object has a real
/// `length` key, which wins).
pub fn get(value: &Value, path: &str) -> Option<Value> {
    let mut current = value.clone();
    for segment in segments(path.trim())? {
        current = step(&current, &segment)?;
    }
    Some(current)
}

/// Like [`get`], plus `[*]` / `.*` wildcards over array items or object values. With a
/// wildcard the result is an array of every match (paths missing in some items are skipped);
/// without one it is the single value. `None` when the path is malformed or nothing matches.
pub fn query(value: &Value, path: &str) -> Option<Value> {
    let segments = segments(path.trim())?;
    let wildcard = segments.iter().any(|s| matches!(s, Segment::Wildcard));
    let mut current = vec![value.clone()];
    for segment in &segments {
        current = current
            .iter()
            .flat_map(|v| match segment {
                Segment::Wildcard => match v {
                    Value::Array(items) => items.clone(),
                    Value::Object(map) => map.values().cloned().collect(),
                    _ => Vec::new(),
                },
                other => step(v, other).into_iter().collect(),
            })
            .collect();
    }
    match (wildcard, current.len()) {
        (true, _) => Some(Value::Array(current)),
        (false, 1) => current.pop(),
        _ => None,
    }
}

enum Segment {
    Key(String),
    Index(usize),
    /// `*` or `[*]`: every item of an array or value of an object (only in [`query`]).
    Wildcard,
}

fn step(value: &Value, segment: &Segment) -> Option<Value> {
    match segment {
        Segment::Wildcard => None,
        Segment::Index(i) => value.as_array()?.get(*i).cloned(),
        Segment::Key(key) => {
            if let Some(found) = value.as_object().and_then(|o| o.get(key)) {
                return Some(found.clone());
            }
            if key != "length" {
                return None;
            }
            let len = match value {
                Value::Array(a) => a.len(),
                Value::String(s) => s.chars().count(),
                Value::Object(o) => o.len(),
                _ => return None,
            };
            Some(Value::from(len))
        }
    }
}

fn segments(path: &str) -> Option<Vec<Segment>> {
    let mut rest = path.strip_prefix('$')?;
    let mut out = Vec::new();
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('.') {
            let end = after.find(['.', '[']).unwrap_or(after.len());
            if end == 0 {
                return None;
            }
            out.push(match &after[..end] {
                "*" => Segment::Wildcard,
                key => Segment::Key(key.to_string()),
            });
            rest = &after[end..];
        } else {
            let after = rest.strip_prefix('[')?;
            let close = after.find(']')?;
            let inner = after[..close].trim();
            let quoted = inner
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .or_else(|| inner.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')));
            match quoted {
                Some(key) => out.push(Segment::Key(key.to_string())),
                None if inner == "*" => out.push(Segment::Wildcard),
                None => out.push(Segment::Index(inner.parse().ok()?)),
            }
            rest = &after[close + 1..];
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn doc() -> Value {
        json!({"user": {"id": 1, "full name": "Ada"}, "items": [{"name": "a"}, {"name": "b"}], "s": "héllo"})
    }

    #[test]
    fn walks_keys_indexes_and_quoted_keys() {
        assert_eq!(get(&doc(), "$.user.id"), Some(json!(1)));
        assert_eq!(get(&doc(), "$.items[1].name"), Some(json!("b")));
        assert_eq!(get(&doc(), "$.user[\"full name\"]"), Some(json!("Ada")));
        assert_eq!(get(&doc(), "$"), Some(doc()));
    }

    #[test]
    fn length_of_arrays_strings_and_objects() {
        assert_eq!(get(&doc(), "$.items.length"), Some(json!(2)));
        assert_eq!(get(&doc(), "$.s.length"), Some(json!(5)));
        assert_eq!(get(&doc(), "$.user.length"), Some(json!(2)));
    }

    #[test]
    fn missing_or_malformed_paths_are_none() {
        assert_eq!(get(&doc(), "$.nope"), None);
        assert_eq!(get(&doc(), "$.items[5]"), None);
        assert_eq!(get(&doc(), "user.id"), None);
        assert_eq!(get(&doc(), "$..x"), None);
        assert_eq!(get(&doc(), "$.items[x]"), None);
    }

    #[test]
    fn query_expands_wildcards() {
        let value = doc();
        assert_eq!(query(&value, "$.items[*].name"), Some(json!(["a", "b"])));
        assert_eq!(query(&value, "$.items.*.name"), Some(json!(["a", "b"])));
        assert_eq!(query(&value, "$.user.*"), Some(json!([1, "Ada"])));
        assert_eq!(query(&value, "$.items[*].missing"), Some(json!([])));
    }

    #[test]
    fn query_without_wildcards_matches_get() {
        let value = doc();
        assert_eq!(
            query(&value, "$.items[1].name"),
            get(&value, "$.items[1].name")
        );
        assert_eq!(query(&value, "$.nope"), None);
        assert_eq!(
            get(&value, "$.items[*].name"),
            None,
            "get stays wildcard-free"
        );
    }
}
