use plumbq_core::permissions::{self, AgentPermissions, Permission, PermissionContext};

use super::*;

impl PlumbqMcp {
    /// A server on a fixed workspace with default settings and nobody to ask.
    pub(crate) fn test(root: PathBuf) -> Self {
        Self::new(Workspace::Fixed(root), Arc::default(), Approver::Nobody)
    }
}

fn server(read_only: bool) -> PlumbqMcp {
    let server = PlumbqMcp::test(PathBuf::from("/ws"));
    if read_only {
        server.read_only()
    } else {
        server
    }
}

#[test]
fn every_tool_says_whether_it_changes_files() {
    for tool in server(false).tool_router.list_all() {
        let hint = tool.annotations.as_ref().and_then(|a| a.read_only_hint);
        assert!(hint.is_some(), "{} has no read_only_hint", tool.name);
    }
}

#[test]
fn read_only_mode_offers_only_reads_and_sends() {
    let all = server(false).tool_router.list_all();
    let read_only = server(true).tool_names();

    let writes: Vec<_> = all.iter().filter(|t| !offered_read_only(t)).collect();
    assert!(writes.len() > 15, "write tools are missing their hints");
    for tool in writes {
        assert!(!read_only.contains(&tool.name.to_string()), "{}", tool.name);
    }
    for name in [
        "workspace_tree",
        "requests_send",
        "folders_run",
        "requests_preview",
    ] {
        assert!(read_only.contains(&name.to_string()), "{name}");
    }
    for name in [
        "requests_update",
        "folders_remove",
        "cookies_clear",
        "examples_create",
    ] {
        assert!(!read_only.contains(&name.to_string()), "{name}");
    }
    assert_eq!(server(true).permissions(), AgentPermissions::read_only());
}

#[test]
fn a_shared_workspace_follows_the_app() {
    let open = Arc::new(RwLock::new(None));
    let server = PlumbqMcp::new(
        Workspace::Shared(open.clone()),
        Arc::default(),
        Approver::Nobody,
    );
    assert!(server.root().unwrap_err().contains("no workspace is open"));
    *open.write().unwrap() = Some(PathBuf::from("/ws"));
    assert_eq!(server.root().unwrap(), PathBuf::from("/ws"));
}

#[tokio::test]
async fn authorize_follows_the_policy_and_asks_when_needed() {
    let (approver, mut rx) = Approver::channel();
    let server = PlumbqMcp::new(
        Workspace::Fixed(PathBuf::from("/ws")),
        Arc::default(),
        approver,
    );
    let call = |permission, production| Call {
        tool: "requests_send",
        permission,
        context: PermissionContext {
            production,
            ..PermissionContext::default()
        },
        action: "send DELETE /users/7".into(),
        detail: "DELETE http://x.test/users/7".into(),
        environment: Some("prod".into()),
        reason: Some("tidy up".into()),
    };
    // Safe sends run without asking.
    server
        .authorize(call(Permission::SendSafe, false))
        .await
        .unwrap();

    let host = tokio::spawn(async move {
        let asked = rx.recv().await.unwrap();
        assert!(
            asked.request.strong,
            "production writes need the strong confirmation"
        );
        assert_eq!(asked.request.reason.as_deref(), Some("tidy up"));
        asked.respond.send(true).unwrap();
        rx.recv().await.unwrap().respond.send(false).unwrap();
    });
    server
        .authorize(call(Permission::SendWrite, true))
        .await
        .unwrap();
    let denied = server
        .authorize(call(Permission::SendWrite, false))
        .await
        .unwrap_err();
    assert!(denied.contains("denied"), "{denied}");
    host.await.unwrap();

    let settings = Arc::new(RwLock::new(McpSettings::default()));
    settings.write().unwrap().permissions.destructive = permissions::PermissionRule::Deny;
    let strict = PlumbqMcp::new(
        Workspace::Fixed(PathBuf::from("/ws")),
        settings,
        Approver::Nobody,
    );
    let refused = strict
        .authorize(call(Permission::Destructive, false))
        .await
        .unwrap_err();
    assert!(refused.starts_with("refused"), "{refused}");
}

#[tokio::test]
async fn the_read_only_setting_and_hidden_environments_apply_to_every_call() {
    let settings = Arc::new(RwLock::new(McpSettings::default()));
    let server = PlumbqMcp::new(
        Workspace::Fixed(PathBuf::from("/ws")),
        settings.clone(),
        Approver::Nobody,
    );
    let call = |permission, environment: &str| Call {
        tool: "requests_send",
        permission,
        context: PermissionContext::default(),
        action: "send".into(),
        detail: String::new(),
        environment: Some(environment.into()),
        reason: None,
    };

    settings.write().unwrap().read_only = true;
    assert_eq!(server.permissions(), AgentPermissions::read_only());
    let refused = server
        .authorize(call(Permission::EditFiles, "dev"))
        .await
        .unwrap_err();
    assert!(refused.starts_with("refused"), "{refused}");
    server
        .authorize(call(Permission::SendSafe, "dev"))
        .await
        .unwrap();

    settings.write().unwrap().hidden_environments = vec!["prod".into()];
    let hidden = server
        .authorize(call(Permission::SendSafe, "prod"))
        .await
        .unwrap_err();
    assert!(hidden.contains("hidden"), "{hidden}");
    assert!(
        server.check_visible(Some("PROD")).is_err(),
        "any letter case"
    );
    assert!(server.check_visible(Some("dev")).is_ok());
    assert!(server.check_visible(None).is_ok());
}

#[test]
fn unreadable_saved_settings_refuse_every_call() {
    let dir = std::env::temp_dir().join(format!("ac-mcp-settings-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("mcp.json");
    let server = PlumbqMcp::test(PathBuf::from("/ws")).with_settings_file(Some(file.clone()));
    assert!(
        server.call_settings().is_ok(),
        "a missing file means defaults"
    );

    std::fs::write(&file, "{ not json").unwrap();
    let problem = server.call_settings().unwrap_err();
    assert!(problem.contains("can't be read"), "{problem}");

    let token = crate::settings::new_token();
    let saved = format!(r#"{{"enabled":true,"port":7459,"token":"{token}"}}"#);
    std::fs::write(&file, saved).unwrap();
    assert!(server.call_settings().is_ok());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn the_stdio_server_reads_its_settings_once_per_call() {
    let dir = std::env::temp_dir().join(format!(
        "ac-mcp-settings-once-{}",
        &crate::settings::new_token()[..12]
    ));
    let file = dir.join("mcp.json");
    crate::settings::save(&file, &McpSettings::default()).unwrap();
    let server = PlumbqMcp::test(PathBuf::from("/ws")).with_settings_file(Some(file.clone()));

    let record = call_log::CallRecord {
        settings: server.call_settings().unwrap(),
        ..call_log::CallRecord::default()
    };
    call_log::recorded(record, async {
        let changed = McpSettings {
            read_only: true,
            hidden_environments: vec!["prod".into()],
            ..crate::settings::load(&file).unwrap()
        };
        crate::settings::save(&file, &changed).unwrap();
        assert!(!server.is_read_only(), "the call goes on as it started");
        assert!(server.check_visible(Some("prod")).is_ok());
    })
    .await;

    assert!(server.is_read_only(), "the next call sees the change");
    assert!(server.check_visible(Some("prod")).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_read_only_setting_keeps_the_real_cookie_jar_like_the_flag() {
    let settings = Arc::new(RwLock::new(McpSettings::default()));
    let jar = std::env::temp_dir().join("ac-mcp-real-jar");
    let server = PlumbqMcp::new(
        Workspace::Fixed(PathBuf::from("/ws")),
        settings.clone(),
        Approver::Nobody,
    )
    .with_cookie_dir(Some(jar.clone()));
    let root = PathBuf::from("/ws");
    assert_eq!(
        server.send_context(&root, None).cookie_dir,
        Some(jar.clone())
    );

    settings.write().unwrap().read_only = true;
    assert_ne!(server.send_context(&root, None).cookie_dir, Some(jar));
}

#[tokio::test]
async fn servers_of_one_host_share_the_write_lock() {
    let host = crate::HttpServer {
        workspace: Arc::default(),
        data_dir: std::env::temp_dir().join("ac-mcp-shared-lock"),
        settings: Arc::default(),
        approver: Approver::Nobody,
        write_lock: crate::WriteLock::default(),
    };
    let chat = crate::in_app(host.clone()).for_chat();
    let http = crate::in_app(host);

    let _held = chat.write_lock().await;
    assert!(http.write_lock.try_lock().is_err(), "a second writer waits");
}

#[test]
fn text_from_outside_is_one_line_in_approvals_and_cut() {
    assert_eq!(super::shown("a\nb\r\tc\u{1b}[2J"), r"a\nb\r\tc\u{1b}[2J");
    let long = super::shown(&"x".repeat(1000));
    assert!(long.ends_with("... (cut)"), "{long}");
    assert!(long.len() < 400);
}

#[test]
fn format_characters_cant_reorder_approval_text() {
    for c in [
        '\u{200b}', '\u{200f}', '\u{2028}', '\u{2029}', '\u{202e}', '\u{2066}', '\u{feff}',
    ] {
        let shown = super::shown(&format!("a{c}b"));
        assert_eq!(shown, format!("a\\u{{{:x}}}b", u32::from(c)));
    }
    assert_eq!(super::shown("café ✓"), "café ✓");
}

#[tokio::test]
async fn the_agent_name_in_approvals_is_one_short_line() {
    let (approver, mut rx) = Approver::channel();
    let server = PlumbqMcp::new(
        Workspace::Fixed(PathBuf::from("/ws")),
        Arc::default(),
        approver,
    );
    let call = Call {
        tool: "requests_create",
        permission: Permission::EditFiles,
        context: PermissionContext::default(),
        action: "create users/new.http".into(),
        detail: String::new(),
        environment: None,
        reason: None,
    };
    let record = call_log::CallRecord {
        actor: format!("Claude Code\nwants nothing\u{202e}{}", "x".repeat(200)),
        ..call_log::CallRecord::default()
    };
    let host = tokio::spawn(async move {
        let asked = rx.recv().await.unwrap();
        asked.respond.send(false).unwrap();
        asked.request
    });
    let _ = call_log::recorded(record, server.authorize(call)).await;
    let asked = host.await.unwrap();
    for name in [&asked.actor, &asked.actor_label] {
        assert!(
            name.starts_with(r"Claude Code\nwants nothing\u{202e}x"),
            "{name}"
        );
        assert!(name.ends_with("... (cut)"), "{name}");
        assert!(name.chars().count() < 80, "{name}");
    }
}
