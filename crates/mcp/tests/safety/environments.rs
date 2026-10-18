//! Hidden and production environments stay out of the agent's reach.

use serde_json::json;

use crate::common::{call, text};
use crate::harness::{agent, write};

#[tokio::test]
async fn hidden_environments_stay_hidden_in_any_letter_case() {
    let mut agent = agent().await;
    agent.settings.write().unwrap().hidden_environments = vec!["prod".into()];

    for name in ["PROD", "Prod"] {
        let preview = call(
            &agent.client,
            "requests_preview",
            json!({ "id": "users/list.http", "environment": name }),
        )
        .await;
        assert_eq!(preview.is_error, Some(true), "{name}: {}", text(&preview));
        assert!(
            !text(&preview).contains("prod-only.test"),
            "{}",
            text(&preview)
        );

        let set = call(
            &agent.client,
            "environments_set_variable",
            json!({ "environment": name, "name": "x", "value": "1" }),
        )
        .await;
        assert_eq!(set.is_error, Some(true), "{name}: {}", text(&set));
    }
    assert!(agent.approvals.try_recv().is_err(), "nothing was asked");
    let file = std::fs::read_to_string(agent.root.join("environments/prod.toml")).unwrap();
    assert!(!file.contains("x = "), "{file}");
}

#[tokio::test]
async fn refusals_never_name_a_hidden_environment() {
    let mut agent = agent().await;
    write(
        &agent.root,
        "environments/zebra.toml",
        "tag = \"production\"\n[variables]\nbaseUrl = \"https://vault.test\"\n",
    );
    agent.settings.write().unwrap().hidden_environments = vec!["zebra".into()];
    write(&agent.root, "wipe.http", "DELETE https://vault.test/all\n");

    let send = call(&agent.client, "requests_send", json!({ "id": "wipe.http" }));
    let deny = async {
        let asked = agent.approvals.recv().await.unwrap();
        assert!(asked.request.strong, "a production write");
        asked.respond.send(false).unwrap();
    };
    let (sent, ()) = tokio::join!(send, deny);
    assert_eq!(sent.is_error, Some(true));
    assert!(
        text(&sent).contains("production environment"),
        "{}",
        text(&sent)
    );
    assert!(!text(&sent).contains("zebra"), "{}", text(&sent));
}

#[tokio::test]
async fn imports_show_their_environments_and_never_add_to_production() {
    let mut agent = agent().await;
    let collection = r#"{"info": {"name": "prod",
        "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"},
        "item": [{"name": "Ping", "request": {"method": "GET", "url": "https://x.test/ping"}}],
        "variable": [{"key": "exfil", "value": "https://evil.test"}]}"#;
    write(&agent.root, "prod.postman.json", collection);
    let args = json!({ "folder": "", "path": "prod.postman.json" });

    let first = call(&agent.client, "import_file", args.clone());
    let approve = async {
        let asked = tokio::time::timeout(std::time::Duration::from_secs(5), agent.approvals.recv())
            .await
            .expect("the first import is asked")
            .unwrap();
        assert!(
            asked.request.detail.contains("creates environment prod-2"),
            "{}",
            asked.request.detail
        );
        assert!(asked.request.detail.contains("exfil = https://evil.test"));
        asked.respond.send(true).unwrap();
    };
    let (first, ()) = tokio::join!(first, approve);
    assert_ne!(first.is_error, Some(true), "{}", text(&first));

    let before = std::fs::read_to_string(agent.root.join("environments/prod.toml")).unwrap();
    let again = call(&agent.client, "import_file", args);
    let approve_anything = async {
        while let Some(asked) = agent.approvals.recv().await {
            let _ = asked.respond.send(true);
        }
    };
    let again = tokio::select! {
        again = again => again,
        () = approve_anything => panic!("the approvals channel closed"),
    };
    assert_eq!(again.is_error, Some(true), "{}", text(&again));
    assert!(
        text(&again).contains("production environment prod"),
        "{}",
        text(&again)
    );
    let after = std::fs::read_to_string(agent.root.join("environments/prod.toml")).unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
async fn the_wrong_case_hint_only_names_environments_the_agent_may_see() {
    let agent = agent().await;
    write(
        &agent.root,
        "environments/staging.toml",
        "tag = \"staging\"\n",
    );
    agent.settings.write().unwrap().hidden_environments = vec!["prod".into()];

    let wrong = call(
        &agent.client,
        "requests_preview",
        json!({ "id": "users/list.http", "environment": "Staging" }),
    )
    .await;
    assert_eq!(wrong.is_error, Some(true));
    assert!(
        text(&wrong).contains("did you mean staging?"),
        "{}",
        text(&wrong)
    );
    assert!(!text(&wrong).contains("prod"), "{}", text(&wrong));

    let hidden = call(
        &agent.client,
        "requests_preview",
        json!({ "id": "users/list.http", "environment": "Prod" }),
    )
    .await;
    assert!(!text(&hidden).contains("did you mean"), "{}", text(&hidden));
}
