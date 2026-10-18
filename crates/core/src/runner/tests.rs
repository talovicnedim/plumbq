use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::*;
use crate::workspace::tests::{temp_workspace, write};

/// Serves until the test ends: `/fail` answers 500, everything else 200 echoing the path.
pub(super) async fn server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = vec![0; 4096];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();
            let status = match path.starts_with("/fail") {
                true => "500 Internal Server Error",
                false => "200 OK",
            };
            let body = format!("{{\"ok\":true,\"path\":\"{path}\"}}");
            let reply = format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(reply.as_bytes()).await;
        }
    });
    format!("http://{addr}")
}

pub(super) fn workspace(name: &str, base: &str) -> std::path::PathBuf {
    let root = temp_workspace(name);
    write(
        &root,
        "environments/local.toml",
        &format!("tag = \"local\"\n[variables]\nbase = \"{base}\"\n"),
    );
    write(
        &root,
        "environments/prod.toml",
        &format!("tag = \"production\"\n[variables]\nbase = \"{base}\"\n"),
    );
    write(
        &root,
        "folder.toml",
        "order = [\"users\", \"health.http\"]\n",
    );
    write(
        &root,
        "health.http",
        "### Health\n# @tag smoke\n# @assert body.$.ok == true\nGET {{base}}/health\n",
    );
    write(
        &root,
        "users/list.http",
        "### List\n# @assert status == 200\nGET {{base}}/users\n",
    );
    write(
        &root,
        "users/broken.http",
        "### Broken\nGET {{base}}/fail\n",
    );
    write(
        &root,
        "users/remove.http",
        "### Remove\nDELETE {{base}}/users/1\n",
    );
    root
}

pub(super) fn options(env: &str) -> RunOptions {
    RunOptions {
        environment: Some(env.into()),
        ..Default::default()
    }
}

fn names(report: &RunReport) -> Vec<(String, bool, bool)> {
    report
        .outcomes
        .iter()
        .map(|o| (o.name.clone(), o.failed(), o.skipped.is_some()))
        .collect()
}

pub(super) async fn run_all(root: &Path, target: &str, options: &RunOptions) -> RunReport {
    let selection = select(root, target, &options.tags).unwrap();
    run(root, &selection, options, |_| true).await
}

#[tokio::test]
async fn runs_in_display_order_and_judges_results() {
    let root = workspace("run-order", &server().await);
    let report = run_all(&root, "", &options("local")).await;
    assert_eq!(
        names(&report),
        [
            ("Broken".to_string(), true, false),
            ("List".to_string(), false, false),
            ("Remove".to_string(), false, false),
            ("Health".to_string(), false, false),
        ]
    );
}

#[tokio::test]
async fn bail_stops_and_callback_can_stop() {
    let root = workspace("run-bail", &server().await);
    let bail = RunOptions {
        bail: true,
        ..options("local")
    };
    assert_eq!(run_all(&root, "users", &bail).await.outcomes.len(), 1);

    let selection = select(&root, "", &[]).unwrap();
    let mut seen = 0;
    let report = run(&root, &selection, &options("local"), |_| {
        seen += 1;
        seen < 2
    })
    .await;
    assert_eq!(report.outcomes.len(), 2);
}

#[tokio::test]
async fn filters_by_tag_and_target() {
    let root = workspace("run-tags", &server().await);
    let smoke = RunOptions {
        tags: vec!["SMOKE".into()],
        ..options("local")
    };
    assert_eq!(
        names(&run_all(&root, "", &smoke).await),
        [("Health".to_string(), false, false)]
    );
    assert_eq!(
        run_all(&root, "users/list.http", &options("local"))
            .await
            .outcomes
            .len(),
        1
    );
}

#[tokio::test]
async fn production_writes_and_read_only_mode_skip() {
    let root = workspace("run-prod", &server().await);
    let report = run_all(&root, "users/remove.http", &options("prod")).await;
    assert!(report.outcomes[0].skipped.is_some());

    let allowed = RunOptions {
        allow_production_writes: true,
        ..options("prod")
    };
    assert_eq!(
        run_all(&root, "users/remove.http", &allowed).await.outcomes[0].status,
        Some(200)
    );

    let safe = RunOptions {
        safe_methods_only: true,
        ..options("local")
    };
    let report = run_all(&root, "users", &safe).await;
    assert_eq!(names(&report)[2], ("Remove".to_string(), false, true));
}

#[tokio::test]
async fn missing_prompts_are_reported_by_name() {
    let root = workspace("run-prompt", &server().await);
    write(
        &root,
        "otp.http",
        "### Otp\nGET {{base}}/otp?code={{$prompt code}}\n",
    );
    let report = run_all(&root, "otp.http", &options("local")).await;
    assert_eq!(report.outcomes[0].prompts_needed, ["code"]);

    let mut answered = options("local");
    answered.prompts.insert("code".into(), "123".into());
    assert_eq!(
        run_all(&root, "otp.http", &answered).await.outcomes[0].status,
        Some(200)
    );
}

#[test]
fn collects_prompts_of_the_whole_selection_once() {
    let root = temp_workspace("runner-prompts");
    write(
        &root,
        "a.http",
        "### A\nGET http://x.test/{{$prompt otp}}\n",
    );
    write(
        &root,
        "b/folder.toml",
        "[headers]\nX-Pin = \"{{$prompt pin}}\"\n",
    );
    write(
        &root,
        "b/c.http",
        "### C\nPOST http://x.test/{{$prompt otp}}\n",
    );
    write(&root, "d.http", "### D\nGET http://x.test/plain\n");

    let selection = select(&root, "", &[]).unwrap();

    assert_eq!(prompts_needed(&root, &selection).unwrap(), ["otp", "pin"]);
    let only_d = select(&root, "d.http", &[]).unwrap();
    assert!(prompts_needed(&root, &only_d).unwrap().is_empty());
}

#[tokio::test]
async fn read_only_runs_check_the_request_as_sent_not_as_selected() {
    let root = workspace("run-edited", &server().await);
    let selection = select(&root, "users/list.http", &[]).unwrap();
    // Edited into a write after it was selected.
    write(
        &root,
        "users/list.http",
        "### List\nDELETE {{base}}/users\n",
    );
    let safe = RunOptions {
        safe_methods_only: true,
        ..options("local")
    };

    let report = run(&root, &selection, &safe, |_| true).await;

    assert!(report.outcomes[0].skipped.is_some());
    assert_eq!(report.outcomes[0].method, "DELETE");
}

#[tokio::test]
async fn read_only_runs_skip_reads_that_override_their_method() {
    let root = workspace("run-override", &server().await);
    write(
        &root,
        "users/list.http",
        "### List\nGET {{base}}/users\nX-HTTP-Method-Override: DELETE\n",
    );
    let selection = select(&root, "users/list.http", &[]).unwrap();
    let safe = RunOptions {
        safe_methods_only: true,
        ..options("local")
    };

    let report = run(&root, &selection, &safe, |_| true).await;

    assert!(
        report.outcomes[0].skipped.is_some(),
        "{:?}",
        report.outcomes[0]
    );
}

#[test]
fn absurd_iteration_counts_are_refused() {
    assert!(check_iterations(MAX_ITERATIONS).is_ok());
    assert!(check_iterations(MAX_ITERATIONS + 1).is_err());
}
