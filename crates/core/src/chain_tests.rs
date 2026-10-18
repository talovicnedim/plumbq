use super::*;
use crate::send::{Response, SentRequest};
use crate::workspace::tests::{temp_workspace, write};

fn response(body: &str) -> Response {
    Response {
        status: 200,
        status_text: "OK".into(),
        headers: vec![crate::http_file::Header {
            name: "Location".into(),
            value: "/users/7".into(),
        }],
        body: body.into(),
        size: 0,
        binary: false,
        body_base64: String::new(),
        truncated: false,
        duration_ms: 1,
        sent: SentRequest {
            method: "POST".into(),
            url: "http://x.test/login".into(),
            headers: Vec::new(),
            body: String::new(),
        },
        assertions: Vec::new(),
        masked_secrets: Vec::new(),
    }
}

fn workspace_with_login() -> (std::path::PathBuf, std::path::PathBuf) {
    let root = temp_workspace("chain");
    let history = temp_workspace("chain-history");
    write(&root, "auth/login.http", "### Log in\nPOST /login\n");
    let body = r#"{"token":"abc","user":{"id":7,"roles":["admin"]}}"#;
    history::add(
        &history,
        &root,
        "auth/login.http",
        Some("local"),
        response(body),
    )
    .unwrap();
    (root, history)
}

#[test]
fn parses_references() {
    assert_eq!(
        parse("login.response.body.$.token"),
        Some(Reference {
            request: "login",
            part: Part::Body("$.token")
        })
    );
    assert_eq!(
        parse("login.response.headers.Location"),
        Some(Reference {
            request: "login",
            part: Part::Header("Location")
        })
    );
    assert_eq!(parse("baseUrl"), None);
    assert_eq!(parse("$prompt x.response.body.$"), None);
}

#[test]
fn resolves_body_paths_headers_and_masks_tokens() {
    let (root, history) = workspace_with_login();
    let text = "{{login.response.body.$.token}} {{log-in.response.body.$.user.id}} \
                {{auth/login.http.response.headers.location}} {{login.response.body.$.user.roles}}";
    let mut values = Values::default();

    resolve(text, &root, Some(&history), Some("local"), &mut values).unwrap();

    assert_eq!(values.secrets["login.response.body.$.token"], "abc");
    assert_eq!(values.variables["log-in.response.body.$.user.id"], "7");
    assert_eq!(
        values.variables["auth/login.http.response.headers.location"],
        "/users/7"
    );
    assert_eq!(
        values.variables["login.response.body.$.user.roles"],
        r#"["admin"]"#
    );
}

#[test]
fn needs_a_response_in_the_same_environment() {
    let (root, history) = workspace_with_login();
    let mut values = Values::default();
    let err = resolve(
        "{{login.response.body.$.token}}",
        &root,
        Some(&history),
        Some("prod"),
        &mut values,
    )
    .unwrap_err();
    assert!(err.to_string().contains("send that request first"));
}

#[test]
fn reports_unknown_requests_and_paths() {
    let (root, history) = workspace_with_login();
    let mut values = Values::default();
    let unknown = resolve(
        "{{nope.response.body.$}}",
        &root,
        Some(&history),
        Some("local"),
        &mut values,
    );
    assert!(
        unknown
            .unwrap_err()
            .to_string()
            .contains("no request named nope")
    );
    let missing = resolve(
        "{{login.response.body.$.missing}}",
        &root,
        Some(&history),
        Some("local"),
        &mut values,
    );
    assert!(
        missing
            .unwrap_err()
            .to_string()
            .contains("not in the last response")
    );
}

#[test]
fn masked_values_in_history_are_restored_or_refused() {
    let mut values = Values::default();
    values.secrets.insert("token".into(), "real-token".into());
    let masked = ["token".to_string()];

    let (restored, secret) = restore_masked("{{token}}".into(), &masked, &values).unwrap();
    assert_eq!(restored, "real-token");
    assert!(secret);
    let (text, secret) = restore_masked("Bearer {{token}} {{x}}".into(), &masked, &values).unwrap();
    assert_eq!(text, "Bearer real-token {{x}}");
    assert!(secret);
    assert_eq!(
        restore_masked("{{other}}".into(), &masked, &values),
        Err("{{other}}".to_string())
    );
}

fn history_with(body: &str, masked: &[&str]) -> (std::path::PathBuf, std::path::PathBuf) {
    let root = temp_workspace("chain-masked");
    let history = temp_workspace("chain-masked-history");
    write(&root, "list.http", "### List\nGET /items\n");
    let mut response = response(body);
    response.masked_secrets = masked.iter().map(|name| name.to_string()).collect();
    history::add(&history, &root, "list.http", None, response).unwrap();
    (root, history)
}

#[test]
fn placeholders_a_server_wrote_are_never_filled_with_secrets() {
    let (root, history) = history_with(r#"{"next":"https://evil.test/?k={{apiKey}}"}"#, &[]);
    let mut values = Values::default();
    values
        .secrets
        .insert("apiKey".into(), "real-api-key".into());

    resolve(
        "{{list.response.body.$.next}}",
        &root,
        Some(&history),
        None,
        &mut values,
    )
    .unwrap();

    assert_eq!(
        values.variables["list.response.body.$.next"],
        "https://evil.test/?k={{apiKey}}"
    );
}

#[test]
fn secrets_history_masked_are_chained_back() {
    let (root, history) = history_with(r#"{"key":"{{apiKey}}"}"#, &["apiKey"]);
    let mut values = Values::default();
    values
        .secrets
        .insert("apiKey".into(), "real-api-key".into());

    resolve(
        "{{list.response.body.$.key}}",
        &root,
        Some(&history),
        None,
        &mut values,
    )
    .unwrap();

    assert_eq!(values.secrets["list.response.body.$.key"], "real-api-key");
}

#[test]
fn text_without_references_needs_no_history() {
    let mut values = Values::default();
    resolve(
        "{{baseUrl}}/x",
        Path::new("/nowhere"),
        None,
        None,
        &mut values,
    )
    .unwrap();
    assert!(values.variables.is_empty());
}

#[test]
fn rest_client_reference_names_win() {
    let root = temp_workspace("chain-ref-name");
    let history = temp_workspace("chain-ref-name-history");
    // `login` is the display-name match of another request; `# @name login` is explicit.
    write(&root, "login.http", "### Login\nGET /other\n");
    write(
        &root,
        "auth/session.http",
        "###\n# @name login\nPOST /session\n",
    );
    history::add(
        &history,
        &root,
        "auth/session.http",
        None,
        response(r#"{"id":"s1"}"#),
    )
    .unwrap();
    history::add(
        &history,
        &root,
        "login.http",
        None,
        response(r#"{"id":"other"}"#),
    )
    .unwrap();
    let mut values = Values::default();

    resolve(
        "{{login.response.body.$.id}}",
        &root,
        Some(&history),
        None,
        &mut values,
    )
    .unwrap();

    assert_eq!(values.variables["login.response.body.$.id"], "s1");
}
