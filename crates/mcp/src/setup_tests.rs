use std::path::Path;

use super::*;

const URL: &str = "http://127.0.0.1:7459/mcp/abc123";

fn snippet(client: McpClient, binary: &str) -> McpClientSetup {
    let target = McpTarget {
        url: URL,
        binary: Path::new(binary),
    };
    setup(client, &target, "/w")
}

#[test]
fn names_stdio_servers_after_the_workspace() {
    assert_eq!(server_name("/Users/ada/My API/"), "api-my-api");
    assert_eq!(server_name("/"), "api-workspace");
}

#[test]
fn adds_the_http_server_to_claude_code() {
    assert_eq!(
        snippet(McpClient::ClaudeCode, "/bin/mcp").snippet,
        format!("claude mcp add --scope user --transport http plumbq {URL}")
    );
}

#[test]
fn produces_valid_json_configs_for_http_clients() {
    let cursor: serde_json::Value =
        serde_json::from_str(&snippet(McpClient::Cursor, "/bin/mcp").snippet).unwrap();
    assert_eq!(
        cursor,
        json!({ "mcpServers": { "plumbq": { "url": URL } } })
    );
    let vscode: serde_json::Value =
        serde_json::from_str(&snippet(McpClient::VsCode, "/bin/mcp").snippet).unwrap();
    assert_eq!(
        vscode["servers"]["plumbq"],
        json!({ "type": "http", "url": URL })
    );
}

#[test]
fn starts_the_bundled_bridge_for_claude_desktop_and_codex() {
    let desktop: serde_json::Value =
        serde_json::from_str(&snippet(McpClient::ClaudeDesktop, "/bin/mcp").snippet).unwrap();
    assert_eq!(
        desktop["mcpServers"]["plumbq"],
        json!({ "command": "/bin/mcp", "args": ["--url", URL] })
    );
    assert_eq!(
        snippet(McpClient::Codex, "/bin/mcp").snippet,
        format!("[mcp_servers.plumbq]\ncommand = \"/bin/mcp\"\nargs = [\"--url\", \"{URL}\"]")
    );
}

#[test]
fn tunnels_the_origin_for_chatgpt_and_keeps_the_token_path() {
    let setup = snippet(McpClient::Chatgpt, "/bin/mcp");
    assert_eq!(
        setup.snippet,
        "cloudflared tunnel --url http://127.0.0.1:7459"
    );
    assert!(setup.note.unwrap_or_default().contains("/mcp/abc123"));
}

#[test]
fn quotes_paths_with_spaces_for_the_stdio_binary() {
    assert_eq!(
        snippet(McpClient::Stdio, "/My Apps/Plumbq.app/mcp").snippet,
        "claude mcp add api-w -- '/My Apps/Plumbq.app/mcp' --workspace /w"
    );
}

#[test]
fn every_client_has_a_label_and_stdio_comes_last() {
    let target = McpTarget {
        url: URL,
        binary: Path::new("/bin/mcp"),
    };
    let all = clients(&target, "/w");
    assert_eq!(all.len(), 7);
    assert_eq!(all[5].label, "ChatGPT");
    assert_eq!(all[6].client, McpClient::Stdio);
}
