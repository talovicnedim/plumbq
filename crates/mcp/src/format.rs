//! Compact text for agents. Tokens are a budget: one line per item, bodies capped.

use plumbq_core::assertions::AssertionResult;
use plumbq_core::compare::{BodyChangeKind, BodyComparison};
use plumbq_core::cookies::Cookie;
use plumbq_core::environments::Environment;
use plumbq_core::examples::ResponseExample;
use plumbq_core::history::{HistoryEntry, HistorySummary};
use plumbq_core::http_file::Header;
use plumbq_core::send::Response;

/// Response bodies longer than this are cut, with a note saying so.
pub const MAX_BODY_CHARS: usize = 8_000;

/// In place of an assertion's message when bodies are hidden: messages quote what was found.
pub const VALUES_HIDDEN: &str = "values hidden";

/// Response headers worth an agent's tokens; the rest (CDN, caching, rate limits) are counted.
const USEFUL_HEADERS: [&str; 9] = [
    "content-type",
    "content-length",
    "location",
    "www-authenticate",
    "retry-after",
    "set-cookie",
    "x-request-id",
    "allow",
    "content-disposition",
];

pub fn environments(environments: &[Environment]) -> String {
    if environments.is_empty() {
        return "No environments. Create one with environments_create.\n".into();
    }
    let mut out = String::new();
    for env in environments {
        out.push_str(&format!("{} ({:?})\n", env.name, env.tag).to_lowercase());
        for (name, value) in &env.variables {
            out.push_str(&format!("  {name} = {value}\n"));
        }
        for secret in &env.secrets {
            let state = if secret.is_set { "set" } else { "not set" };
            out.push_str(&format!("  {} = <secret, {state}>\n", secret.name));
        }
    }
    out
}

/// Status line, headers and a capped, pretty-printed body.
pub fn response(response: &Response) -> String {
    let mut out = format!(
        "{} {}  ({} ms, {} bytes)\nSent: {} {}\n\nResponse headers:\n",
        response.status,
        response.status_text,
        response.duration_ms,
        response.size,
        response.sent.method,
        response.sent.url
    );
    out.push_str(&headers(&response.headers));
    out.push_str(&assertions(&response.assertions));
    out.push_str("\nBody:\n");
    if response.binary {
        out.push_str("(binary, not shown)\n");
        return out;
    }
    out.push_str(&body(&response.body));
    out
}

/// Useful headers in full, the rest only counted.
fn headers(headers: &[Header]) -> String {
    let (useful, other): (Vec<_>, Vec<_>) = headers
        .iter()
        .partition(|h| USEFUL_HEADERS.contains(&h.name.to_lowercase().as_str()));
    let mut out = String::new();
    for header in useful {
        // Cookie values are session credentials; the agent only needs to know one was set.
        let value = if header.name.eq_ignore_ascii_case("set-cookie") {
            let name = header.value.split('=').next().unwrap_or_default();
            format!("{name}=… (stored in the cookie jar)")
        } else {
            header.value.clone()
        };
        out.push_str(&format!("  {}: {value}\n", header.name));
    }
    if !other.is_empty() {
        out.push_str(&format!(
            "  (+{} more, e.g. caching and CDN headers)\n",
            other.len()
        ));
    }
    out
}

fn assertions(results: &[AssertionResult]) -> String {
    if results.is_empty() {
        return String::new();
    }
    let passed = results.iter().filter(|a| a.passed).count();
    let hidden = crate::server::bodies_hidden();
    let mut out = format!("\nAssertions: {passed}/{} passed\n", results.len());
    for result in results {
        let mark = if result.passed { "✓" } else { "✗" };
        let message = if hidden {
            VALUES_HIDDEN
        } else {
            &result.message
        };
        out.push_str(&format!("  {mark} {}  ({message})\n", result.expression));
    }
    out
}

/// A response body for the agent: capped and pretty-printed, or a note when the user doesn't
/// share bodies with this agent.
pub fn body(text: &str) -> String {
    if crate::server::bodies_hidden() {
        return format!(
            "(hidden: {} characters; the user doesn't share response bodies with the assistant)\n",
            text.chars().count()
        );
    }
    cap(&pretty(text))
}

/// Saved examples shown in full; the rest are only counted.
const MAX_EXAMPLES: usize = 5;

pub fn examples(examples: &[ResponseExample]) -> String {
    if examples.is_empty() {
        return "No saved examples for this request.\n".into();
    }
    let mut out: String = examples
        .iter()
        .take(MAX_EXAMPLES)
        .map(|example| {
            let content_type = example
                .headers
                .iter()
                .find(|h| h.name.eq_ignore_ascii_case("content-type"))
                .map(|h| format!(" ({})", h.value))
                .unwrap_or_default();
            format!(
                "Example \"{}\": {}{content_type}\n{}\n",
                example.name,
                example.status,
                body(&example.body)
            )
        })
        .collect();
    if examples.len() > MAX_EXAMPLES {
        out.push_str(&format!(
            "… and {} more examples: {}\n",
            examples.len() - MAX_EXAMPLES,
            examples[MAX_EXAMPLES..]
                .iter()
                .map(|e| e.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    out
}

/// A body comparison as a list of changed fields or lines.
pub fn comparison(comparison: &BodyComparison) -> String {
    if comparison.changes.is_empty() {
        return "Bodies are identical.\n".into();
    }
    // Values are body content: with bodies hidden, only the changed paths are listed.
    let hidden = crate::server::bodies_hidden();
    let value = |v: &Option<String>| {
        if hidden {
            "…".to_string()
        } else {
            v.clone().unwrap_or_default()
        }
    };
    let mut out: String = comparison
        .changes
        .iter()
        .map(|change| match change.kind {
            BodyChangeKind::Added => format!("+ {} = {}\n", change.path, value(&change.after)),
            BodyChangeKind::Removed => {
                format!("- {} (was {})\n", change.path, value(&change.before))
            }
            BodyChangeKind::Changed => format!(
                "~ {}: {} -> {}\n",
                change.path,
                value(&change.before),
                value(&change.after)
            ),
        })
        .collect();
    if comparison.omitted > 0 {
        out.push_str(&format!("… and {} more changes\n", comparison.omitted));
    }
    out
}

/// Cookie names, domains, paths and expiry; never values.
pub fn cookies(jar: &[Cookie]) -> String {
    if jar.is_empty() {
        return "No cookies stored.\n".into();
    }
    jar.iter()
        .map(|c| {
            let expires = c
                .expires
                .as_deref()
                .map_or("session".to_string(), |at| format!("expires {at}"));
            format!(
                "{}  domain {}  path {}  {expires}\n",
                c.name, c.domain, c.path
            )
        })
        .collect()
}

pub fn history(rows: &[HistorySummary]) -> String {
    if rows.is_empty() {
        return "Never sent.\n".into();
    }
    rows.iter()
        .map(|row| {
            format!(
                "{}  {}  {} {}  -> {} ({} ms){}{}\n",
                row.id,
                row.sent_at,
                row.method,
                row.url,
                row.status,
                row.duration_ms,
                if row.passed + row.failed > 0 {
                    format!("  assertions {}/{}", row.passed, row.passed + row.failed)
                } else {
                    String::new()
                },
                row.environment
                    .as_ref()
                    .map(|e| format!("  env: {e}"))
                    .unwrap_or_default()
            )
        })
        .collect()
}

pub fn history_entry(entry: &HistoryEntry) -> String {
    format!(
        "Sent {}{}\n{}",
        entry.sent_at,
        entry
            .environment
            .as_ref()
            .map(|e| format!(" with environment {e}"))
            .unwrap_or_default(),
        response(&entry.response)
    )
}

fn pretty(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .and_then(|value| serde_json::to_string_pretty(&value))
        .unwrap_or_else(|_| body.to_string())
}

/// Text cut at [`MAX_BODY_CHARS`], with a note saying so.
pub fn cap(text: &str) -> String {
    cap_with(text, "full body in the app's history")
}

/// Cuts `text` at [`MAX_BODY_CHARS`] (on a character boundary), saying where the rest is.
pub fn cap_with(text: &str, rest: &str) -> String {
    match text.char_indices().nth(MAX_BODY_CHARS) {
        Some((cut, _)) => format!(
            "{}\n… (cut at {MAX_BODY_CHARS} characters; {rest})\n",
            &text[..cut]
        ),
        None => format!("{text}\n"),
    }
}

#[cfg(test)]
#[path = "format_tests.rs"]
mod tests;
