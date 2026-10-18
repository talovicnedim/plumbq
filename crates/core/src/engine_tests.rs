//! End-to-end tests of the HTTP engine against local servers: file variables, file and
//! multipart bodies, and per-request settings.

use std::collections::BTreeMap;
use std::path::Path;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::requests;
use crate::send::{SendContext, send};
use crate::workspace::tests::{temp_workspace, write};

/// Serves `replies` in order, one per connection, and returns what each request sent.
async fn server(replies: Vec<String>) -> (String, tokio::task::JoinHandle<Vec<Vec<u8>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        let mut received = Vec::new();
        for reply in replies {
            let (mut socket, _) = listener.accept().await.unwrap();
            received.push(read_request(&mut socket).await);
            socket.write_all(reply.as_bytes()).await.unwrap();
        }
        received
    });
    (base, handle)
}

/// Reads headers, then as many body bytes as `content-length` says.
async fn read_request(socket: &mut tokio::net::TcpStream) -> Vec<u8> {
    let mut data = Vec::new();
    let mut buf = vec![0; 65536];
    loop {
        let n = socket.read(&mut buf).await.unwrap();
        data.extend_from_slice(&buf[..n]);
        let text = String::from_utf8_lossy(&data).to_lowercase();
        let Some(end) = text.find("\r\n\r\n") else {
            continue;
        };
        let length = text[..end]
            .lines()
            .find_map(|l| l.strip_prefix("content-length:"))
            .and_then(|v| v.trim().parse::<usize>().ok())
            .unwrap_or(0);
        if n == 0 || data.len() >= end + 4 + length {
            return data;
        }
    }
}

fn ok(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n{body}",
        body.len()
    )
}

async fn send_saved(root: &Path, id: &str) -> crate::Result<crate::send::Response> {
    let request = requests::get(root, id)?;
    send(
        root,
        id,
        &request,
        None,
        &BTreeMap::new(),
        &SendContext::default(),
        false,
    )
    .await
}

#[tokio::test]
async fn file_variables_and_file_bodies_are_sent() {
    let (base, server) = server(vec![ok("done")]).await;
    let root = temp_workspace("engine-file");
    write(&root, "users/new.json", "{\"name\":\"Ada\"}");
    write(
        &root,
        "users/create.http",
        &format!(
            "@api = {base}/v2\n\n### Create\nPOST {{{{api}}}}/users\nContent-Type: application/json\n\n< ./new.json\n"
        ),
    );

    let response = send_saved(&root, "users/create.http").await.unwrap();

    let sent = String::from_utf8(server.await.unwrap().remove(0)).unwrap();
    assert!(sent.starts_with("POST /v2/users "), "{sent}");
    assert!(sent.ends_with("{\"name\":\"Ada\"}"));
    assert_eq!(response.sent.body, "<file ./new.json, 14 B>");
}

#[tokio::test]
async fn multipart_uploads_send_file_bytes() {
    let (base, server) = server(vec![ok("")]).await;
    let root = temp_workspace("engine-multipart");
    std::fs::write(root.join("pixel.bin"), [0u8, 255, 1]).unwrap();
    write(
        &root,
        "upload.http",
        &format!(
            "POST {base}/upload\nContent-Type: multipart/form-data; boundary=B\n\n--B\n\
             Content-Disposition: form-data; name=\"title\"\n\nhi\n--B\n\
             Content-Disposition: form-data; name=\"f\"; filename=\"pixel.bin\"\n\n< ./pixel.bin\n--B--\n"
        ),
    );

    send_saved(&root, "upload.http").await.unwrap();

    let sent = server.await.unwrap().remove(0);
    let tail = b"filename=\"pixel.bin\"\r\n\r\n\x00\xff\x01\r\n--B--\r\n";
    assert!(sent.ends_with(tail), "{}", String::from_utf8_lossy(&sent));
    assert!(String::from_utf8_lossy(&sent).contains("name=\"title\"\r\n\r\nhi\r\n--B\r\n"));
}

#[tokio::test]
async fn no_redirect_returns_the_redirect_itself() {
    let redirect =
        "HTTP/1.1 302 Found\r\nlocation: /final\r\ncontent-length: 0\r\n\r\n".to_string();
    let (base, server) = server(vec![redirect.clone(), ok("final")]).await;
    let root = temp_workspace("engine-redirect");
    write(&root, "follow.http", &format!("GET {base}/start\n"));
    write(
        &root,
        "stay.http",
        &format!("# @no-redirect\nGET {base}/start\n"),
    );

    let followed = send_saved(&root, "follow.http").await.unwrap();
    assert_eq!(followed.status, 200);
    assert_eq!(server.await.unwrap().len(), 2);

    let (base, _server) = self::server(vec![redirect]).await;
    write(
        &root,
        "stay.http",
        &format!("# @no-redirect\nGET {base}/start\n"),
    );
    let stayed = send_saved(&root, "stay.http").await.unwrap();
    assert_eq!(stayed.status, 302);
}

#[tokio::test]
async fn timeout_directive_limits_the_wait() {
    // Accepts the connection but never answers.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let _hold = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        drop(socket);
    });
    let root = temp_workspace("engine-timeout");
    write(
        &root,
        "slow.http",
        &format!("# @timeout 1\nGET {base}/slow\n"),
    );

    let started = std::time::Instant::now();
    let err = send_saved(&root, "slow.http").await.unwrap_err();

    assert!(started.elapsed() < std::time::Duration::from_secs(10));
    assert!(err.to_string().contains("timed out"), "{err}");
}
