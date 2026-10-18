use super::*;

fn values() -> Values {
    Values {
        variables: BTreeMap::from([("baseUrl".into(), "https://api.test".into())]),
        secrets: BTreeMap::from([("token".into(), "s3cr3t".into())]),
        ..Default::default()
    }
}

#[test]
fn substitutes_variables_and_secrets() {
    let text = substitute("{{baseUrl}}/me?t={{ token }}", &values()).unwrap();
    assert_eq!(text, "https://api.test/me?t=s3cr3t");
}

#[test]
fn reports_all_missing_variables_once() {
    let err = substitute("{{a}} {{b}} {{a}}", &values()).unwrap_err();
    assert_eq!(err.to_string(), "unknown variables: a, b");
}

#[test]
fn leaves_unclosed_braces_alone() {
    assert_eq!(substitute("{\"a\": {{", &values()).unwrap(), "{\"a\": {{");
}

#[test]
fn dynamic_variables_resolve() {
    let text = substitute("{{$uuid}}", &values()).unwrap();
    assert_eq!(text.len(), 36);
    let n: u32 = substitute("{{$randomInt}}", &values())
        .unwrap()
        .parse()
        .unwrap();
    assert!(n < 1000);
}

#[test]
fn masks_secret_values() {
    let secrets = values().secrets;
    assert_eq!(mask("Bearer s3cr3t", &secrets), "Bearer {{token}}");
}

#[test]
fn masks_encoded_forms_of_secrets() {
    let secrets = BTreeMap::from([("token".to_string(), "ab/cd e+f".to_string())]);
    let laravel_json = r#"{"token":"ab\/cd e+f"}"#;
    assert_eq!(mask(laravel_json, &secrets), r#"{"token":"{{token}}"}"#);
    assert_eq!(mask("?t=ab%2Fcd%20e%2Bf", &secrets), "?t={{token}}");
    assert_eq!(mask("?t=ab%2fcd%20e%2bf", &secrets), "?t={{token}}");
    assert_eq!(mask("?t=ab%2Fcd+e%2Bf", &secrets), "?t={{token}}");
    let b64 = base64::engine::general_purpose::STANDARD.encode("ab/cd e+f");
    assert_eq!(mask(&format!("x {b64} y"), &secrets), "x {{token}} y");
}

#[test]
fn masks_basic_auth_credentials_containing_a_secret() {
    let secrets = BTreeMap::from([("pw".to_string(), "hunter22".to_string())]);
    let basic = base64::engine::general_purpose::STANDARD.encode("ada:hunter22");
    assert_eq!(
        mask(&format!("Authorization: Basic {basic}"), &secrets),
        "Authorization: Basic {{pw}}"
    );
}

#[test]
fn short_secrets_are_not_masked_and_placeholders_survive() {
    let secrets = BTreeMap::from([
        ("otp".to_string(), "1".to_string()),
        ("long".to_string(), "secret-value".to_string()),
        ("inner".to_string(), "long".to_string()),
    ]);
    // A one-character secret would break JSON; the "long" name inside a placeholder
    // must not be replaced by the "inner" secret.
    assert_eq!(
        mask(r#"{"id": 1, "k": "secret-value"}"#, &secrets),
        r#"{"id": 1, "k": "{{long}}"}"#
    );
    assert_eq!(short_secrets(&secrets), ["otp"]);
}

#[test]
fn masks_bytes_of_binary_bodies() {
    let secrets = values().secrets;
    let mut bytes = vec![0u8, 159, 146];
    bytes.extend_from_slice(b"s3cr3t");
    bytes.push(255);
    let masked = mask_bytes(&bytes, &secrets);
    assert!(!masked.windows(6).any(|w| w == b"s3cr3t"));
    assert!(masked.windows(9).any(|w| w == b"{{token}}"));
}

#[test]
fn lists_used_variables() {
    assert_eq!(used("{{a}}/{{ b }}/{{a}}"), ["a", "b"]);
}

#[test]
fn a_reference_still_being_typed_never_swallows_the_text_after_it() {
    let text = "{{base}}/lists/{{xy\n{ \"list\": \"News\" }\nstatus == 200\n{{apiKey}}";
    assert_eq!(used(text), ["base", "apiKey"]);
}

#[test]
fn substitution_keeps_a_reference_still_being_typed_as_text() {
    let text = substitute("{{baseUrl}}/{{xy\n{{token}}", &values()).unwrap();
    assert_eq!(text, "https://api.test/{{xy\ns3cr3t");
}

#[test]
fn a_third_brace_stays_text() {
    assert_eq!(used("{\"a\":{{{token}}}}"), ["token"]);
    let text = substitute("{\"a\":{{{token}}}}", &values()).unwrap();
    assert_eq!(text, "{\"a\":{s3cr3t}}");
}
