//! `application/x-www-form-urlencoded` without a dependency: encoding token requests and
//! decoding the OAuth redirect's query string.

use std::collections::BTreeMap;

/// `application/x-www-form-urlencoded` without a dependency: unreserved characters kept,
/// everything else percent-encoded.
pub(crate) fn form_encode(fields: &[(&str, &str)]) -> String {
    let encode = |text: &str| -> String {
        text.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    (b as char).to_string()
                }
                _ => format!("%{b:02X}"),
            })
            .collect()
    };
    fields
        .iter()
        .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// Query-string pairs, percent-decoded.
pub(crate) fn parse_query(query: &str) -> BTreeMap<String, String> {
    query
        .split('&')
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            Some((percent_decode(k)?, percent_decode(v)?))
        })
        .collect()
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = text.get(i + 1..i + 3)?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_and_decodes_forms() {
        assert_eq!(
            form_encode(&[("scope", "read write"), ("x", "a&b=c")]),
            "scope=read%20write&x=a%26b%3Dc"
        );
        let query = parse_query("code=abc%2F1&state=x+y");
        assert_eq!(query["code"], "abc/1");
        assert_eq!(query["state"], "x y");
    }
}
