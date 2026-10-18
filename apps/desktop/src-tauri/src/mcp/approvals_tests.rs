#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use plumbq_core::permissions::Permission;

use super::*;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ac-test-desktop-approvals-{name}-{}",
        &plumbq_mcp::settings::new_token()[..12]
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn request(strong: bool) -> ApprovalRequest {
    let mut request = ApprovalRequest {
        id: String::new(),
        created_at: String::new(),
        actor: "claude-code".into(),
        actor_label: String::new(),
        workspace_name: String::new(),
        workspace: "/home/ada/shop-api".into(),
        tool: "requests_send".into(),
        permission: Permission::SendWrite,
        action: "send DELETE {{baseUrl}}/users/7".into(),
        environment: Some("prod".into()),
        production: strong,
        strong,
        confirm_name: None,
        always_allow: None,
        detail: String::new(),
        lines: Vec::new(),
        warnings: Vec::new(),
        reason: None,
    };
    approvals::stamp(&mut request);
    request
}

fn add(queue: &Approvals, request: ApprovalRequest) -> oneshot::Receiver<bool> {
    let (respond, answer) = oneshot::channel();
    queue.add(Approval { request, respond });
    answer
}

/// Files a request like the stdio server does and returns once the app can see it.
async fn file(dir: &Path) -> tokio::task::JoinHandle<bool> {
    let data_dir = dir.to_path_buf();
    let before = approvals::pending(dir).unwrap().len();
    let waiting = tokio::spawn(async move {
        approvals::request_and_wait(&data_dir, request(false), Duration::from_secs(10))
            .await
            .unwrap()
    });
    for _ in 0..200 {
        if approvals::pending(dir).unwrap().len() > before {
            return waiting;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the request was never filed");
}

#[test]
fn strong_requests_in_the_app_need_the_typed_name() {
    let queue = Approvals::new(temp_dir("strong"));
    let asked = request(true);
    let id = asked.id.clone();
    let mut answer = add(&queue, asked);

    for typed in [None, Some("production")] {
        let err = queue.answer(&id, true, typed).unwrap_err();
        assert_eq!(err.kind, "needsConfirmation");
    }
    assert_eq!(queue.pending().len(), 1, "it still waits");
    assert!(answer.try_recv().is_err(), "no answer was sent");

    queue.answer(&id, true, Some("prod")).unwrap();
    assert_eq!(answer.try_recv(), Ok(true));
    assert!(queue.pending().is_empty());
}

#[test]
fn denying_never_needs_a_name() {
    let queue = Approvals::new(temp_dir("deny"));
    let asked = request(true);
    let id = asked.id.clone();
    let mut answer = add(&queue, asked);
    queue.answer(&id, false, None).unwrap();
    assert_eq!(answer.try_recv(), Ok(false));
}

#[tokio::test]
async fn requests_stay_in_the_order_they_arrived() {
    let dir = temp_dir("order");
    let queue = Approvals::new(dir.clone());
    let mut late_clock = request(false);
    late_clock.created_at = "2999-01-01T00:00:00.000Z".into();
    let first = late_clock.id.clone();
    let _answer = add(&queue, late_clock);
    let waiting = file(&dir).await;

    let ids: Vec<String> = queue.pending().into_iter().map(|r| r.id).collect();
    assert_eq!(ids.len(), 2);
    assert_eq!(
        ids[0], first,
        "the file's earlier timestamp doesn't move it ahead"
    );

    queue.answer(&ids[1], false, None).unwrap();
    assert!(!waiting.await.unwrap());
}

#[tokio::test]
async fn strong_file_requests_are_checked_too() {
    let dir = temp_dir("strong-file");
    let queue = Approvals::new(dir.clone());
    let data_dir = dir.clone();
    let waiting = tokio::spawn(async move {
        approvals::request_and_wait(&data_dir, request(true), Duration::from_secs(10))
            .await
            .unwrap()
    });
    let mut filed = None;
    for _ in 0..200 {
        filed = queue.pending().into_iter().next();
        if filed.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let filed = filed.expect("the request was filed");
    let err = queue.answer(&filed.id, true, None).unwrap_err();
    assert_eq!(err.kind, "needsConfirmation");
    queue.answer(&filed.id, true, Some("prod")).unwrap();
    assert!(waiting.await.unwrap());
}

#[tokio::test]
async fn requests_whose_call_was_cancelled_leave_the_list() {
    let dir = temp_dir("cancelled");
    let queue = Approvals::new(dir.clone());
    let answer = add(&queue, request(false));
    let waiting = file(&dir).await;
    assert_eq!(queue.pending().len(), 2);

    drop(answer);
    waiting.abort();
    let _ = waiting.await;
    assert!(queue.pending().is_empty());
}
