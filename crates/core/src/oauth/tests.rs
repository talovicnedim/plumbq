use std::collections::BTreeMap;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::token::parse_token_response;
use super::*;
use crate::Error;

fn client(token_url: &str) -> OAuthClient {
    OAuthClient {
        name: "api".into(),
        grant: OAuthGrant::ClientCredentials,
        auth_url: String::new(),
        token_url: token_url.into(),
        client_id: "my-app".into(),
        client_secret: "{{secret}}".into(),
        scope: "read write".into(),
        redirect_url: String::new(),
        ..OAuthClient::new("api")
    }
}

#[test]
fn reads_client_names() {
    assert_eq!(client_name("$auth.token(\"github\")"), Some("github"));
    assert_eq!(client_name("$auth.token github"), Some("github"));
    assert_eq!(client_name("$auth.token"), None);
    assert_eq!(client_name("token"), None);
}

#[test]
fn refresh_keeps_the_old_refresh_token_when_none_is_returned() {
    let fresh = Token {
        access_token: "new".into(),
        refresh_token: None,
        expires_at: Some(10),
    };
    assert_eq!(
        keep_refresh_token(fresh.clone(), "r1")
            .refresh_token
            .as_deref(),
        Some("r1")
    );
    let rotated = Token {
        refresh_token: Some("r2".into()),
        ..fresh
    };
    assert_eq!(
        keep_refresh_token(rotated, "r1").refresh_token.as_deref(),
        Some("r2")
    );
}

#[test]
fn huge_expiry_values_do_not_overflow() {
    let token = parse_token_response(
        "api",
        200,
        r#"{"access_token":"a","expires_in":9223372036854775807}"#,
        100,
    )
    .unwrap()
    .into_token()
    .unwrap();
    assert_eq!(token.expires_at, Some(i64::MAX));
    assert!(!is_expired(&token, 0));
    let ancient = Token {
        access_token: "a".into(),
        refresh_token: None,
        expires_at: Some(i64::MIN),
    };
    assert!(is_expired(&ancient, 0));
}

#[test]
fn tokens_expire_with_a_margin() {
    let token = Token {
        access_token: "t".into(),
        refresh_token: None,
        expires_at: Some(1000),
    };
    assert!(!is_expired(&token, 900));
    assert!(is_expired(&token, 980));
    let forever = Token {
        expires_at: None,
        ..token
    };
    assert!(!is_expired(&forever, i64::MAX));
}

#[test]
fn parses_token_responses_and_provider_errors() {
    let ok =
        parse_token_response("api", 200, r#"{"access_token":"a","expires_in":60}"#, 100).unwrap();
    assert_eq!(
        ok,
        TokenAnswer::Granted(Token {
            access_token: "a".into(),
            refresh_token: None,
            expires_at: Some(160)
        })
    );

    let refused = parse_token_response(
        "api",
        401,
        r#"{"error":"invalid_client","error_description":"bad secret"}"#,
        0,
    )
    .unwrap();
    assert_eq!(
        refused.into_token().unwrap_err().to_string(),
        "token request for api failed (401): bad secret"
    );
    assert!(parse_token_response("api", 200, "<html>", 0).is_err());
    // A provider that is down hasn't refused anything.
    assert!(parse_token_response("api", 503, r#"{"error":"temporarily_unavailable"}"#, 0).is_err());
}

#[tokio::test]
async fn client_credentials_post_resolved_secret_and_scope() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/token", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0; 4096];
        let n = socket.read(&mut buf).await.unwrap();
        let body = r#"{"access_token":"tok","expires_in":3600}"#;
        let reply = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(reply.as_bytes()).await.unwrap();
        String::from_utf8_lossy(&buf[..n]).to_string()
    });
    let values = Values {
        secrets: BTreeMap::from([("secret".into(), "s3cr3t".into())]),
        ..Default::default()
    };

    let token = request_token(
        &crate::workspace::tests::temp_workspace("oauth-token"),
        &client(&url),
        &values,
        &[("grant_type", "client_credentials")],
    )
    .await
    .unwrap()
    .into_token()
    .unwrap();

    assert_eq!(token.access_token, "tok");
    let request = server.await.unwrap();
    assert!(request.contains(
        "grant_type=client_credentials&client_id=my-app&client_secret=s3cr3t&scope=read%20write"
    ));
}

#[test]
fn sign_in_is_needed_without_a_token_or_with_an_expired_one() {
    let token = |expires_at| Token {
        access_token: "a".into(),
        refresh_token: Some("r".into()),
        expires_at,
    };
    assert!(needs_sign_in(None, 1_000));
    assert!(needs_sign_in(Some(&token(Some(1_000))), 2_000));
    assert!(!needs_sign_in(Some(&token(Some(10_000))), 2_000));
    assert!(!needs_sign_in(Some(&token(None)), 2_000));
}

#[test]
fn new_clients_show_the_default_redirect() {
    let client = OAuthClient::new("github");
    assert_eq!(client.grant, OAuthGrant::AuthorizationCode);
    assert!(client.redirect_url.is_empty());
    assert_eq!(client.default_redirect_url, DEFAULT_REDIRECT);
}

/// Answers one token request: a token for `client_credentials`, `invalid_grant` for anything
/// else. Resolves with the request it got.
async fn token_server() -> (String, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/token", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0; 4096];
        let n = socket.read(&mut buf).await.unwrap();
        let request = String::from_utf8_lossy(&buf[..n]).to_string();
        let (status, body) = match request.contains("grant_type=client_credentials") {
            true => ("200 OK", r#"{"access_token":"fresh","expires_in":3600}"#),
            false => ("400 Bad Request", r#"{"error":"invalid_grant"}"#),
        };
        let reply = format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(reply.as_bytes()).await.unwrap();
        request
    });
    (url, server)
}

fn expired_with_refresh() -> Option<Token> {
    Some(Token {
        access_token: "old".into(),
        refresh_token: Some("stale-refresh".into()),
        expires_at: Some(0),
    })
}

fn secret_values() -> Values {
    Values {
        secrets: BTreeMap::from([("secret".into(), "s3cr3t".into())]),
        ..Default::default()
    }
}

#[tokio::test]
async fn client_credentials_fetch_a_new_token_instead_of_a_stale_refresh() {
    let (url, server) = token_server().await;
    let root = crate::workspace::tests::temp_workspace("oauth-cc-renew");

    let token = renew(
        &root,
        &client(&url),
        &secret_values(),
        expired_with_refresh(),
    )
    .await
    .unwrap();

    assert_eq!(token.access_token, "fresh");
    assert!(
        server
            .await
            .unwrap()
            .contains("grant_type=client_credentials")
    );
}

#[tokio::test]
async fn a_refused_refresh_needs_a_sign_in() {
    let (url, _server) = token_server().await;
    let root = crate::workspace::tests::temp_workspace("oauth-code-refused");
    let code_client = OAuthClient {
        grant: OAuthGrant::AuthorizationCode,
        ..client(&url)
    };

    let err = renew(
        &root,
        &code_client,
        &secret_values(),
        expired_with_refresh(),
    )
    .await
    .unwrap_err();

    assert!(matches!(err, Error::NeedsAuth(name) if name == "api"));
}

#[tokio::test]
async fn a_network_error_on_refresh_stays_a_network_error() {
    // Bound and dropped, so nothing listens there.
    let port = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let root = crate::workspace::tests::temp_workspace("oauth-code-offline");
    let code_client = OAuthClient {
        grant: OAuthGrant::AuthorizationCode,
        ..client(&format!("http://127.0.0.1:{port}/token"))
    };

    let err = renew(
        &root,
        &code_client,
        &secret_values(),
        expired_with_refresh(),
    )
    .await
    .unwrap_err();

    assert!(matches!(err, Error::Http(_)), "{err:?}");
}
