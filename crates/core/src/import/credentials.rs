//! Keeps credentials out of imported files: literal tokens become `{{secret}}` placeholders and
//! secret-looking variables become secret names.

use std::collections::BTreeMap;

use super::camel;
use crate::http_file::{Header, Request};
use crate::secret_names::looks_secret;

/// Header names whose whole value is a credential, and the variable that replaces it.
const KEY_HEADERS: [(&str, &str); 6] = [
    ("x-api-key", "apiKey"),
    ("api-key", "apiKey"),
    ("apikey", "apiKey"),
    ("x-auth-token", "authToken"),
    ("x-access-token", "accessToken"),
    ("private-token", "privateToken"),
];

/// Secret names found so far and the values they replaced, so equal values share a name and
/// different values get `token2`, `token3`, …. Values the source declared as credentials (an
/// API key in its auth settings) are replaced wherever they appear, whatever the field is called.
#[derive(Debug, Default)]
pub struct Found {
    values: BTreeMap<String, String>,
    declared: Vec<String>,
}

impl Found {
    pub fn with_declared(declared: Vec<String>) -> Self {
        Self {
            values: BTreeMap::new(),
            declared,
        }
    }

    pub fn names(&self) -> Vec<String> {
        self.values.keys().cloned().collect()
    }

    pub fn into_values(self) -> BTreeMap<String, String> {
        self.values
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    fn name_for(&mut self, base: &str, value: &str) -> String {
        let mut name = base.to_string();
        let mut n = 2;
        loop {
            match self.values.get(&name) {
                Some(existing) if existing == value => return name,
                Some(_) => {
                    name = format!("{base}{n}");
                    n += 1;
                }
                None => {
                    self.values.insert(name.clone(), value.to_string());
                    return name;
                }
            }
        }
    }

    /// The `{{secret}}` for `value` of the field `name`, or `None` when it isn't a credential.
    fn replace(&mut self, name: &str, value: &str) -> Option<String> {
        let literal = !value.is_empty() && !value.contains("{{");
        if !literal || !(looks_secret(name) || self.declared.iter().any(|d| d == value)) {
            return None;
        }
        let var = self.name_for(&variable_name(name), value);
        Some(format!("{{{{{var}}}}}"))
    }
}

/// The header value with its literal credential replaced by a placeholder, or `None` when it
/// holds no literal credential.
pub fn placeholder(name: &str, value: &str, found: &mut Found) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.contains("{{") {
        return None;
    }
    let lower = name.to_ascii_lowercase();
    if lower == "authorization" || lower == "proxy-authorization" {
        let (scheme, rest) = value.split_once(' ').unwrap_or(("", value));
        let base = match scheme.to_ascii_lowercase().as_str() {
            "basic" => "basicAuth",
            "" => "authorization",
            _ => "token",
        };
        let var = found.name_for(base, rest.trim());
        return Some(match scheme {
            "" => format!("{{{{{var}}}}}"),
            _ => format!("{scheme} {{{{{var}}}}}"),
        });
    }
    if let Some((_, base)) = KEY_HEADERS.iter().find(|(header, _)| *header == lower) {
        let var = found.name_for(base, value);
        return Some(format!("{{{{{var}}}}}"));
    }
    found.replace(name, value)
}

/// Scrubs a request's headers, query parameters and body fields (form fields and top-level
/// JSON keys).
pub fn request(request: &mut Request, found: &mut Found) {
    for Header { name, value } in &mut request.headers {
        if let Some(replaced) = placeholder(name, value, found) {
            *value = replaced;
        }
    }
    if let Some((base, query)) = request.url.split_once('?') {
        request.url = format!("{base}?{}", pairs(query, found));
    }
    let content_type = request
        .headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("content-type"))
        .map(|h| h.value.to_ascii_lowercase())
        .unwrap_or_default();
    if content_type.contains("x-www-form-urlencoded") {
        request.body = pairs(&request.body, found);
    } else if content_type.contains("json") {
        request.body = json_fields(&request.body, found);
    }
}

pub fn headers(headers: &mut BTreeMap<String, String>, found: &mut Found) {
    for (name, value) in headers.iter_mut() {
        if let Some(replaced) = placeholder(name, value, found) {
            *value = replaced;
        }
    }
}

/// `a=1&token=abc` with the credential values replaced.
fn pairs(text: &str, found: &mut Found) -> String {
    text.split('&')
        .map(|pair| match pair.split_once('=') {
            Some((name, value)) => match found.replace(name, value) {
                Some(replaced) => format!("{name}={replaced}"),
                None => pair.to_string(),
            },
            None => pair.to_string(),
        })
        .collect::<Vec<_>>()
        .join("&")
}

/// A JSON object body with the string values of its credential keys replaced. The text is
/// edited in place, so the body keeps its layout.
fn json_fields(body: &str, found: &mut Found) -> String {
    let Ok(serde_json::Value::Object(fields)) = serde_json::from_str(body) else {
        return body.to_string();
    };
    let mut out = body.to_string();
    for (key, value) in fields {
        let Some(value) = value.as_str() else {
            continue;
        };
        let Some(replaced) = found.replace(&key, value) else {
            continue;
        };
        let (Ok(key_text), Ok(value_text)) =
            (serde_json::to_string(&key), serde_json::to_string(value))
        else {
            continue;
        };
        let Some(at) = out.find(&key_text).map(|at| at + key_text.len()) else {
            continue;
        };
        if let Some(offset) = out[at..].find(&value_text) {
            let start = at + offset;
            out.replace_range(start..start + value_text.len(), &format!("\"{replaced}\""));
        }
    }
    out
}

/// A field name that is already a plain identifier is used as is (`client_secret`); others
/// are camel-cased (`X-CSRF-Token` → `xCsrfToken`).
fn variable_name(name: &str) -> String {
    match name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        true => name.to_string(),
        false => camel(name),
    }
}

#[cfg(test)]
#[path = "credentials_tests.rs"]
mod tests;
