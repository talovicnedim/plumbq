use std::collections::BTreeMap;
use std::fs;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::*;
use crate::args::Reporter;

fn report_root(root: &Path) -> String {
    plumbq_core::workspace::canonical_root(root)
        .unwrap()
        .display()
        .to_string()
}

/// Serves requests until the test ends: `/fail` answers 500, everything else 200 with JSON.
async fn server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = vec![0; 4096];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            let (status, body) = match request.contains(" /fail ") {
                true => ("500 Internal Server Error", "{}"),
                false => ("200 OK", r#"{"ok":true}"#),
            };
            let reply = format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(reply.as_bytes()).await;
        }
    });
    format!("http://{addr}")
}

fn workspace(name: &str, base: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let root = std::env::temp_dir().join(format!("ac-cli-{name}-{nanos:x}"));
    let files = [
        (
            "environments/local.toml",
            format!("tag = \"local\"\n[variables]\nbase = \"{base}\"\n"),
        ),
        (
            "environments/prod.toml",
            format!("tag = \"production\"\n[variables]\nbase = \"{base}\"\n"),
        ),
        (
            "folder.toml",
            "order = [\"users\", \"health.http\"]\n".to_string(),
        ),
        (
            "health.http",
            "### Health\n# @assert body.$.ok == true\nGET {{base}}/health\n".into(),
        ),
        (
            "users/list.http",
            "### List\n# @assert status == 200\nGET {{base}}/users\n".into(),
        ),
        (
            "users/broken.http",
            "### Broken\nGET {{base}}/fail\n".into(),
        ),
        (
            "users/remove.http",
            "### Remove\nDELETE {{base}}/users/1\n".into(),
        ),
    ];
    for (rel, text) in files {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    root
}

fn args(target: PathBuf, env: &str) -> RunArgs {
    RunArgs {
        target,
        environment: Some(env.into()),
        bail: false,
        reporter: Reporter::Pretty,
        output: None,
        prompts: BTreeMap::new(),
        vars: BTreeMap::new(),
        root: None,
        allow_production_writes: false,
        history: false,
        cookies: true,
        tags: Vec::new(),
        data: None,
        iterations: 1,
        delay_ms: 0,
    }
}

fn summary(report: &RunReport) -> Vec<(String, bool, bool)> {
    report
        .outcomes
        .iter()
        .map(|o| (o.name.clone(), o.failed(), o.skipped.is_some()))
        .collect()
}

#[tokio::test]
async fn runs_workspace_in_display_order_and_judges_results() {
    let root = workspace("all", &server().await);
    let report = run(&args(root.clone(), "local")).await.unwrap();

    assert_eq!(
        summary(&report),
        [
            ("Broken".to_string(), true, false),
            ("List".to_string(), false, false),
            ("Remove".to_string(), false, false),
            ("Health".to_string(), false, false),
        ]
    );
    assert_eq!(report.root, report_root(&root));
}

#[tokio::test]
async fn production_writes_are_skipped_unless_allowed() {
    let root = workspace("prod", &server().await);
    let report = run(&args(root.join("users/remove.http"), "prod"))
        .await
        .unwrap();
    assert_eq!(summary(&report), [("Remove".to_string(), false, true)]);
    let reason = report.outcomes[0].skipped.clone().unwrap_or_default();
    assert!(reason.contains("--allow-production-writes"), "{reason}");

    let mut allowed = args(root.join("users/remove.http"), "prod");
    allowed.allow_production_writes = true;
    let report = run(&allowed).await.unwrap();
    assert_eq!(report.outcomes[0].status, Some(200));
}

#[tokio::test]
async fn subfolder_target_finds_the_root_and_bail_stops() {
    let root = workspace("bail", &server().await);
    let mut bail = args(root.join("users"), "local");
    bail.bail = true;
    let report = run(&bail).await.unwrap();

    assert_eq!(report.root, report_root(&root));
    assert_eq!(summary(&report), [("Broken".to_string(), true, false)]);
}

#[tokio::test]
async fn missing_prompt_values_fail_with_a_hint() {
    let root = workspace("prompt", &server().await);
    fs::write(
        root.join("otp.http"),
        "### Otp\nGET {{base}}/otp?code={{$prompt code}}\n",
    )
    .unwrap();

    let report = run(&args(root.join("otp.http"), "local")).await.unwrap();
    let error = report.outcomes[0].error.clone().unwrap_or_default();
    assert!(error.contains("--prompt code="), "{error}");

    let mut answered = args(root.join("otp.http"), "local");
    answered.prompts.insert("code".into(), "123".into());
    assert_eq!(run(&answered).await.unwrap().outcomes[0].status, Some(200));
}

fn temp(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = std::env::temp_dir().join(format!("ac-cli-{name}-{nanos:x}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn root_is_the_highest_workspace_folder_even_across_gaps() {
    let root = temp("root");
    fs::create_dir_all(root.join("a/b")).unwrap();
    fs::write(root.join("folder.toml"), "").unwrap();
    // `a` has no folder.toml (hand-made or cloned folders), which mustn't stop the climb.
    fs::write(root.join("a/b/x.http"), "GET /x\n").unwrap();
    assert_eq!(find_root(&root.join("a/b/x.http")), root);
}

#[test]
fn root_stays_inside_the_repository_and_ignores_other_tools_environments() {
    let repo = temp("repo");
    fs::create_dir_all(repo.join(".git")).unwrap();
    // Terraform-style environments/ at the repo root: no .toml files, so not ours.
    fs::create_dir_all(repo.join("environments/prod")).unwrap();
    fs::write(repo.join("environments/prod/main.tf"), "").unwrap();
    fs::create_dir_all(repo.join("api/users")).unwrap();
    fs::write(repo.join("api/folder.toml"), "").unwrap();
    assert_eq!(find_root(&repo.join("api/users")), repo.join("api"));
}

#[tokio::test]
async fn unknown_environment_is_a_usage_error() {
    let root = workspace("unknown-env", "http://127.0.0.1:9");
    let err = run(&args(root.clone(), "stagin")).await.unwrap_err();
    assert!(err.contains("unknown environment stagin"), "{err}");
    assert!(err.contains("local, prod"), "{err}");
}

#[tokio::test]
async fn an_environment_in_the_wrong_case_gets_a_hint() {
    let root = workspace("env-case", "http://127.0.0.1:9");
    let err = run(&args(root.clone(), "Local")).await.unwrap_err();
    assert!(
        err.contains("unknown environment Local (did you mean local?)"),
        "{err}"
    );

    let err = run(&args(root, "$x")).await.unwrap_err();
    assert!(err.contains("starts with $"), "{err}");
}

#[tokio::test]
async fn vars_fill_variables_and_win_over_data_rows() {
    let root = workspace("vars", &server().await);
    fs::write(root.join("rows.csv"), "user\n1\n").unwrap();
    fs::write(
        root.join("user.http"),
        "### User\nGET {{base}}/{{user}}\nX-Version: {{version}}\n",
    )
    .unwrap();
    let mut run_args = args(root.join("user.http"), "local");
    run_args.data = Some(root.join("rows.csv"));
    run_args.vars.insert("version".into(), "2".into());
    // The row says 1; --var wins, and the test server fails `/fail`.
    run_args.vars.insert("user".into(), "fail".into());
    run_args.root = Some(root.clone());

    let report = run(&run_args).await.unwrap();
    let outcome = &report.outcomes[0];
    assert_eq!(outcome.status, Some(500), "{outcome:?}");
}

#[tokio::test]
async fn data_file_runs_once_per_row() {
    let root = workspace("data", &server().await);
    fs::write(root.join("rows.csv"), "user\n1\n2\n3\n").unwrap();
    fs::write(
        root.join("user.http"),
        "### User\n# @assert status == 200\nGET {{base}}/users/{{user}}\n",
    )
    .unwrap();
    let mut data = args(root.join("user.http"), "local");
    data.data = Some(root.join("rows.csv"));

    let report = run(&data).await.unwrap();
    let iterations: Vec<u32> = report.outcomes.iter().map(|o| o.iteration).collect();
    assert_eq!(iterations, [1, 2, 3]);
}

#[test]
fn secret_looking_vars_are_masked_like_secrets() {
    let mut run_args = args(PathBuf::from("."), "local");
    run_args.vars = BTreeMap::from([
        ("API_TOKEN".into(), "t0ken-value".into()),
        ("userId".into(), "7".into()),
    ]);

    let context = run_options(&run_args, Vec::new(), None).context;

    assert_eq!(context.secrets["API_TOKEN"], "t0ken-value");
    assert_eq!(context.variables["userId"], "7");
    assert!(!context.variables.contains_key("API_TOKEN"));
}

#[cfg(unix)]
#[test]
fn scratch_folders_are_new_private_and_unpredictable() {
    use std::os::unix::fs::PermissionsExt;
    let (first, second) = (scratch_dir().unwrap(), scratch_dir().unwrap());

    assert_ne!(first, second);
    let mode = fs::metadata(&first).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700);
    assert!(
        create_private_dir(&first).is_err(),
        "an existing folder is never reused"
    );
    for dir in [first, second] {
        fs::remove_dir_all(dir).unwrap();
    }
}
