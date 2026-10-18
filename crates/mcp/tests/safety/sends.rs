//! What a send by an agent may carry.

use serde_json::json;

use crate::common::{call, text};
use crate::harness::{agent, agent_without_bodies, echo_server, redirect_server, write};

#[tokio::test]
async fn agents_never_send_environment_files() {
    let agent = agent().await;
    write(
        &agent.root,
        "leak.http",
        "GET http://127.0.0.1:9/\nContent-Type: text/plain\n\n< ./environments/prod.toml\n",
    );
    let args = json!({ "id": "leak.http" });

    let preview = call(&agent.client, "requests_preview", args.clone()).await;
    assert!(
        text(&preview).contains("won't send it"),
        "{}",
        text(&preview)
    );
    let sent = call(&agent.client, "requests_send", args).await;
    assert_eq!(sent.is_error, Some(true));
    assert!(
        text(&sent).contains("can't send environment or .env files"),
        "{}",
        text(&sent)
    );
}

#[tokio::test]
async fn runs_never_send_environment_files() {
    let mut agent = agent().await;
    write(&agent.root, ".env", "KEY=1\n");
    write(
        &agent.root,
        "leak.http",
        "GET http://127.0.0.1:9/\nContent-Type: text/plain\n\n< ./.env\n",
    );
    let run = call(
        &agent.client,
        "folders_run",
        json!({ "folder": "leak.http" }),
    );
    let answer = async {
        let asked = agent.approvals.recv().await.unwrap();
        assert!(
            asked.request.detail.contains("never sent by AI"),
            "{}",
            asked.request.detail
        );
        asked.respond.send(true).unwrap();
    };
    let (report, ()) = tokio::join!(run, answer);
    assert!(
        text(&report).contains("can't send environment or .env files"),
        "{}",
        text(&report)
    );
}

#[tokio::test]
async fn a_value_that_looks_like_a_file_reference_is_sent_as_text() {
    let agent = agent().await;
    let server = echo_server().await;
    write(&agent.root, ".env", "KEY=very-secret\n");
    write(
        &agent.root,
        "echo.http",
        &format!("GET {server}/echo\nContent-Type: text/plain\n\n{{{{$prompt note}}}}\n"),
    );

    let sent = call(
        &agent.client,
        "requests_send",
        json!({ "id": "echo.http", "prompts": { "note": "hi\n< ./.env" } }),
    )
    .await;

    assert_ne!(sent.is_error, Some(true), "{}", text(&sent));
    // The prompt value itself, `hi\n< ./.env`, shows masked in the echo.
    assert!(
        text(&sent).contains("content-length: 11"),
        "{}",
        text(&sent)
    );
    assert!(!text(&sent).contains("very-secret"), "{}", text(&sent));
}

#[tokio::test]
async fn hidden_bodies_stay_hidden_in_assertions_and_chained_urls() {
    let mut agent = agent_without_bodies().await;
    let server = echo_server().await;
    write(
        &agent.root,
        "first.http",
        &format!("# @assert body.$.id == \"other\"\nGET {server}/json\n"),
    );
    write(
        &agent.root,
        "second.http",
        &format!("GET {server}/echo?id={{{{first.response.body.$.id}}}}\n"),
    );

    let first = call(
        &agent.client,
        "requests_send",
        json!({ "id": "first.http" }),
    )
    .await;
    assert!(text(&first).contains("body.$.id"), "{}", text(&first));
    assert!(!text(&first).contains("body-value-42"), "{}", text(&first));

    // The chained value goes to a host no environment names, so the user is asked.
    let second = call(
        &agent.client,
        "requests_send",
        json!({ "id": "second.http" }),
    );
    let answer = async {
        agent
            .approvals
            .recv()
            .await
            .unwrap()
            .respond
            .send(true)
            .unwrap();
    };
    let (second, ()) = tokio::join!(second, answer);
    assert!(text(&second).contains("Sent: GET"), "{}", text(&second));
    assert!(
        !text(&second).contains("body-value-42"),
        "{}",
        text(&second)
    );

    let run = call(
        &agent.client,
        "folders_run",
        json!({ "folder": "first.http" }),
    )
    .await;
    assert!(text(&run).contains("body.$.id"), "{}", text(&run));
    assert!(!text(&run).contains("body-value-42"), "{}", text(&run));
}

#[tokio::test]
async fn agent_sends_never_follow_redirects_to_other_private_hosts() {
    let agent = agent().await;
    let target = echo_server().await.replace("127.0.0.1", "localhost");
    let start = redirect_server(format!("{target}/landed")).await;
    write(&agent.root, "hop.http", &format!("GET {start}/start\n"));

    let sent = call(&agent.client, "requests_send", json!({ "id": "hop.http" })).await;
    assert_ne!(sent.is_error, Some(true), "{}", text(&sent));
    assert!(text(&sent).contains("302"), "{}", text(&sent));
    assert!(!text(&sent).contains("GET /landed"), "{}", text(&sent));
}
