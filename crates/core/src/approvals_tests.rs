use super::*;
use crate::workspace::tests::temp_workspace;

pub(crate) fn request() -> ApprovalRequest {
    ApprovalRequest {
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
        detail: "DELETE https://api.test/users/7".into(),
        lines: Vec::new(),
        warnings: Vec::new(),
        reason: Some("clean up the test user".into()),
    }
}

#[tokio::test]
async fn waits_for_the_app_to_answer() {
    let dir = temp_workspace("approvals-answer");
    let waiting = {
        let dir = dir.clone();
        tokio::spawn(
            async move { request_and_wait(&dir, request(), Duration::from_secs(10)).await },
        )
    };
    let mut pending_requests = Vec::new();
    for _ in 0..100 {
        pending_requests = pending(&dir).unwrap();
        if !pending_requests.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(pending_requests.len(), 1);
    assert_eq!(
        pending_requests[0].action,
        "send DELETE {{baseUrl}}/users/7"
    );
    answer(&dir, &pending_requests[0].id, true, Some("prod")).unwrap();
    assert!(waiting.await.unwrap().unwrap());
    assert!(pending(&dir).unwrap().is_empty());
}

async fn first_pending(dir: &Path) -> ApprovalRequest {
    for _ in 0..100 {
        if let Some(first) = pending(dir).unwrap().into_iter().next() {
            return first;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("no request was filed");
}

#[tokio::test]
async fn strong_requests_need_the_typed_name() {
    let dir = temp_workspace("approvals-strong");
    let waiting = {
        let dir = dir.clone();
        tokio::spawn(
            async move { request_and_wait(&dir, request(), Duration::from_secs(10)).await },
        )
    };
    let filed = first_pending(&dir).await;
    assert_eq!(filed.confirm_name.as_deref(), Some("prod"));
    for typed in [None, Some("production"), Some("")] {
        let err = answer(&dir, &filed.id, true, typed).unwrap_err();
        assert_eq!(err.kind(), "needsConfirmation");
    }
    assert_eq!(pending(&dir).unwrap().len(), 1, "still waiting");
    answer(&dir, &filed.id, true, Some("prod")).unwrap();
    assert!(waiting.await.unwrap().unwrap());
}

#[test]
fn the_name_to_type_is_the_environment_or_the_workspace_folder() {
    let mut no_env = ApprovalRequest {
        environment: None,
        workspace: "/home/ada/shop-api".into(),
        ..request()
    };
    stamp(&mut no_env);
    assert_eq!(no_env.confirm_name.as_deref(), Some("shop-api"));
    let mut plain = ApprovalRequest {
        strong: false,
        ..request()
    };
    stamp(&mut plain);
    assert_eq!(plain.confirm_name, None);
    assert!(
        check_typed(&plain, None).is_ok(),
        "denying or a plain approve needs no name"
    );
}

#[tokio::test]
async fn no_answer_means_denied() {
    let dir = temp_workspace("approvals-timeout");
    let approved = request_and_wait(&dir, request(), Duration::from_millis(300))
        .await
        .unwrap();
    assert!(!approved);
    assert!(
        pending(&dir).unwrap().is_empty(),
        "the request is cleaned up"
    );
}

#[tokio::test]
async fn a_request_whose_caller_stopped_waiting_is_removed() {
    let dir = temp_workspace("approvals-cancelled");
    let waiting = {
        let dir = dir.clone();
        tokio::spawn(
            async move { request_and_wait(&dir, request(), Duration::from_secs(60)).await },
        )
    };
    first_pending(&dir).await;
    waiting.abort();
    let _ = waiting.await;
    assert!(pending(&dir).unwrap().is_empty());
}

#[test]
fn requests_left_by_a_stopped_server_are_removed() {
    let dir = temp_workspace("approvals-orphans");
    let mut old = request();
    stamp(&mut old);
    let file = request_path(&dir, &old.id).unwrap();
    fs_util::write_private(&file, serde_json::to_string(&old).unwrap()).unwrap();
    let long_ago = std::time::SystemTime::now() - DEFAULT_TIMEOUT * 3;
    fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_modified(long_ago)
        .unwrap();
    let mut fresh = request();
    stamp(&mut fresh);
    let fresh_file = request_path(&dir, &fresh.id).unwrap();
    fs_util::write_private(&fresh_file, serde_json::to_string(&fresh).unwrap()).unwrap();

    let ids: Vec<String> = pending(&dir).unwrap().into_iter().map(|r| r.id).collect();
    assert_eq!(ids, [fresh.id]);
    assert!(!file.exists());
}

#[test]
fn at_most_keep_requests_wait() {
    let dir = temp_workspace("approvals-keep");
    for _ in 0..KEEP + 3 {
        let mut waiting = request();
        stamp(&mut waiting);
        let file = request_path(&dir, &waiting.id).unwrap();
        fs_util::write_private(&file, serde_json::to_string(&waiting).unwrap()).unwrap();
    }
    assert_eq!(pending(&dir).unwrap().len(), KEEP);
    assert_eq!(fs::read_dir(dir.join("approvals")).unwrap().count(), KEEP);
}

#[test]
fn rejects_ids_that_could_escape_the_folder() {
    let dir = temp_workspace("approvals-ids");
    assert_eq!(
        answer(&dir, "../x", true, None).unwrap_err().kind(),
        "invalid"
    );
    assert_eq!(answer(&dir, "", true, None).unwrap_err().kind(), "invalid");
    assert_eq!(
        answer(&dir, "nope", true, None).unwrap_err().kind(),
        "notFound"
    );
}

#[test]
fn always_allow_is_offered_only_where_a_rule_can_cover_the_call() {
    let offered = |request: ApprovalRequest| {
        let mut request = request;
        stamp(&mut request);
        request.always_allow.map(|offer| offer.permission)
    };
    let plain = ApprovalRequest {
        strong: false,
        production: false,
        permission: Permission::EditFiles,
        ..request()
    };
    assert_eq!(offered(plain.clone()), Some(Permission::EditFiles));
    assert_eq!(offered(request()), None, "strong requests ask every time");
    let warned = ApprovalRequest {
        warnings: vec!["secrets go to a new host".into()],
        ..plain.clone()
    };
    assert_eq!(offered(warned), None);
    let delete = ApprovalRequest {
        permission: Permission::Destructive,
        ..plain.clone()
    };
    assert_eq!(offered(delete), None);
    assert!(check_always_allow(&request()).is_err());
    assert_eq!(check_always_allow(&plain).unwrap(), Permission::EditFiles);
}
