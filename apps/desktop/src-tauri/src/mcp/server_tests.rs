#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{Read, Write};
use std::sync::{Arc, RwLock};

use plumbq_mcp::Approver;

use super::*;

/// Like startup: a plain thread without a Tokio runtime.
#[test]
fn starts_from_a_thread_without_a_runtime() {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let settings = McpSettings {
        enabled: true,
        port,
        ..McpSettings::default()
    };
    let path = plumbq_mcp::path(&settings.token);
    let (approver, _approvals) = Approver::channel();
    let server = Server::new(HttpServer {
        workspace: Arc::default(),
        data_dir: std::env::temp_dir().join("ac-test-mcp-host"),
        settings: Arc::new(RwLock::new(settings)),
        approver,
        write_lock: Default::default(),
    });
    server.start();
    assert!(server.url_and_error().0.is_some());

    // The server answers HTTP (an MCP POST without a body is a client error, not a hang).
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(
        stream,
        "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    let _ = stream.read_to_string(&mut response);
    assert!(response.starts_with("HTTP/1.1 4"), "{response}");
}
