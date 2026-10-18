use super::*;

#[test]
fn replaces_literal_auth_headers() {
    let mut found = Found::default();
    assert_eq!(
        placeholder("Authorization", "Bearer abc", &mut found).as_deref(),
        Some("Bearer {{token}}")
    );
    assert_eq!(
        placeholder("authorization", "Basic YWRhOnB3", &mut found).as_deref(),
        Some("Basic {{basicAuth}}")
    );
    assert_eq!(
        placeholder("X-API-Key", "k1", &mut found).as_deref(),
        Some("{{apiKey}}")
    );
    // The same value reuses its name; another value gets a new one.
    assert_eq!(
        placeholder("Authorization", "Bearer abc", &mut found).as_deref(),
        Some("Bearer {{token}}")
    );
    assert_eq!(
        placeholder("Authorization", "Bearer xyz", &mut found).as_deref(),
        Some("Bearer {{token2}}")
    );
    assert_eq!(found.names(), ["apiKey", "basicAuth", "token", "token2"]);
}

#[test]
fn leaves_placeholders_and_other_headers_alone() {
    let mut found = Found::default();
    assert_eq!(
        placeholder("Authorization", "Bearer {{token}}", &mut found),
        None
    );
    assert_eq!(placeholder("Accept", "application/json", &mut found), None);
    assert!(found.is_empty());
}

/// Imports `source` into a fresh workspace; returns the summary and every written file's text.
fn imported(source: &str) -> (crate::import::ImportSummary, String) {
    let root = crate::workspace::tests::temp_workspace("import-scrub");
    let summary = crate::import::write(&root, "", crate::import::parse(source).unwrap()).unwrap();
    let mut text = String::new();
    let mut dirs = vec![root];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            match path.is_dir() {
                true => dirs.push(path),
                false => text.push_str(&std::fs::read_to_string(path).unwrap()),
            }
        }
    }
    (summary, text)
}

fn postman(request: &str) -> String {
    format!(
        r#"{{"info": {{"name": "Shop", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"}},
            "item": [{{"name": "Call", "request": {request}}}]}}"#
    )
}

fn assert_scrubbed(source: &str, secret: &str, name: &str) {
    let (summary, text) = imported(source);
    assert!(!text.contains(secret), "{text}");
    assert!(text.contains(&format!("{{{{{name}}}}}")), "{text}");
    assert!(
        summary.notes.iter().any(|n| n.contains(name)),
        "{:?}",
        summary.notes
    );
}

#[test]
fn postman_api_keys_never_reach_the_files() {
    let in_header = postman(
        r#"{"method": "GET", "url": "https://shop.test/a",
            "auth": {"type": "apikey", "apikey": [{"key": "key", "value": "Ocp-Apim-Subscription-Key"},
                {"key": "value", "value": "LIVEKEY123"}]}}"#,
    );
    assert_scrubbed(&in_header, "LIVEKEY123", "ocpApimSubscriptionKey");

    let in_query = postman(
        r#"{"method": "GET", "url": "https://shop.test/a",
            "auth": {"type": "apikey", "apikey": [{"key": "key", "value": "api_key"},
                {"key": "value", "value": "LIVEKEY123"}, {"key": "in", "value": "query"}]}}"#,
    );
    assert_scrubbed(&in_query, "LIVEKEY123", "api_key");

    let odd_name = postman(
        r#"{"method": "GET", "url": "https://shop.test/a",
            "auth": {"type": "apikey", "apikey": [{"key": "key", "value": "X-Customer"},
                {"key": "value", "value": "LIVEKEY123"}]}}"#,
    );
    assert_scrubbed(&odd_name, "LIVEKEY123", "xCustomer");
}

#[test]
fn secret_looking_query_and_form_fields_become_secrets() {
    let form = postman(
        r#"{"method": "POST", "url": "https://shop.test/token?debug=1&access_token=q-123",
            "body": {"mode": "urlencoded", "urlencoded": [{"key": "grant_type", "value": "client_credentials"},
                {"key": "client_secret", "value": "s3cr3t"}]}}"#,
    );
    let (_, text) = imported(&form);
    assert!(
        !text.contains("s3cr3t") && !text.contains("q-123"),
        "{text}"
    );
    assert!(text.contains("grant_type=client_credentials&client_secret={{client_secret}}"));
    assert!(
        text.contains("?debug=1&access_token={{access_token}}"),
        "{text}"
    );
}

#[test]
fn har_csrf_headers_and_json_passwords_become_secrets() {
    let har = r#"{"log": {"entries": [{"request": {"method": "POST", "url": "https://shop.test/login",
        "headers": [{"name": "X-CSRF-Token", "value": "csrf-123"}, {"name": "Accept", "value": "*/*"}],
        "postData": {"mimeType": "application/json", "text": "{\"user\": \"ada\", \"password\": \"hunter2\"}"}},
        "response": {"content": {"mimeType": "application/json"}}}]}}"#;
    let (summary, text) = imported(har);
    assert!(
        !text.contains("csrf-123") && !text.contains("hunter2"),
        "{text}"
    );
    assert!(text.contains("X-CSRF-Token: {{xCsrfToken}}"), "{text}");
    assert!(
        text.contains("{\"user\": \"ada\", \"password\": \"{{password}}\"}"),
        "{text}"
    );
    assert!(summary.notes.iter().any(|n| n.contains("password")));
}
