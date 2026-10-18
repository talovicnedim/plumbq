use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::download::MAX_DOWNLOAD_BYTES;
use super::*;
use crate::workspace::tests::temp_workspace;

const SPEC: &str = r#"{"openapi": "3.0.0", "info": {"title": "Pets"}, "servers": [{"url": "https://pets.test"}],
    "paths": {"/pets": {"get": {"summary": "List pets"}}}}"#;

/// Serves `body` to each of `times` connections.
async fn serve(body: &'static str, times: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        for _ in 0..times {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0; 4096];
            let _ = socket.read(&mut buf).await.unwrap();
            let reply = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(reply.as_bytes()).await.unwrap();
        }
    });
    format!("http://{addr}/openapi.json")
}

#[tokio::test]
async fn imports_from_url_and_updates_on_second_import() {
    let root = temp_workspace("import-url");
    let address = serve(SPEC, 2).await;

    let first = url(&root, "", &address).await.unwrap();
    assert_eq!(first.format, ImportFormat::OpenApi);
    assert_eq!((first.folder.as_str(), first.added), ("pets", 1));

    let second = url(&root, "", &address).await.unwrap();
    assert!(second.merged);
    assert_eq!((second.added, second.unchanged), (0, 1));
}

/// Sends `head`, then `body_bytes` bytes, then keeps the connection open.
async fn serve_large(head: &'static str, body_bytes: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0; 4096];
        let _ = socket.read(&mut buf).await.unwrap();
        socket.write_all(head.as_bytes()).await.unwrap();
        let chunk = vec![b' '; 1024 * 1024];
        for _ in 0..body_bytes.div_ceil(chunk.len()) {
            if socket.write_all(&chunk).await.is_err() {
                return;
            }
        }
        std::future::pending::<()>().await;
    });
    format!("http://{addr}/openapi.json")
}

#[tokio::test]
async fn downloads_over_the_limit_stop_early() {
    let root = temp_workspace("import-url-large");
    let declared = serve_large("HTTP/1.1 200 OK\r\ncontent-length: 999999999\r\n\r\n", 0).await;
    let err = url(&root, "", &declared).await.unwrap_err();
    assert!(err.to_string().contains("too large"), "{err}");

    let streamed = serve_large(
        "HTTP/1.1 200 OK\r\ntransfer-encoding: identity\r\n\r\n",
        MAX_DOWNLOAD_BYTES + 1024 * 1024,
    )
    .await;
    let err = url(&root, "", &streamed).await.unwrap_err();
    assert!(err.to_string().contains("too large"), "{err}");
}

#[tokio::test]
async fn rejects_non_http_urls() {
    let root = temp_workspace("import-url-bad");
    assert!(url(&root, "", "file:///etc/passwd").await.is_err());
}

#[test]
fn rejects_unknown_documents() {
    assert!(parse("{\"hello\": 1}").is_err());
    assert!(parse("just: yaml").is_err());
}

#[test]
fn loads_yaml_and_json() {
    assert_eq!(load("a: 1").unwrap()["a"], 1);
    assert_eq!(load("{\"a\": 1}").unwrap()["a"], 1);
}
