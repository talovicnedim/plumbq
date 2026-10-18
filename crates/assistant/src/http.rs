//! The HTTP side shared by the adapters: one client, error mapping that never echoes keys, and
//! a server-sent events reader.

use std::sync::OnceLock;
use std::time::Duration;

use futures_util::StreamExt;

use crate::{Error, Result};

/// Longest provider error text shown; some servers answer with a whole HTML page.
const MAX_DETAIL_CHARS: usize = 300;

/// Shared so every turn reuses the connection instead of a new TLS handshake.
pub(crate) fn client() -> Result<reqwest::Client> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client.clone());
    }
    plumbq_core::http_client::install_crypto();
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        // Long answers stream for a while; this only stops a hung server.
        .read_timeout(Duration::from_secs(120))
        .build()
        .map_err(|err| Error::Provider(format!("failed to set up HTTP: {err}")))?;
    Ok(CLIENT.get_or_init(|| client).clone())
}

/// Turns a failed response into an error a user can act on. `key` is removed from the text,
/// because some servers echo it back.
pub(crate) async fn check(response: reqwest::Response, key: &str) -> Result<reqwest::Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = response.text().await.unwrap_or_default();
    let detail = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| {
            v.pointer("/error/message")
                .or_else(|| v.pointer("/error"))
                .or_else(|| v.pointer("/message"))
                .and_then(|m| m.as_str().map(str::to_string))
        })
        .unwrap_or(body);
    let detail = cut(&hide(&detail, key));
    Err(match status.as_u16() {
        401 | 403 => Error::Provider(format!(
            "the provider rejected the API key ({status}): {detail}"
        )),
        404 => Error::Invalid(format!(
            "not found ({status}): {detail}; check the base URL and model name"
        )),
        429 => Error::Provider(format!(
            "rate limited or out of credits ({status}): {detail}"
        )),
        _ => Error::Provider(format!("the provider returned {status}: {detail}")),
    })
}

/// reqwest errors can include the URL; keys go in headers, never in URLs, but drop it anyway.
pub(crate) fn send_error(err: reqwest::Error) -> Error {
    Error::Provider(format!(
        "couldn't reach the provider: {}",
        err.without_url()
    ))
}

fn cut(text: &str) -> String {
    match text.char_indices().nth(MAX_DETAIL_CHARS) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_string(),
    }
}

fn hide(text: &str, key: &str) -> String {
    if key.len() < 8 {
        return text.to_string();
    }
    text.replace(key, "<api key>")
}

/// Calls `on_event` with the `data:` payload of each server-sent event.
pub(crate) async fn sse(
    response: reqwest::Response,
    mut on_event: impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();
    while let Some(chunk) = stream.next().await {
        buffer.extend_from_slice(&chunk.map_err(send_error)?);
        for line in complete_lines(&mut buffer) {
            if let Some(data) = line.strip_prefix("data:") {
                let data = data.trim();
                if data == "[DONE]" {
                    return Ok(());
                }
                if !data.is_empty() {
                    on_event(data)?;
                }
            }
        }
    }
    Ok(())
}

/// Takes the complete lines out of `buffer` and decodes each one. Bytes after the last newline
/// stay, because a network chunk can end inside a multi-byte character.
fn complete_lines(buffer: &mut Vec<u8>) -> Vec<String> {
    let Some(last) = buffer.iter().rposition(|&b| b == b'\n') else {
        return Vec::new();
    };
    let rest = buffer.split_off(last + 1);
    let lines = buffer
        .split(|&b| b == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| {
            String::from_utf8_lossy(line)
                .trim_end_matches('\r')
                .to_string()
        })
        .collect();
    *buffer = rest;
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_echoed_keys() {
        assert_eq!(
            hide("bad key sk-ant-12345678", "sk-ant-12345678"),
            "bad key <api key>"
        );
        assert_eq!(hide("short", "abc"), "short");
    }

    #[test]
    fn a_character_split_across_chunks_stays_whole() {
        let text = "data: grüße\n".as_bytes();
        let split = text.iter().position(|&b| b == 0xc3).unwrap() + 1;
        let mut buffer = text[..split].to_vec();
        assert!(complete_lines(&mut buffer).is_empty());
        buffer.extend_from_slice(&text[split..]);
        assert_eq!(complete_lines(&mut buffer), ["data: grüße"]);
        assert!(buffer.is_empty());
    }

    #[test]
    fn long_error_text_is_cut_with_a_mark() {
        let long = "é".repeat(MAX_DETAIL_CHARS + 10);
        let cut = cut(&long);
        assert_eq!(cut.chars().count(), MAX_DETAIL_CHARS + 1);
        assert!(cut.ends_with('…'));
        assert_eq!(super::cut("short"), "short");
    }
}
