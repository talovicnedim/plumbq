use super::*;

#[test]
fn pkce_challenge_matches_rfc_7636() {
    assert_eq!(
        challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
}

#[test]
fn listens_only_on_loopback() {
    assert_eq!(
        loopback_addr("http://127.0.0.1:53682/callback").unwrap(),
        "127.0.0.1:53682"
    );
    assert_eq!(
        loopback_addr("http://localhost:8080/cb").unwrap(),
        "127.0.0.1:8080"
    );
    assert!(loopback_addr("http://example.com:80/cb").is_err());
    assert!(loopback_addr("https://127.0.0.1:1/cb").is_err());
    assert!(loopback_addr("http://127.0.0.1/cb").is_err());
}

#[test]
fn reads_the_code_and_checks_state() {
    let request = "GET /callback?code=abc&state=s1 HTTP/1.1\r\nHost: x\r\n\r\n";
    assert_eq!(parse_callback(request, "s1").unwrap(), Some("abc".into()));
    assert_eq!(
        parse_callback("GET /favicon.ico HTTP/1.1", "s1").unwrap(),
        None
    );
    let denied =
        "GET /callback?error=access_denied&error_description=No%20thanks&state=s1 HTTP/1.1";
    assert_eq!(
        parse_callback(denied, "s1").unwrap_err().to_string(),
        "sign-in was refused: No thanks"
    );
}

#[test]
fn requests_without_our_state_are_ignored() {
    // A wrong state, even with a code or an error, must not end the sign-in.
    let wrong = "GET /callback?code=abc&state=other HTTP/1.1\r\n\r\n";
    assert_eq!(parse_callback(wrong, "s1").unwrap(), None);
    let forged = "GET /callback?error=x&error_description=%3Cscript%3E HTTP/1.1";
    assert_eq!(parse_callback(forged, "s1").unwrap(), None);
}

#[test]
fn escapes_html_in_the_failure_page() {
    assert_eq!(escape_html("<script>&\""), "&lt;script&gt;&amp;&quot;");
}

#[tokio::test]
async fn a_silent_connection_does_not_block_the_callback() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let waiting = tokio::spawn(async move { wait_for_code(&listener, "s1").await });
    // A preconnect that never sends anything, then the real callback in two pieces.
    let _idle = tokio::net::TcpStream::connect(addr).await.unwrap();
    let mut real = tokio::net::TcpStream::connect(addr).await.unwrap();
    real.write_all(b"GET /callback?code=c1&state=s1 HT")
        .await
        .unwrap();
    real.write_all(b"TP/1.1\r\nHost: x\r\n\r\n").await.unwrap();

    let code = tokio::time::timeout(Duration::from_secs(15), waiting)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(code, "c1");
}

#[test]
fn random_strings_are_long_and_url_safe() {
    let text = random_string();
    assert_eq!(text.len(), 64);
    assert!(text.chars().all(|c| c.is_ascii_alphanumeric()));
}

#[tokio::test]
async fn only_web_pages_are_opened_for_sign_in() {
    let root = crate::workspace::tests::temp_workspace("oauth-scheme");
    crate::workspace::tests::write(
        &root,
        "environments/dev.toml",
        "[auth.evil]\ngrant = \"authorization_code\"\nauth_url = \"file:///etc/passwd\"\n\
         token_url = \"http://x.test/token\"\nclient_id = \"a\"\n",
    );
    let mut opened = false;

    let err = sign_in(&root, "dev", "evil", |_| {
        opened = true;
        Err(Error::Invalid("not opened in tests".into()))
    })
    .await
    .unwrap_err();

    assert!(!opened);
    assert!(
        err.to_string().contains("must start with https://"),
        "{err}"
    );
}
