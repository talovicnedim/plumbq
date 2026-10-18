use std::time::Instant;

use super::tests::{options, run_all, server, workspace};
use super::*;
use crate::history;
use crate::workspace::tests::{temp_workspace, write};

#[tokio::test]
async fn data_rows_become_variables_per_iteration() {
    let root = workspace("run-data", &server().await);
    write(
        &root,
        "user.http",
        "### User\n# @assert body.$.path startsWith \"/users/\"\nGET {{base}}/users/{{id}}\n",
    );
    let history = temp_workspace("run-data-history");
    let data = parse_data("id\n7\n8\n", false).unwrap();
    let opts = RunOptions {
        data,
        context: SendContext {
            history_dir: Some(history.clone()),
            ..Default::default()
        },
        ..options("local")
    };

    let report = run_all(&root, "user.http", &opts).await;

    let iterations: Vec<u32> = report.outcomes.iter().map(|o| o.iteration).collect();
    assert_eq!(iterations, [1, 2]);
    assert!(
        report.outcomes.iter().all(|o| !o.failed()),
        "{:?}",
        report.outcomes
    );
    let paths: Vec<String> = report
        .outcomes
        .iter()
        .map(|o| {
            let id = o.history_id.clone().unwrap();
            history::get(&history, &root, "user.http", &id)
                .unwrap()
                .response
                .sent
                .url
        })
        .collect();
    assert!(
        paths[0].ends_with("/users/7") && paths[1].ends_with("/users/8"),
        "{paths:?}"
    );
}

#[tokio::test]
async fn data_rows_can_be_checked_in_assertions() {
    let root = workspace("run-data-assert", &server().await);
    write(
        &root,
        "user.http",
        "### User\n# @assert body.$.path == \"/users/{{id}}\"\nGET {{base}}/users/{{id}}\n",
    );
    let opts = RunOptions {
        data: parse_data("id\n7\n8\n", false).unwrap(),
        ..options("local")
    };

    let report = run_all(&root, "user.http", &opts).await;

    assert_eq!(report.outcomes.len(), 2);
    for outcome in &report.outcomes {
        assert!(!outcome.failed(), "{:?}", outcome.assertions);
    }
}

#[tokio::test]
async fn iterations_repeat_and_delay_spaces_requests() {
    let root = workspace("run-delay", &server().await);
    let opts = RunOptions {
        iterations: 2,
        delay_ms: 40,
        ..options("local")
    };
    let started = Instant::now();
    let report = run_all(&root, "health.http", &opts).await;
    assert_eq!(report.outcomes.len(), 2);
    assert!(started.elapsed().as_millis() >= 40);
}

#[tokio::test]
async fn secret_overrides_stay_masked_in_history() {
    let root = workspace("run-secret-var", &server().await);
    write(
        &root,
        "me.http",
        "### Me\nGET {{base}}/me\nAuthorization: Bearer {{token}}\n",
    );
    let history = temp_workspace("run-secret-var-history");
    let opts = RunOptions {
        context: SendContext {
            history_dir: Some(history.clone()),
            secrets: BTreeMap::from([("token".into(), "cli-secret-value".into())]),
            ..Default::default()
        },
        ..options("local")
    };

    let report = run_all(&root, "me.http", &opts).await;

    let id = report.outcomes[0].history_id.clone().unwrap();
    let entry = history::get(&history, &root, "me.http", &id).unwrap();
    let text = serde_json::to_string(&entry).unwrap();
    assert!(!text.contains("cli-secret-value"), "{text}");
    assert!(text.contains("Bearer {{token}}"));
}
