//! Binary responses: kept as base64 for previews and "save to file".

use std::collections::BTreeMap;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::*;
use crate::workspace::tests::temp_workspace;

/// The 8-byte PNG signature plus a byte that is invalid UTF-8.
const PNG: [u8; 9] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0xFF];

async fn png_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0; 4096];
        let _ = socket.read(&mut buf).await.unwrap();
        let head = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: image/png\r\ncontent-length: {}\r\n\r\n",
            PNG.len()
        );
        socket.write_all(head.as_bytes()).await.unwrap();
        socket.write_all(&PNG).await.unwrap();
    });
    format!("http://{addr}/logo.png")
}

#[tokio::test]
async fn binary_bodies_round_trip_as_base64() {
    let root = temp_workspace("send-binary");
    let request = Request::new("Logo", "GET", &png_server().await);

    let response = send(
        &root,
        "logo.http",
        &request,
        None,
        &BTreeMap::new(),
        &SendContext::default(),
        false,
    )
    .await
    .unwrap();

    assert!(response.binary);
    assert!(response.body.is_empty());
    assert_eq!(response.body_base64, "iVBORw0KGgr/");
    assert_eq!(body_bytes(&response).unwrap(), PNG);
}

#[test]
fn text_bodies_are_their_utf8_bytes() {
    let response = Response {
        status: 200,
        status_text: String::new(),
        headers: Vec::new(),
        body: "héllo".into(),
        size: 6,
        binary: false,
        body_base64: String::new(),
        truncated: false,
        duration_ms: 0,
        sent: SentRequest {
            method: "GET".into(),
            url: String::new(),
            headers: Vec::new(),
            body: String::new(),
        },
        assertions: Vec::new(),
        masked_secrets: Vec::new(),
    };
    assert_eq!(body_bytes(&response).unwrap(), "héllo".as_bytes());
}
