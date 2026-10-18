use super::wire::add_cookie_header;
use reqwest::Url;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::*;
use crate::workspace::tests::temp_workspace;

/// Answers two requests: the first sets a cookie, the second echoes what it received.
async fn cookie_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        for set_cookie in ["set-cookie: sid=abc; Path=/; HttpOnly\r\n", ""] {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0; 8192];
            let n = socket.read(&mut buf).await.unwrap();
            let body = String::from_utf8_lossy(&buf[..n]).to_string();
            let reply = format!(
                "HTTP/1.1 200 OK\r\n{set_cookie}content-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(reply.as_bytes()).await.unwrap();
        }
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn cookies_from_a_response_go_with_the_next_request() {
    let root = temp_workspace("send-cookies");
    let context = SendContext {
        history_dir: None,
        cookie_dir: Some(temp_workspace("send-cookie-jar")),
        ..Default::default()
    };
    let base = cookie_server().await;
    let login = Request::new("", "POST", &format!("{base}/login"));
    let me = Request::new("", "GET", &format!("{base}/me"));
    let send_one = |request: Request| {
        let (root, context) = (root.clone(), context.clone());
        async move {
            send(
                &root,
                "x.http",
                &request,
                None,
                &BTreeMap::new(),
                &context,
                false,
            )
            .await
        }
    };

    send_one(login).await.unwrap();
    let response = send_one(me).await.unwrap();

    assert!(response.body.contains("cookie: sid=abc"));
    let jar = crate::cookies::list(context.cookie_dir.as_deref().unwrap(), &root, None).unwrap();
    assert_eq!(jar.len(), 1);
}

#[test]
fn explicit_cookie_header_wins_per_name() {
    let jar = vec![
        crate::cookies::parse("sid=jar", &Url::parse("http://x.test/").unwrap(), 0).unwrap(),
        crate::cookies::parse("theme=dark", &Url::parse("http://x.test/").unwrap(), 0).unwrap(),
    ];
    let mut sent = SentRequest {
        method: "GET".into(),
        url: "http://x.test/".into(),
        headers: vec![Header {
            name: "Cookie".into(),
            value: "sid=mine".into(),
        }],
        body: String::new(),
    };

    add_cookie_header(&mut sent, &jar);

    assert_eq!(sent.headers.len(), 1);
    assert_eq!(sent.headers[0].value, "sid=mine; theme=dark");
}
