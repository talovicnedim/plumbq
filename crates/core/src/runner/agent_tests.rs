use super::tests::{options, server, workspace};
use super::*;
use crate::guard::SendAssessment;
use crate::requests;
use crate::workspace::tests::write;

/// Each selected request as the user saw it when approving the run.
fn approve(root: &Path, selection: &[Selected], env: &str) -> BTreeMap<String, SendAssessment> {
    selection
        .iter()
        .map(|item| {
            let request = requests::get(root, &item.id).unwrap();
            let seen = guard::assess_agent_send(root, &item.id, &request, Some(env)).unwrap();
            (item.id.clone(), seen)
        })
        .collect()
}

fn outcome<'a>(report: &'a RunReport, id: &str) -> &'a RunOutcome {
    report.outcomes.iter().find(|o| o.id == id).unwrap()
}

#[tokio::test]
async fn an_agent_run_refuses_requests_that_changed_since_the_approval() {
    let root = workspace("run-agent-changed", &server().await);
    let selection = select(&root, "", &[]).unwrap();
    let approved = approve(&root, &selection, "local");
    write(
        &root,
        "users/list.http",
        "### List\nDELETE {{base}}/users\n",
    );
    let agent = RunOptions {
        agent: Some(approved),
        ..options("local")
    };

    let report = run(&root, &selection, &agent, |_| true).await;

    let list = outcome(&report, "users/list.http");
    assert_eq!(list.status, None, "{list:?}");
    assert!(
        list.error.as_deref().is_some_and(|e| e.contains("changed")),
        "{list:?}"
    );
    assert_eq!(outcome(&report, "health.http").status, Some(200));
}

#[tokio::test]
async fn an_agent_run_confirms_only_the_production_writes_it_showed() {
    let root = workspace("run-agent-prod", &server().await);
    let selection = select(&root, "users", &[]).unwrap();
    let mut approved = approve(&root, &selection, "prod");
    approved.remove("users/broken.http");
    let agent = RunOptions {
        agent: Some(approved),
        ..options("prod")
    };

    let report = run(&root, &selection, &agent, |_| true).await;

    assert_eq!(outcome(&report, "users/remove.http").status, Some(200));
    let unseen = outcome(&report, "users/broken.http");
    assert_eq!(unseen.status, None, "{unseen:?}");
}
