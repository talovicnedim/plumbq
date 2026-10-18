use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::*;
use crate::workspace::tests::{temp_workspace, write};

/// Serves one HTTP response that echoes the raw request back as the body.
async fn echo_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0; 8192];
        let n = socket.read(&mut buf).await.unwrap();
        let body = String::from_utf8_lossy(&buf[..n]).to_string();
        let reply = format!(
            "HTTP/1.1 201 Created\r\ncontent-type: text/plain\r\ncontent-length: {}\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(reply.as_bytes()).await.unwrap();
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn sends_with_variables_folder_headers_and_masked_secrets() {
    let root = temp_workspace("send");
    write(&root, "users/folder.toml", "[headers]\nX-Team = \"core\"\n");
    let base = echo_server().await;
    let values = Values {
        variables: BTreeMap::from([("baseUrl".into(), base)]),
        secrets: BTreeMap::from([("token".into(), "s3cr3t".into())]),
        ..Default::default()
    };
    let mut request = Request::new("Create", "post", "{{baseUrl}}/users");
    request.headers.push(Header {
        name: "Authorization".into(),
        value: "Bearer {{token}}".into(),
    });

    let sent = prepare(&root, "users/create.http", &request, &values).unwrap();
    let response = mask_response(execute(&sent).await.unwrap(), &values);

    assert_eq!(response.status, 201);
    assert!(response.body.starts_with("POST /users"));
    assert!(response.body.contains("x-team: core"));
    assert!(response.body.contains("Bearer {{token}}"));
    assert!(!response.body.contains("s3cr3t"));
    assert_eq!(response.sent.headers[1].value, "Bearer {{token}}");
}

#[tokio::test]
async fn readable_basic_credentials_go_out_encoded_and_come_back_masked() {
    let root = temp_workspace("send-basic");
    let values = Values {
        variables: BTreeMap::from([("user".into(), "ada".into())]),
        secrets: BTreeMap::from([("pw".into(), "hunter22".into())]),
        ..Default::default()
    };
    for template in ["Basic {{user}}:{{pw}}", "basic {{user}} {{pw}}"] {
        let mut request = Request::new("", "GET", &echo_server().await);
        request.headers.push(Header {
            name: "Authorization".into(),
            value: template.into(),
        });

        let Wire {
            prepared,
            body,
            client,
            ..
        } = wire(&root, "x.http", &request, None, None, &values, false).unwrap();
        let (response, _) = execute_with(&client, &prepared, body).await.unwrap();

        assert!(
            response
                .body
                .contains("authorization: Basic YWRhOmh1bnRlcjIy"),
            "{template}: {}",
            response.body
        );
        let masked = mask_response(response, &values);
        assert!(!masked.body.contains("YWRhOmh1bnRlcjIy"));
        assert_eq!(masked.sent.headers[0].value, "Basic {{pw}}");
    }
}

#[test]
fn overrides_of_secrets_stay_secret() {
    let mut values = secret_values("from-keychain");
    values.variables.insert("userId".into(), "1".into());
    let context = SendContext {
        variables: BTreeMap::from([
            ("token".into(), "from-row".into()),
            ("userId".into(), "7".into()),
        ]),
        secrets: BTreeMap::from([("apiKey".into(), "from-cli".into())]),
        ..Default::default()
    };

    apply_overrides(&context, &mut values);

    assert_eq!(values.secrets["token"], "from-row");
    assert_eq!(values.secrets["apiKey"], "from-cli");
    assert_eq!(values.variables["userId"], "7");
    assert!(!values.variables.contains_key("token"));
}

#[test]
fn prompts_needed_include_folder_headers() {
    let root = temp_workspace("send-prompts");
    write(
        &root,
        "folder.toml",
        "[headers]\nX-Otp = \"{{$prompt otp}}\"\n",
    );
    let request = Request::new("", "POST", "/login?user={{$prompt user}}");

    let names = prompts_needed(&root, "login.http", &request).unwrap();
    assert_eq!(names, ["user", "otp"]);
}

#[tokio::test]
async fn send_fails_before_sending_without_prompt_answers() {
    let root = temp_workspace("send-needs-input");
    // Port 9 (discard) would fail to connect; reaching it would be a different error.
    let request = Request::new("", "GET", "http://127.0.0.1:9/?code={{$prompt code}}");

    let result = send(
        &root,
        "x.http",
        &request,
        None,
        &BTreeMap::new(),
        &SendContext::default(),
        false,
    )
    .await;
    assert!(matches!(result, Err(Error::NeedsInput(names)) if names == ["code"]));
}

#[test]
fn request_headers_override_folder_headers() {
    let root = temp_workspace("send-override");
    write(&root, "folder.toml", "[headers]\nAccept = \"text/plain\"\n");
    let mut request = Request::new("", "GET", "example.test");
    request.headers.push(Header {
        name: "accept".into(),
        value: "application/json".into(),
    });

    let sent = prepare(&root, "a.http", &request, &Values::default()).unwrap();
    assert_eq!(sent.url, "http://example.test");
    assert_eq!(sent.headers.len(), 1);
    assert_eq!(sent.headers[0].value, "application/json");
}

#[test]
fn urls_without_a_scheme_get_http_even_with_one_in_the_query() {
    let root = temp_workspace("send-scheme");
    let url = |raw: &str| {
        prepare(
            &root,
            "a.http",
            &Request::new("", "GET", raw),
            &Values::default(),
        )
        .unwrap()
        .url
    };
    assert_eq!(
        url("localhost:3000/cb?next=https://x.test"),
        "http://localhost:3000/cb?next=https://x.test"
    );
    assert_eq!(url("https://x.test/a"), "https://x.test/a");
    assert_eq!(url("HTTP://x.test"), "HTTP://x.test");
}

/// Answers each connection with `reply(request_head)`.
pub(super) async fn scripted_server(replies: Vec<String>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        for reply in replies {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let mut buf = vec![0; 8192];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let head = String::from_utf8_lossy(&buf[..n]).to_string();
            let reply = reply.replace("{echo}", &head);
            let _ = socket.write_all(reply.as_bytes()).await;
        }
    });
    format!("http://{addr}")
}

pub(super) fn secret_values(secret: &str) -> Values {
    Values {
        secrets: BTreeMap::from([("token".into(), secret.into())]),
        ..Default::default()
    }
}

#[tokio::test]
async fn transport_errors_omit_the_url() {
    let root = temp_workspace("send-error-url");
    write(
        &root,
        "environments/dev.toml",
        "secrets = [\"token\"]\n[variables]\n",
    );
    // Port 9 refuses connections; reqwest's message would include the URL with the key.
    let request = Request::new("", "GET", "http://127.0.0.1:9/x?key=plain-key-value");
    let err = send(
        &root,
        "x.http",
        &request,
        None,
        &BTreeMap::new(),
        &SendContext::default(),
        false,
    )
    .await
    .unwrap_err();
    assert!(!err.to_string().contains("plain-key-value"), "{err}");
}

#[tokio::test]
async fn huge_bodies_are_cut_without_reading_everything() {
    // Announces far more than it sends; reading stops one chunk past the cap.
    let body = "x".repeat(MAX_BODY_BYTES + 4096);
    let base = scripted_server(vec![format!(
        "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )])
    .await;
    let sent = SentRequest {
        method: "GET".into(),
        url: base,
        headers: Vec::new(),
        body: String::new(),
    };
    let response = execute(&sent).await.unwrap();
    assert!(response.truncated);
    assert_eq!(response.body.len(), MAX_BODY_BYTES);
    assert_eq!(response.size as usize, body.len());
}
