#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::Duration;

use plumbq_assistant::tools::Tools;
use plumbq_core::permissions::Permission;
use rmcp::RoleClient;
use rmcp::ServiceExt;
use rmcp::service::RunningService;
use rmcp::transport::StreamableHttpClientTransport;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::*;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ac-test-desktop-mcp-{name}-{}",
        &settings::new_token()[..12]
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

/// The app's MCP state on a temp workspace, with the server on.
fn state(name: &str) -> (Arc<McpState>, mpsc::Receiver<Approval>, PathBuf) {
    plumbq_core::http_client::install_crypto();
    let root = temp_dir(&format!("{name}-ws"));
    std::fs::create_dir_all(root.join("users")).unwrap();
    let data_dir = temp_dir(&format!("{name}-data"));
    let on = McpSettings {
        enabled: true,
        port: free_port(),
        ..McpSettings::default()
    };
    settings::save(&settings::path(&data_dir), &on).unwrap();
    let (state, approvals) = McpState::load(&data_dir, Arc::new(RwLock::new(Some(root.clone()))));
    (state, approvals, root)
}

async fn client(url: &str) -> RunningService<RoleClient, ()> {
    ().serve(StreamableHttpClientTransport::from_uri(url.to_string()))
        .await
        .unwrap()
}

/// Shows approval requests in the "app" like `forward` does, without a window.
fn show_approvals(state: &Arc<McpState>, mut approvals: mpsc::Receiver<Approval>) {
    let state = state.clone();
    tokio::spawn(async move {
        while let Some(approval) = approvals.recv().await {
            state.approvals.add(approval);
        }
    });
}

async fn first_pending(state: &McpState) -> ApprovalRequest {
    for _ in 0..300 {
        if let Some(first) = state.pending().into_iter().next() {
            return first;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("nothing asked for approval");
}

#[tokio::test(flavor = "multi_thread")]
async fn permission_changes_keep_the_server_and_its_sessions() {
    let (state, _approvals, _) = state("keep");
    state.start();
    let url = state.status().url.expect("the server runs");
    let connected = client(&url).await;

    let changes: [fn(&mut McpSettings); 3] = [
        |s| s.read_only = true,
        |s| s.hidden_environments = vec!["prod".into()],
        |s| s.permissions.send_write = PermissionRule::Deny,
    ];
    for change in changes {
        let mut settings = state.status().settings;
        change(&mut settings);
        let status = state.update(settings).await.unwrap();
        assert_eq!(
            status.url.as_deref(),
            Some(url.as_str()),
            "{:?}",
            status.error
        );
        connected
            .list_all_tools()
            .await
            .expect("the session lives on");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_address_moves_the_server_on_the_same_port() {
    let (state, _approvals, _) = state("move");
    state.start();
    for _ in 0..5 {
        let before = state.status();
        let status = state
            .update(McpSettings {
                token: settings::new_token(),
                ..before.settings.clone()
            })
            .await
            .unwrap();
        assert_eq!(status.error, None);
        let url = status.url.expect("the server runs on the new address");
        assert_ne!(Some(&url), before.url.as_ref());
        assert!(url.contains(&format!(":{}/", before.settings.port)));
        client(&url).await.list_all_tools().await.unwrap();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn turning_ai_off_stops_the_server_until_it_is_turned_on() {
    let (state, _approvals, _) = state("off");
    state.start();
    let on = state.status().settings;
    let off = state
        .update(McpSettings {
            turned_off: true,
            ..on.clone()
        })
        .await
        .unwrap();
    assert_eq!(off.url, None);
    assert!(
        off.settings.enabled,
        "the server stays enabled for when AI is back"
    );
    let back = state.update(on).await.unwrap();
    assert!(back.url.is_some(), "{:?}", back.error);
}

#[test]
fn a_server_never_starts_while_ai_is_off() {
    let (state, _approvals, _) = state("start-off");
    let mut off = state.status().settings;
    off.turned_off = true;
    state.save(off).unwrap();
    state.start();
    assert_eq!(state.status().url, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn turning_ai_off_denies_what_waits_and_shows_nothing_new() {
    let (state, approvals, root) = state("deny");
    show_approvals(&state, approvals);
    let tools = Arc::new(
        Tools::connect(plumbq_mcp::in_app(state.http_server()), "Chat · test")
            .await
            .unwrap(),
    );
    let waiting = tools.clone();
    let call = tokio::spawn(async move {
        let args = json!({"folder": "users", "content": "### One\nGET https://api.test/one"});
        waiting
            .call("requests_create", args, &CancellationToken::new())
            .await
            .unwrap()
    });
    first_pending(&state).await;

    let mut off = state.status().settings;
    off.turned_off = true;
    state.update(off).await.unwrap();
    let (text, ok) = call.await.unwrap();
    assert!(!ok && text.contains("denied"), "{text}");
    assert!(state.pending().is_empty());
    assert!(!root.join("users/one.http").exists());

    let (text, ok) = tools
        .call("workspace_tree", json!({}), &CancellationToken::new())
        .await
        .unwrap();
    assert!(
        !ok && text.contains("turned off"),
        "even reads are refused: {text}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn always_allow_lets_later_calls_of_that_kind_run_without_asking() {
    let (state, approvals, root) = state("always");
    show_approvals(&state, approvals);
    let tools = Arc::new(
        Tools::connect(plumbq_mcp::in_app(state.http_server()), "Chat · test")
            .await
            .unwrap(),
    );
    let create = |name: &'static str| {
        let tools = tools.clone();
        let content = format!("### {name}\nGET https://api.test/{name}");
        async move {
            let args = json!({"folder": "users", "content": content});
            tools
                .call("requests_create", args, &CancellationToken::new())
                .await
                .unwrap()
        }
    };
    let first = tokio::spawn(create("one"));
    let asked = first_pending(&state).await;
    let offer = asked.always_allow.clone().expect("always allow is offered");
    assert_eq!(offer.permission, Permission::EditFiles);

    let status = state
        .answer(&asked.id, ApprovalAnswer::AlwaysAllow, None)
        .unwrap();
    assert_eq!(
        status.settings.permissions.edit_files,
        PermissionRule::Allow
    );
    assert!(first.await.unwrap().1);
    let second = tokio::time::timeout(Duration::from_secs(5), create("two"))
        .await
        .expect("the second call didn't ask");
    assert!(second.1, "{}", second.0);
    assert!(root.join("users/two.http").exists());
    let saved = settings::load(&settings::path(state.data_dir())).unwrap();
    assert_eq!(saved.permissions.edit_files, PermissionRule::Allow);
}

#[test]
fn always_allow_is_refused_for_strong_requests() {
    let (state, _approvals, _) = state("strong");
    let mut request = ApprovalRequest {
        id: String::new(),
        created_at: String::new(),
        actor: "claude-code".into(),
        actor_label: String::new(),
        workspace_name: String::new(),
        workspace: "/ws".into(),
        tool: "requests_send".into(),
        permission: Permission::SendWrite,
        action: "send DELETE {{baseUrl}}/users/7".into(),
        environment: Some("prod".into()),
        production: true,
        strong: true,
        confirm_name: None,
        always_allow: None,
        detail: String::new(),
        lines: Vec::new(),
        warnings: Vec::new(),
        reason: None,
    };
    requests::stamp(&mut request);
    assert!(request.always_allow.is_none(), "not offered");
    let id = request.id.clone();
    let (respond, _answer) = tokio::sync::oneshot::channel();
    state.approvals.add(Approval { request, respond });

    let err = state
        .answer(&id, ApprovalAnswer::AlwaysAllow, Some("prod"))
        .unwrap_err();
    assert_eq!(err.kind, "invalid");
    assert_eq!(state.pending().len(), 1, "it still waits");
    assert_eq!(
        state.status().settings.permissions.send_write,
        PermissionRule::Ask
    );
}

#[cfg(unix)]
#[test]
fn always_allow_keeps_the_rules_when_the_answer_cant_be_written() {
    use std::os::unix::fs::PermissionsExt;

    let (state, _approvals, _) = state("always-failed");
    let mut request = ApprovalRequest {
        id: String::new(),
        created_at: String::new(),
        actor: "codex".into(),
        actor_label: String::new(),
        workspace_name: String::new(),
        workspace: "/ws".into(),
        tool: "requests_create".into(),
        permission: Permission::EditFiles,
        action: "create users/one.http".into(),
        environment: None,
        production: false,
        strong: false,
        confirm_name: None,
        always_allow: None,
        detail: String::new(),
        lines: Vec::new(),
        warnings: Vec::new(),
        reason: None,
    };
    requests::stamp(&mut request);
    let filed = state.data_dir().join("approvals");
    std::fs::create_dir_all(&filed).unwrap();
    std::fs::write(
        filed.join(format!("{}.json", request.id)),
        serde_json::to_string(&request).unwrap(),
    )
    .unwrap();
    std::fs::set_permissions(&filed, std::fs::Permissions::from_mode(0o555)).unwrap();

    let answered = state.answer(&request.id, ApprovalAnswer::AlwaysAllow, None);
    std::fs::set_permissions(&filed, std::fs::Permissions::from_mode(0o700)).unwrap();

    assert!(answered.is_err(), "the answer file can't be written");
    assert_eq!(
        state.status().settings.permissions.edit_files,
        PermissionRule::Ask
    );
    let saved = settings::load(&settings::path(state.data_dir())).unwrap();
    assert_eq!(saved.permissions.edit_files, PermissionRule::Ask);
}
