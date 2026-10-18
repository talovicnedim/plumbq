//! What the user approved is what happens, and only while the call is still wanted.

use rmcp::model::{CallToolRequest, CallToolRequestParams, ClientRequest};
use rmcp::service::PeerRequestOptions;
use serde_json::json;

use crate::common::{call, text};
use crate::harness::{agent, write};

#[tokio::test]
async fn a_send_that_changed_while_waiting_for_approval_is_refused() {
    let mut agent = agent().await;
    write(
        &agent.root,
        "environments/dev.toml",
        "[variables]\nbaseUrl = \"http://127.0.0.1:9\"\n",
    );
    write(&agent.root, "users/create.http", "POST {{baseUrl}}/users\n");
    let send = call(
        &agent.client,
        "requests_send",
        json!({ "id": "users/create.http", "environment": "dev" }),
    );
    let answer = async {
        let asked = agent.approvals.recv().await.unwrap();
        assert!(asked.request.detail.contains("127.0.0.1:9"));
        write(
            &agent.root,
            "environments/dev.toml",
            "[variables]\nbaseUrl = \"http://localhost:9\"\n",
        );
        asked.respond.send(true).unwrap();
    };
    let (sent, ()) = tokio::join!(send, answer);
    assert_eq!(sent.is_error, Some(true));
    assert!(
        text(&sent).contains("changed since the user approved it"),
        "{}",
        text(&sent)
    );
}

#[tokio::test]
async fn an_update_never_overwrites_a_file_that_changed_while_waiting_for_approval() {
    let mut agent = agent().await;
    let update = call(
        &agent.client,
        "requests_update",
        json!({ "id": "users/list.http", "content": "GET {{baseUrl}}/users?page=2\n" }),
    );
    let answer = async {
        let asked = agent.approvals.recv().await.unwrap();
        write(&agent.root, "users/list.http", "GET {{baseUrl}}/people\n");
        asked.respond.send(true).unwrap();
    };
    let (updated, ()) = tokio::join!(update, answer);
    assert_eq!(updated.is_error, Some(true), "{}", text(&updated));
    let file = std::fs::read_to_string(agent.root.join("users/list.http")).unwrap();
    assert_eq!(file, "GET {{baseUrl}}/people\n");
}

#[tokio::test]
async fn cancelling_a_call_withdraws_its_approval() {
    let mut agent = agent().await;
    let serde_json::Value::Object(args) =
        json!({ "folder": "users", "content": "### New\nGET https://api.test/new\n" })
    else {
        panic!("args must be an object");
    };
    let call =
        CallToolRequest::new(CallToolRequestParams::new("requests_create").with_arguments(args));
    let handle = agent
        .client
        .peer()
        .send_cancellable_request(
            ClientRequest::CallToolRequest(call),
            PeerRequestOptions::no_options(),
        )
        .await
        .unwrap();
    let mut asked = agent.approvals.recv().await.unwrap();

    handle.cancel(Some("stopped".into())).await.unwrap();

    tokio::time::timeout(std::time::Duration::from_secs(5), asked.respond.closed())
        .await
        .expect("the approval is withdrawn");
    assert!(!agent.root.join("users/new.http").exists());
}
