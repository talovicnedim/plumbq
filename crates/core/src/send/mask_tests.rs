use base64::Engine;

use super::tests::secret_values;
use super::*;

#[test]
fn error_messages_never_show_secrets() {
    let values = secret_values("s3cr3t-key");
    let err = Error::Http("error sending request for url (http://h/x?k=s3cr3t-key)".into());
    assert_eq!(
        mask_error(err, &values).to_string(),
        "error sending request for url (http://h/x?k={{token}})"
    );
}

#[test]
fn responses_list_the_secrets_they_masked() {
    let values = secret_values("s3cr3t-key");
    let echoed = |body: &str| {
        let mut response = mask_response(
            Response {
                body: body.into(),
                ..text_response()
            },
            &values,
        );
        std::mem::take(&mut response.masked_secrets)
    };

    assert_eq!(echoed("key=s3cr3t-key"), ["token"]);
    assert!(echoed("key={{token}}").is_empty());
    // The server's own `{{token}}` can't be told apart from the mask, so neither is restored.
    assert!(echoed("key=s3cr3t-key or {{token}}").is_empty());
}

fn text_response() -> Response {
    Response {
        status: 200,
        status_text: String::new(),
        headers: Vec::new(),
        body: String::new(),
        size: 0,
        binary: false,
        body_base64: String::new(),
        truncated: false,
        duration_ms: 0,
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

#[test]
fn binary_bodies_are_masked_too() {
    let engine = base64::engine::general_purpose::STANDARD;
    let mut bytes = vec![0xff, 0x00];
    bytes.extend_from_slice(b"token=s3cr3t-key");
    let response = Response {
        binary: true,
        body_base64: engine.encode(&bytes),
        ..text_response()
    };
    let masked = mask_response(response, &secret_values("s3cr3t-key"));
    let decoded = engine.decode(masked.body_base64).unwrap();
    assert!(decoded.ends_with(b"token={{token}}"));
}
