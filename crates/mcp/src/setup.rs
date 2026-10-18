//! Copyable setup for every AI client in "AI apps": the fallback next to one-click setup,
//! plus ChatGPT (through a tunnel) and the headless stdio server.

use serde::Serialize;
use serde_json::json;

use crate::apps::{McpApp, McpTarget, NAME};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum McpClient {
    ClaudeCode,
    ClaudeDesktop,
    Cursor,
    VsCode,
    Codex,
    Chatgpt,
    /// The bundled binary on one workspace, without the app (shown under Server settings).
    Stdio,
}

const CLIENTS: [McpClient; 7] = [
    McpClient::ClaudeCode,
    McpClient::ClaudeDesktop,
    McpClient::Cursor,
    McpClient::VsCode,
    McpClient::Codex,
    McpClient::Chatgpt,
    McpClient::Stdio,
];

const BRIDGE_NOTE: &str = "Runs the MCP bridge bundled with Plumbq (no Node.js needed); \
changes still pop up here for approval.";

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct McpClientSetup {
    pub client: McpClient,
    pub label: String,
    /// The app Plumbq can add itself to with one click, if any.
    pub one_click: Option<McpApp>,
    /// Starts the bundled binary, which has to be built first in development.
    pub uses_binary: bool,
    /// What to do with the snippet.
    pub hint: String,
    pub snippet: String,
    /// Extra steps after the snippet, if any.
    pub note: Option<String>,
}

/// User scope, so the server works in every folder; without it Claude Code keeps it for the
/// folder the command ran in.
const CLAUDE_CODE_ADD: [&str; 8] = [
    "claude",
    "mcp",
    "add",
    "--scope",
    "user",
    "--transport",
    "http",
    NAME,
];

/// Setup for every client, in tab order, `stdio` last. `root` is the open workspace.
pub fn clients(target: &McpTarget<'_>, root: &str) -> Vec<McpClientSetup> {
    CLIENTS
        .iter()
        .map(|&client| setup(client, target, root))
        .collect()
}

fn setup(client: McpClient, target: &McpTarget<'_>, root: &str) -> McpClientSetup {
    let (hint, snippet, note) = match client {
        McpClient::ClaudeCode => (
            "Run in a terminal:".to_string(),
            command(&[CLAUDE_CODE_ADD.as_slice(), &[target.url]].concat()),
            None,
        ),
        McpClient::ClaudeDesktop => (
            "Add to claude_desktop_config.json (Settings → Developer → Edit Config), then restart Claude:"
                .to_string(),
            json_config(McpApp::ClaudeDesktop, target),
            Some(BRIDGE_NOTE.to_string()),
        ),
        McpClient::Cursor => (
            "Add to ~/.cursor/mcp.json:".to_string(),
            json_config(McpApp::Cursor, target),
            None,
        ),
        McpClient::VsCode => (
            "Add to .vscode/mcp.json in your project (or run \"MCP: Add Server…\"):".to_string(),
            json_config(McpApp::VsCode, target),
            None,
        ),
        McpClient::Codex => (
            "Add to ~/.codex/config.toml:".to_string(),
            codex_config(target),
            Some(BRIDGE_NOTE.to_string()),
        ),
        McpClient::Chatgpt => chatgpt(target.url),
        McpClient::Stdio => stdio(target, root),
    };
    McpClientSetup {
        client,
        label: label(client).to_string(),
        one_click: one_click(client),
        uses_binary: matches!(
            client,
            McpClient::ClaudeDesktop | McpClient::Codex | McpClient::Stdio
        ),
        hint,
        snippet,
        note,
    }
}

fn label(client: McpClient) -> &'static str {
    match one_click(client) {
        Some(app) => app.label(),
        None if client == McpClient::Chatgpt => "ChatGPT",
        None => "Without the app",
    }
}

fn one_click(client: McpClient) -> Option<McpApp> {
    match client {
        McpClient::ClaudeCode => Some(McpApp::ClaudeCode),
        McpClient::ClaudeDesktop => Some(McpApp::ClaudeDesktop),
        McpClient::Cursor => Some(McpApp::Cursor),
        McpClient::VsCode => Some(McpApp::VsCode),
        McpClient::Codex => Some(McpApp::Codex),
        McpClient::Chatgpt | McpClient::Stdio => None,
    }
}

fn json_config(app: McpApp, target: &McpTarget<'_>) -> String {
    let config = json!({ app.json_section(): { NAME: app.json_entry(target) } });
    serde_json::to_string_pretty(&config).unwrap_or_default()
}

fn codex_config(target: &McpTarget<'_>) -> String {
    let binary = target.binary.to_string_lossy();
    format!(
        "[mcp_servers.{NAME}]\ncommand = {}\nargs = [\"--url\", {}]",
        toml_string(&binary),
        toml_string(target.url)
    )
}

/// ChatGPT needs a public HTTPS address: a tunnel to the server's origin, then the token path.
fn chatgpt(url: &str) -> (String, String, Option<String>) {
    let (origin, path) = split_origin(url);
    (
        "ChatGPT needs a public HTTPS address. Start a tunnel to the app, e.g.:".to_string(),
        format!("cloudflared tunnel --url {origin}"),
        Some(format!(
            "Then in ChatGPT (Settings → Connectors, developer mode) add the tunnel's https address \
followed by {path}. The secret token in that path is the only key: share it with no one, and \
regenerate it when you stop using the tunnel."
        )),
    )
}

fn stdio(target: &McpTarget<'_>, root: &str) -> (String, String, Option<String>) {
    let binary = target.binary.to_string_lossy();
    let name = server_name(root);
    (
        "Runs the server for this workspace without the app, for headless use (e.g. a CI box). \
For Claude Code:"
            .to_string(),
        command(&[
            "claude",
            "mcp",
            "add",
            &name,
            "--",
            &binary,
            "--workspace",
            root,
        ]),
        Some(format!(
            "Other clients: command {binary} with args [\"--workspace\", {}]. Changes are approved \
in the app when it's open; otherwise they're denied.",
            toml_string(root)
        )),
    )
}

/// A server name per workspace for the stdio binary, so several can be connected at once.
fn server_name(root: &str) -> String {
    let base = root
        .split(['/', '\\'])
        .rfind(|part| !part.is_empty())
        .unwrap_or("workspace");
    let slug: Vec<String> = base
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect();
    match slug.is_empty() {
        true => "api-workspace".to_string(),
        false => format!("api-{}", slug.join("-")),
    }
}

/// `http://127.0.0.1:7459/mcp/abc` → (`http://127.0.0.1:7459`, `/mcp/abc`).
fn split_origin(url: &str) -> (&str, &str) {
    let after_scheme = url.find("://").map_or(0, |i| i + 3);
    match url[after_scheme..].find('/') {
        Some(i) => url.split_at(after_scheme + i),
        None => (url, "/"),
    }
}

fn command(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|part| shell_quote(part))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(text: &str) -> String {
    let plain = !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c));
    match plain {
        true => text.to_string(),
        false => format!("'{}'", text.replace('\'', "'\\''")),
    }
}

/// A TOML basic string (also valid JSON for paths and URLs).
fn toml_string(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_default()
}

#[cfg(test)]
#[path = "setup_tests.rs"]
mod tests;
