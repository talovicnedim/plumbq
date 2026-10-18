use plumbq_core::send::SentRequest;

use super::*;

#[test]
fn response_caps_long_bodies() {
    let response = Response {
        status: 200,
        status_text: "OK".into(),
        headers: Vec::new(),
        body: "x".repeat(MAX_BODY_CHARS + 10),
        size: 0,
        binary: false,
        body_base64: String::new(),
        truncated: false,
        duration_ms: 5,
        sent: SentRequest {
            method: "GET".into(),
            url: "http://x.test".into(),
            headers: Vec::new(),
            body: String::new(),
        },
        assertions: Vec::new(),
        masked_secrets: Vec::new(),
    };
    let text = super::response(&response);
    assert!(text.contains("cut at 8000 characters"));
    assert!(text.len() < MAX_BODY_CHARS + 500);
}

#[test]
fn set_cookie_values_are_not_shown() {
    let response = Response {
        status: 200,
        status_text: "OK".into(),
        headers: vec![plumbq_core::http_file::Header {
            name: "set-cookie".into(),
            value: "sid=very-secret; Path=/; HttpOnly".into(),
        }],
        body: String::new(),
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
    };
    let text = super::response(&response);
    assert!(text.contains("set-cookie: sid=… (stored in the cookie jar)"));
    assert!(!text.contains("very-secret"));
}
