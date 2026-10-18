use super::*;
use crate::http_file::Header;
use crate::send::SentRequest;

fn response(status: u16, body: &str) -> Response {
    Response {
        status,
        status_text: String::new(),
        headers: vec![Header {
            name: "Content-Type".into(),
            value: "application/json; charset=utf-8".into(),
        }],
        body: body.into(),
        size: u32::try_from(body.len()).unwrap_or(u32::MAX),
        binary: false,
        body_base64: String::new(),
        truncated: false,
        duration_ms: 120,
        sent: SentRequest {
            method: "GET".into(),
            url: "http://x.test".into(),
            headers: Vec::new(),
            body: String::new(),
        },
        assertions: Vec::new(),
        masked_secrets: Vec::new(),
    }
}

fn check(expression: &str, response: &Response) -> AssertionResult {
    run(&[expression.to_string()], response, &Values::default()).remove(0)
}

const BODY: &str =
    r#"{"user":{"id":1,"email":"ada@example.com"},"items":[{"name":"Ada"}],"ok":true,"token":"t"}"#;

#[test]
fn status_duration_and_size() {
    let r = response(201, BODY);
    assert!(check("status == 201", &r).passed);
    assert!(check("status in 200..299", &r).passed);
    assert!(check("status < 400", &r).passed);
    assert!(check("duration < 500", &r).passed);
    assert!(check("size > 10", &r).passed);
    let failed = check("status == 200", &r);
    assert!(!failed.passed);
    assert_eq!(failed.message, "expected status == 200, got 201");
}

#[test]
fn headers() {
    let r = response(200, BODY);
    assert!(check("header content-type contains json", &r).passed);
    assert!(check("header Content-Type exists", &r).passed);
    let missing = check("header X-Id exists", &r);
    assert!(!missing.passed);
    assert_eq!(missing.message, "header X-Id is missing");
}

#[test]
fn json_body_paths() {
    let r = response(200, BODY);
    assert!(check("body.$.user.id == 1", &r).passed);
    assert!(check("body.$.items.length > 0", &r).passed);
    assert!(check(r#"body.$.items[0].name == "Ada""#, &r).passed);
    assert!(check("body.$.ok == true", &r).passed);
    assert!(check("body.$.token exists", &r).passed);
    assert!(check("body.$.user.email endsWith @example.com", &r).passed);
    assert!(!check("body.$.user.id == 2", &r).passed);
    assert!(!check("body.$.missing exists", &r).passed);
}

#[test]
fn plain_body() {
    let r = response(200, "created ok");
    assert!(check(r#"body contains "created""#, &r).passed);
    assert!(check("body startsWith created", &r).passed);
    let not_json = check("body.$.id == 1", &r);
    assert!(!not_json.passed);
    assert!(not_json.message.contains("isn't JSON"));
}

#[test]
fn bad_expressions_fail_with_a_reason() {
    let r = response(200, BODY);
    for expression in [
        "",
        "speed < 3",
        "status ~ 200",
        "status ==",
        "status in 2",
        "body.$.a matches x",
    ] {
        let result = check(expression, &r);
        assert!(!result.passed, "{expression} passed");
        assert!(!result.message.is_empty(), "{expression} has no message");
    }
}

#[test]
fn string_and_number_compare_as_text() {
    let mut r = response(200, BODY);
    r.headers.push(Header {
        name: "X-Count".into(),
        value: "3".into(),
    });
    assert!(check("header X-Count == 3", &r).passed);
    assert!(check("header X-Count >= 2", &r).passed);
}

#[test]
fn expressions_use_variables() {
    let values = Values {
        variables: std::collections::BTreeMap::from([
            ("userId".into(), "1".into()),
            ("name".into(), "Ada".into()),
        ]),
        ..Default::default()
    };
    let results = run(
        &[
            "body.$.user.id == {{userId}}".into(),
            "body.$.items[0].name == \"{{name}}\"".into(),
            "body.$.user.id == {{missing}}".into(),
        ],
        &response(200, BODY),
        &values,
    );
    assert!(results[0].passed, "{}", results[0].message);
    assert!(results[1].passed, "{}", results[1].message);
    assert_eq!(results[0].expression, "body.$.user.id == {{userId}}");
    assert!(!results[2].passed);
    assert_eq!(results[2].message, "unknown variables: missing");
}

#[test]
fn secrets_compare_as_placeholders_and_never_show() {
    // The response was masked by `send`: the token's value already reads `{{token}}`.
    let masked = response(200, r#"{"echo":"Bearer {{token}}"}"#);
    let values = Values {
        secrets: std::collections::BTreeMap::from([("token".into(), "s3cr3t".into())]),
        ..Default::default()
    };
    let results = run(
        &[
            "body.$.echo == \"Bearer {{token}}\"".into(),
            "body.$.echo == \"other {{token}}\"".into(),
        ],
        &masked,
        &values,
    );
    assert!(results[0].passed, "{}", results[0].message);
    assert!(!results[1].passed);
    assert!(results.iter().all(|r| !r.message.contains("s3cr3t")));
}
