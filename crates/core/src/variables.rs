//! `{{variable}}` substitution and secret masking, including the JetBrains dynamic variables
//! (`{{$uuid}}`, `{{$timestamp}}`, …).

use std::collections::BTreeMap;

use crate::environments::Values;
use base64::Engine;

use crate::{Error, Result, time};

/// Replaces every `{{name}}` in `text`. Fails listing every unknown variable, so the user can
/// fix them all at once.
pub fn substitute(text: &str, values: &Values) -> Result<String> {
    let mut out = String::with_capacity(text.len());
    let mut missing: Vec<String> = Vec::new();
    let mut last = 0;

    for reference in references(text) {
        out.push_str(&text[last..reference.start]);
        match lookup(reference.name, values) {
            Some(value) => out.push_str(&value),
            None if !missing.iter().any(|m| m == reference.name) => {
                missing.push(reference.name.to_string())
            }
            None => {}
        }
        last = reference.end;
    }
    out.push_str(&text[last..]);

    if !missing.is_empty() {
        return Err(Error::Invalid(format!(
            "unknown variables: {}",
            missing.join(", ")
        )));
    }
    Ok(out)
}

/// Values made up at send time: a fresh uuid, the time, a random number.
const BUILTINS: [&str; 5] = [
    "$uuid",
    "$random.uuid",
    "$timestamp",
    "$isoTimestamp",
    "$randomInt",
];

pub(crate) fn is_builtin(name: &str) -> bool {
    BUILTINS.contains(&name)
}

fn lookup(name: &str, values: &Values) -> Option<String> {
    if let Some(value) = values.secrets.get(name).or(values.variables.get(name)) {
        return Some(value.clone());
    }
    if !is_builtin(name) {
        return None;
    }
    let uuid = uuid::Uuid::new_v4();
    match name {
        "$uuid" | "$random.uuid" => Some(uuid.to_string()),
        "$timestamp" => Some(time::now().0.to_string()),
        "$isoTimestamp" => Some(time::format(time::now())),
        // Below 1000, like the JetBrains client; random bits come from the v4 uuid.
        "$randomInt" => Some((uuid.as_u128() % 1000).to_string()),
        _ => None,
    }
}

/// Secret values shorter than this aren't masked: replacing every `1` or `en` in a response
/// would corrupt it (see [`short_secrets`]).
pub const MIN_MASK_LEN: usize = 4;

/// Replaces every secret value in `text` with `{{name}}`. Applied to everything that is
/// returned to the UI or an agent, or written to history. Encoded forms are masked too: JSON
/// escaped (`a\/b`), percent-encoded, base64, and Basic auth credentials containing a secret.
/// One pass over the original text, so a placeholder is never matched by a later secret.
pub fn mask(text: &str, secrets: &BTreeMap<String, String>) -> String {
    let masked = mask_bytes(text.as_bytes(), secrets);
    // Only whole UTF-8 sequences are replaced, by ASCII, so the result stays valid UTF-8.
    String::from_utf8(masked).unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into())
}

/// [`mask`] for bytes, e.g. a binary response body that echoes a token.
pub fn mask_bytes(bytes: &[u8], secrets: &BTreeMap<String, String>) -> Vec<u8> {
    let patterns = patterns(secrets);
    let mut ranges: Vec<(usize, usize, &str)> = Vec::new();
    for (pattern, name) in &patterns {
        let mut from = 0;
        while let Some(at) = find(bytes, pattern.as_bytes(), from) {
            ranges.push((at, at + pattern.len(), name));
            from = at + pattern.len();
        }
    }
    ranges.extend(basic_auth_ranges(bytes, secrets));
    // Leftmost first; at the same start the longest match wins.
    ranges.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    let mut out = Vec::with_capacity(bytes.len());
    let mut pos = 0;
    for (start, end, name) in ranges {
        if start < pos {
            continue;
        }
        out.extend_from_slice(&bytes[pos..start]);
        out.extend_from_slice(format!("{{{{{name}}}}}").as_bytes());
        pos = end;
    }
    out.extend_from_slice(&bytes[pos..]);
    out
}

/// Names of secrets too short to mask, for a warning (never log the values).
pub fn short_secrets(secrets: &BTreeMap<String, String>) -> Vec<&str> {
    secrets
        .iter()
        .filter(|(_, v)| !v.is_empty() && v.chars().count() < MIN_MASK_LEN)
        .map(|(name, _)| name.as_str())
        .collect()
}

/// Every form a secret can take in a request or response, with its name.
fn patterns(secrets: &BTreeMap<String, String>) -> Vec<(String, &str)> {
    let mut out: Vec<(String, &str)> = Vec::new();
    for (name, value) in secrets {
        if value.chars().count() < MIN_MASK_LEN {
            continue;
        }
        let json = serde_json::Value::String(value.clone()).to_string();
        let json = json[1..json.len() - 1].to_string();
        let percent = percent_encode(value);
        let b64 = base64::engine::general_purpose::STANDARD.encode(value);
        let b64_url = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(value);
        let variants = [
            value.clone(),
            json.replace('/', "\\/"),
            json,
            percent.clone(),
            lowercase_hex(&percent),
            percent.replace("%20", "+"),
            b64.trim_end_matches('=').to_string(),
            b64_url,
        ];
        for variant in variants {
            if variant.len() >= MIN_MASK_LEN && !out.iter().any(|(p, _)| *p == variant) {
                out.push((variant, name));
            }
        }
    }
    out
}

/// `Basic <base64>` credentials whose decoded `user:password` contains a secret.
fn basic_auth_ranges<'a>(
    bytes: &[u8],
    secrets: &'a BTreeMap<String, String>,
) -> Vec<(usize, usize, &'a str)> {
    let mut ranges = Vec::new();
    for prefix in [&b"Basic "[..], &b"basic "[..]] {
        let mut from = 0;
        while let Some(at) = find(bytes, prefix, from) {
            let start = at + prefix.len();
            let end = start
                + bytes[start..]
                    .iter()
                    .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
                    .count();
            from = end.max(start);
            let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(&bytes[start..end])
            else {
                continue;
            };
            let decoded = String::from_utf8_lossy(&decoded);
            if let Some((name, _)) = secrets
                .iter()
                .find(|(_, v)| v.chars().count() >= MIN_MASK_LEN && decoded.contains(v.as_str()))
            {
                ranges.push((start, end, name.as_str()));
            }
        }
    }
    ranges
}

/// Upper-case percent-encoding of everything but unreserved characters.
fn percent_encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// `%2F` → `%2f`, leaving everything else alone.
fn lowercase_hex(encoded: &str) -> String {
    let mut out = String::with_capacity(encoded.len());
    let mut chars = encoded.chars();
    while let Some(c) = chars.next() {
        out.push(c);
        if c == '%' {
            out.extend(chars.by_ref().take(2).map(|h| h.to_ascii_lowercase()));
        }
    }
    out
}

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from >= haystack.len() {
        return None;
    }
    haystack[from..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|i| from + i)
}

/// Names of the `{{variables}}` used in `text`, in order of first use.
pub fn used(text: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for reference in references(text) {
        if !names.iter().any(|n| n == reference.name) {
            names.push(reference.name.to_string());
        }
    }
    names
}

struct Reference<'a> {
    start: usize,
    end: usize,
    name: &'a str,
}

/// The `{{name}}` references in `text`. One never spans a line or holds another brace, so a
/// `{{` the user is still typing doesn't swallow the URL, body and headers after it.
fn references(text: &str) -> Vec<Reference<'_>> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(offset) = text[from..].find("{{") {
        let start = from + offset;
        let Some(len) = text[start + 2..].find("}}") else {
            break;
        };
        let name = &text[start + 2..start + 2 + len];
        if name.contains(['{', '}', '\n', '\r']) {
            from = start + 1;
            continue;
        }
        let end = start + 2 + len + 2;
        found.push(Reference {
            start,
            end,
            name: name.trim(),
        });
        from = end;
    }
    found
}

#[cfg(test)]
#[path = "variables_tests.rs"]
mod tests;
