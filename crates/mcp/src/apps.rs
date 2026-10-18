//! One-click setup in AI apps: adds or removes an `plumbq` entry in each app's MCP config,
//! keeping the rest. HTTP apps get the address; Claude Desktop and Codex start the bundled bridge.

use std::path::{Path, PathBuf};

use plumbq_core::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::config_edit::{self, Entry, edit};

/// The server's name in every config (also the setup snippets' name).
pub const NAME: &str = "plumbq";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum McpApp {
    ClaudeCode,
    ClaudeDesktop,
    Cursor,
    VsCode,
    Codex,
}

pub const APPS: [McpApp; 5] = [
    McpApp::ClaudeCode,
    McpApp::ClaudeDesktop,
    McpApp::Cursor,
    McpApp::VsCode,
    McpApp::Codex,
];

/// Where AI apps reach the app's MCP server.
pub struct McpTarget<'a> {
    /// The in-app HTTP server, `http://127.0.0.1:7459/mcp/<token>`.
    pub url: &'a str,
    /// The bundled `plumbq-mcp` binary, for the apps that start a command.
    pub binary: &'a Path,
}

impl McpTarget<'_> {
    /// The bridge's arguments: the binary forwards stdio to `url`.
    pub(crate) fn bridge_args(&self) -> [&str; 2] {
        ["--url", self.url]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct McpAppStatus {
    pub app: McpApp,
    pub label: String,
    /// The app seems to be on this machine (its config folder exists).
    pub installed: bool,
    pub connected: bool,
    /// Connected with an older server URL (the port or token changed since) or a bridge that
    /// isn't where the app is now.
    pub outdated: bool,
    pub config_path: String,
    /// Why the app's config couldn't be read (not a missing file, which means not connected).
    #[serde(default)]
    pub error: Option<String>,
}

impl McpApp {
    pub fn label(self) -> &'static str {
        match self {
            McpApp::ClaudeCode => "Claude Code",
            McpApp::ClaudeDesktop => "Claude Desktop",
            McpApp::Cursor => "Cursor",
            McpApp::VsCode => "VS Code",
            McpApp::Codex => "Codex",
        }
    }

    /// Where the app keeps its MCP servers, under the user's home folder.
    fn config_path(self, home: &Path) -> PathBuf {
        let app_support = if cfg!(target_os = "macos") {
            home.join("Library/Application Support")
        } else if cfg!(windows) {
            home.join("AppData/Roaming")
        } else {
            home.join(".config")
        };
        match self {
            McpApp::ClaudeCode => home.join(".claude.json"),
            McpApp::ClaudeDesktop => app_support.join("Claude/claude_desktop_config.json"),
            McpApp::Cursor => home.join(".cursor/mcp.json"),
            McpApp::VsCode => app_support.join("Code/User/mcp.json"),
            McpApp::Codex => home.join(".codex/config.toml"),
        }
    }

    /// The folder whose existence means the app is installed.
    fn marker(self, home: &Path) -> PathBuf {
        match self {
            McpApp::ClaudeCode => home.join(".claude"),
            _ => self
                .config_path(home)
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
        }
    }

    /// Connects to the in-app HTTP server directly (the others start the bridge).
    pub fn uses_url(self) -> bool {
        matches!(self, McpApp::ClaudeCode | McpApp::Cursor | McpApp::VsCode)
    }

    /// VS Code calls the section `servers`; the others `mcpServers`.
    pub(crate) fn json_section(self) -> &'static str {
        if self == McpApp::VsCode {
            "servers"
        } else {
            "mcpServers"
        }
    }

    /// The entry for a JSON config.
    pub(crate) fn json_entry(self, target: &McpTarget<'_>) -> Value {
        match self {
            McpApp::ClaudeDesktop | McpApp::Codex => json!({
                "command": target.binary.to_string_lossy(),
                "args": target.bridge_args(),
            }),
            McpApp::Cursor => json!({ "url": target.url }),
            McpApp::ClaudeCode | McpApp::VsCode => json!({"type": "http", "url": target.url}),
        }
    }
}

/// The AI client for people: `claude-code` → `Claude Code`. Unknown names stay as they are.
pub fn client_label(client: &str) -> String {
    let app = match client.to_ascii_lowercase().as_str() {
        "claude-code" => McpApp::ClaudeCode,
        "claude-ai" | "claude desktop" => McpApp::ClaudeDesktop,
        "cursor" | "cursor-vscode" => McpApp::Cursor,
        "vscode" | "visual studio code" => McpApp::VsCode,
        "codex" | "codex-mcp-client" => McpApp::Codex,
        "chatgpt" | "openai-mcp" => return "ChatGPT".to_string(),
        _ => return client.to_string(),
    };
    app.label().to_string()
}

/// Every supported app and whether the server at `target` is set up in it.
pub fn status(home: &Path, target: &McpTarget<'_>) -> Vec<McpAppStatus> {
    APPS.iter()
        .map(|&app| {
            let path = app.config_path(home);
            let (entries, error) = match config_edit::entries(app, &path) {
                Ok(entries) => (entries, None),
                Err(err) => (Vec::new(), Some(err)),
            };
            McpAppStatus {
                app,
                label: app.label().to_string(),
                installed: app.marker(home).exists() || path.exists(),
                connected: !entries.is_empty(),
                outdated: entries.iter().any(|entry| is_outdated(entry, target)),
                config_path: path.to_string_lossy().into_owned(),
                error,
            }
        })
        .collect()
}

/// Set up with an older address, or starting a bridge that isn't where the app is now: the app
/// was moved, or it is an AppImage, which runs from a new folder on every launch.
fn is_outdated(entry: &Entry, target: &McpTarget<'_>) -> bool {
    let moved = entry
        .command
        .as_deref()
        .is_some_and(|command| Path::new(command) != target.binary);
    moved || !entry.text.contains(target.url)
}

/// Adds (or updates) the entry for the server at `target` in the app's config.
pub fn connect(app: McpApp, home: &Path, target: &McpTarget<'_>) -> Result<()> {
    let path = app.config_path(home);
    edit(app, &path, |config| {
        config_edit::insert(app, &path, config, target)
    })
}

/// Removes our entry; everything else in the file stays.
pub fn disconnect(app: McpApp, home: &Path) -> Result<()> {
    let path = app.config_path(home);
    if !path.exists() {
        return Ok(());
    }
    edit(app, &path, |config| {
        config_edit::remove(app, config);
        Ok(())
    })
}

#[cfg(test)]
#[path = "apps_tests.rs"]
mod tests;
