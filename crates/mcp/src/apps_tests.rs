use std::fs;

use super::*;

const URL: &str = "http://127.0.0.1:7459/mcp/abc";
const BINARY: &str = "/Applications/Plumbq.app/Contents/MacOS/plumbq-mcp";

fn target(url: &str) -> McpTarget<'_> {
    McpTarget {
        url,
        binary: Path::new(BINARY),
    }
}

fn temp_home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ac-test-apps-{name}-{}",
        &crate::settings::new_token()[..12]
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn connects_and_disconnects_keeping_other_settings() {
    let home = temp_home("json");
    let config = McpApp::ClaudeDesktop.config_path(&home);
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    fs::write(
        &config,
        r#"{"theme": "dark", "mcpServers": {"other": {"command": "x"}}}"#,
    )
    .unwrap();

    connect(McpApp::ClaudeDesktop, &home, &target(URL)).unwrap();
    let text = fs::read_to_string(&config).unwrap();
    let value: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["theme"], "dark");
    assert_eq!(value["mcpServers"]["other"]["command"], "x");
    assert_eq!(value["mcpServers"][NAME]["command"], BINARY);
    assert_eq!(value["mcpServers"][NAME]["args"], json!(["--url", URL]));
    assert!(
        text.find("theme") < text.find("mcpServers"),
        "key order is kept"
    );
    let claude = |url: &str| {
        status(&home, &target(url))
            .into_iter()
            .find(|s| s.app == McpApp::ClaudeDesktop)
            .unwrap()
    };
    assert!(claude(URL).connected && claude(URL).installed && !claude(URL).outdated);
    assert!(claude("http://127.0.0.1:7459/mcp/new").outdated);

    disconnect(McpApp::ClaudeDesktop, &home).unwrap();
    let value: Value = serde_json::from_str(&fs::read_to_string(&config).unwrap()).unwrap();
    assert!(value["mcpServers"].get(NAME).is_none());
    assert_eq!(value["mcpServers"]["other"]["command"], "x");
    let backup = config.with_extension("json.plumbq-backup");
    assert!(fs::read_to_string(backup).unwrap().contains("\"other\""));
}

#[test]
fn apps_that_start_the_bridge_need_an_update_once_the_app_moves() {
    let home = temp_home("moved");
    for app in [McpApp::ClaudeDesktop, McpApp::Codex, McpApp::Cursor] {
        connect(app, &home, &target(URL)).unwrap();
    }
    let moved = McpTarget {
        url: URL,
        binary: Path::new("/tmp/.mount_PlumbqX1y2Z3/usr/bin/plumbq-mcp"),
    };
    let outdated = |target: &McpTarget<'_>, app: McpApp| {
        status(&home, target)
            .into_iter()
            .find(|s| s.app == app)
            .unwrap()
            .outdated
    };

    for app in [McpApp::ClaudeDesktop, McpApp::Codex] {
        assert!(!outdated(&target(URL), app), "{app:?} where it was set up");
        assert!(outdated(&moved, app), "{app:?} after the app moved");
    }
    assert!(!outdated(&moved, McpApp::Cursor), "URL apps don't start it");
}

#[test]
fn http_clients_get_the_url_and_missing_files_are_created() {
    let home = temp_home("http");
    connect(McpApp::VsCode, &home, &target(URL)).unwrap();
    connect(McpApp::Cursor, &home, &target(URL)).unwrap();
    let read = |app: McpApp| -> Value {
        serde_json::from_str(&fs::read_to_string(app.config_path(&home)).unwrap()).unwrap()
    };
    assert_eq!(read(McpApp::VsCode)["servers"][NAME]["type"], "http");
    assert_eq!(read(McpApp::VsCode)["servers"][NAME]["url"], URL);
    assert_eq!(read(McpApp::Cursor)["mcpServers"][NAME]["url"], URL);
}

#[test]
fn codex_toml_keeps_comments_and_other_servers() {
    let home = temp_home("codex");
    let path = McpApp::Codex.config_path(&home);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        "# my settings\nmodel = \"x\"\n\n[mcp_servers.other]\ncommand = \"o\"\n",
    )
    .unwrap();

    connect(McpApp::Codex, &home, &target(URL)).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.starts_with("# my settings\nmodel = \"x\""), "{text}");
    assert!(
        text.contains("[mcp_servers.other]") && text.contains(URL),
        "{text}"
    );
    let doc: toml_edit::DocumentMut = text.parse().unwrap();
    let entry = &doc["mcp_servers"][NAME];
    assert_eq!(entry["command"].as_str(), Some(BINARY));
    assert_eq!(
        entry["args"].to_string().trim(),
        format!(r#"["--url", "{URL}"]"#)
    );

    disconnect(McpApp::Codex, &home).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(
        !text.contains(NAME) && text.contains("[mcp_servers.other]"),
        "{text}"
    );
}

#[cfg(unix)]
#[test]
fn configs_and_backups_are_private_and_existing_modes_are_kept() {
    use std::os::unix::fs::PermissionsExt;
    let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;

    let home = temp_home("modes");
    connect(McpApp::Cursor, &home, &target(URL)).unwrap();
    let cursor = McpApp::Cursor.config_path(&home);
    assert_eq!(mode(&cursor), 0o600);
    assert_eq!(mode(cursor.parent().unwrap()), 0o700);

    let desktop = McpApp::ClaudeDesktop.config_path(&home);
    fs::create_dir_all(desktop.parent().unwrap()).unwrap();
    fs::write(&desktop, "{}").unwrap();
    fs::set_permissions(&desktop, fs::Permissions::from_mode(0o640)).unwrap();
    connect(McpApp::ClaudeDesktop, &home, &target(URL)).unwrap();
    assert_eq!(mode(&desktop), 0o640, "an existing file keeps its mode");
    assert_eq!(mode(&desktop.with_extension("json.plumbq-backup")), 0o600);
    let leftovers = fs::read_dir(desktop.parent().unwrap())
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        })
        .count();
    assert_eq!(leftovers, 0);
}

#[test]
fn refuses_to_overwrite_a_broken_config() {
    let home = temp_home("broken");
    let path = McpApp::Cursor.config_path(&home);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "{ not json").unwrap();
    assert!(connect(McpApp::Cursor, &home, &target(URL)).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "{ not json");

    let cursor = status(&home, &target(URL))
        .into_iter()
        .find(|s| s.app == McpApp::Cursor)
        .unwrap();
    assert!(!cursor.connected);
    assert!(cursor.error.is_some_and(|e| e.contains("failed to read")));
    let missing = status(&home, &target(URL))
        .into_iter()
        .find(|s| s.app == McpApp::ClaudeDesktop)
        .unwrap();
    assert_eq!(
        missing.error, None,
        "a missing config only means not connected"
    );
}

#[cfg(unix)]
#[test]
fn a_symlinked_config_stays_a_symlink_and_backups_follow_each_edit() {
    let home = temp_home("symlink");
    let dotfiles = home.join("dotfiles/cursor.json");
    fs::create_dir_all(dotfiles.parent().unwrap()).unwrap();
    fs::write(&dotfiles, r#"{"mcpServers": {"other": {"url": "x"}}}"#).unwrap();
    let config = McpApp::Cursor.config_path(&home);
    fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&dotfiles, &config).unwrap();

    connect(McpApp::Cursor, &home, &target(URL)).unwrap();
    assert!(fs::symlink_metadata(&config).unwrap().is_symlink());
    let saved: Value = serde_json::from_str(&fs::read_to_string(&dotfiles).unwrap()).unwrap();
    assert_eq!(saved["mcpServers"][NAME]["url"], URL);
    assert_eq!(saved["mcpServers"]["other"]["url"], "x");

    let backup = config.with_extension("json.plumbq-backup");
    assert!(!fs::read_to_string(&backup).unwrap().contains(NAME));
    disconnect(McpApp::Cursor, &home).unwrap();
    assert!(
        fs::read_to_string(&backup).unwrap().contains(URL),
        "the backup is the file as it was before the last edit"
    );
}

#[test]
fn known_clients_get_their_app_name() {
    assert_eq!(client_label("claude-code"), "Claude Code");
    assert_eq!(client_label("claude-ai"), "Claude Desktop");
    assert_eq!(client_label("Visual Studio Code"), "VS Code");
    assert_eq!(client_label("codex-mcp-client"), "Codex");
    assert_eq!(client_label("openai-mcp"), "ChatGPT");
    assert_eq!(
        client_label("Chat · Claude Sonnet 5.5"),
        "Chat · Claude Sonnet 5.5"
    );
    assert_eq!(client_label("my-agent"), "my-agent");
}

#[test]
fn claude_code_entries_in_project_scope_count_and_disconnect_removes_them_all() {
    let home = temp_home("claude-scopes");
    let config = McpApp::ClaudeCode.config_path(&home);
    let old = "http://127.0.0.1:7459/mcp/old";
    fs::write(
        &config,
        json!({
            "mcpServers": {"github": {"url": "g"}},
            "projects": {
                "/work/app": {"mcpServers": {NAME: {"type": "http", "url": URL}}},
                "/work/other": {"mcpServers": {"x": {"url": "x"}}, "allowedTools": []}
            }
        })
        .to_string(),
    )
    .unwrap();
    let claude = |url: &str| {
        status(&home, &target(url))
            .into_iter()
            .find(|s| s.app == McpApp::ClaudeCode)
            .unwrap()
    };
    assert!(claude(URL).connected, "local scope from `claude mcp add`");
    assert!(!claude(URL).outdated);
    assert!(claude("http://127.0.0.1:7459/mcp/new").outdated);

    connect(McpApp::ClaudeCode, &home, &target(old)).unwrap();
    let value: Value = serde_json::from_str(&fs::read_to_string(&config).unwrap()).unwrap();
    assert_eq!(
        value["mcpServers"][NAME]["url"], old,
        "connect writes user scope"
    );
    assert_eq!(
        value["projects"]["/work/app"]["mcpServers"][NAME]["url"],
        old
    );
    assert!(!claude(old).outdated);

    disconnect(McpApp::ClaudeCode, &home).unwrap();
    let value: Value = serde_json::from_str(&fs::read_to_string(&config).unwrap()).unwrap();
    assert!(value["mcpServers"].get(NAME).is_none());
    assert!(
        value["projects"]["/work/app"]["mcpServers"]
            .get(NAME)
            .is_none()
    );
    assert_eq!(value["mcpServers"]["github"]["url"], "g");
    assert_eq!(
        value["projects"]["/work/other"]["mcpServers"]["x"]["url"],
        "x"
    );
    assert!(!claude(URL).connected);
}

#[test]
fn hand_made_project_entries_are_left_alone() {
    let home = temp_home("claude-hand-made");
    let config = McpApp::ClaudeCode.config_path(&home);
    let workspace = json!({
        "command": BINARY,
        "args": ["--workspace", "/work/api"]
    });
    let bridge = json!({"command": BINARY, "args": ["--url", URL]});
    fs::write(
        &config,
        json!({
            "projects": {
                "/work/api": {"mcpServers": {NAME: workspace}},
                "/work/app": {"mcpServers": {NAME: bridge}},
                "/work/web": {"mcpServers": {NAME: {"type": "http", "url": URL}}},
                "/work/foreign": {"mcpServers": {NAME: {"type": "http", "url": "https://example.com/mcp/x"}}}
            }
        })
        .to_string(),
    )
    .unwrap();
    let read = || -> Value { serde_json::from_str(&fs::read_to_string(&config).unwrap()).unwrap() };
    let connected = || {
        status(&home, &target(URL))
            .into_iter()
            .find(|s| s.app == McpApp::ClaudeCode)
            .unwrap()
            .connected
    };
    assert!(connected(), "our entries count");

    let new = "http://127.0.0.1:7459/mcp/new";
    connect(McpApp::ClaudeCode, &home, &target(new)).unwrap();
    let value = read();
    assert_eq!(
        value["projects"]["/work/api"]["mcpServers"][NAME]["args"][0],
        "--workspace"
    );
    assert_eq!(
        value["projects"]["/work/web"]["mcpServers"][NAME]["url"],
        new
    );
    assert_eq!(
        value["projects"]["/work/app"]["mcpServers"][NAME]["url"], new,
        "a bridge entry becomes the http entry"
    );
    assert_eq!(
        value["projects"]["/work/foreign"]["mcpServers"][NAME]["url"],
        "https://example.com/mcp/x"
    );

    disconnect(McpApp::ClaudeCode, &home).unwrap();
    let value = read();
    assert!(
        value["projects"]["/work/web"]["mcpServers"]
            .get(NAME)
            .is_none()
    );
    assert!(
        value["projects"]["/work/app"]["mcpServers"]
            .get(NAME)
            .is_none()
    );
    assert!(
        value["projects"]["/work/api"]["mcpServers"]
            .get(NAME)
            .is_some()
    );
    assert!(
        value["projects"]["/work/foreign"]["mcpServers"]
            .get(NAME)
            .is_some()
    );
}

#[test]
fn a_hand_made_project_entry_is_not_a_connection() {
    let home = temp_home("claude-not-connected");
    fs::write(
        McpApp::ClaudeCode.config_path(&home),
        json!({"projects": {"/work/api": {"mcpServers": {NAME: {
            "command": BINARY, "args": ["--workspace", "/work/api"]
        }}}}})
        .to_string(),
    )
    .unwrap();
    let claude = status(&home, &target(URL))
        .into_iter()
        .find(|s| s.app == McpApp::ClaudeCode)
        .unwrap();
    assert!(!claude.connected);
}
