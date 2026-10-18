//! Helpers shared by the MCP integration tests.

use std::path::PathBuf;

use rmcp::RoleClient;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::RunningService;

pub fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ac-test-mcp-{name}-{}",
        &plumbq_mcp::settings::new_token()[..12]
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub async fn call(
    client: &RunningService<RoleClient, ()>,
    tool: &'static str,
    args: serde_json::Value,
) -> CallToolResult {
    let serde_json::Value::Object(args) = args else {
        panic!("args must be an object");
    };
    client
        .call_tool(CallToolRequestParams::new(tool).with_arguments(args))
        .await
        .unwrap()
}

pub fn text(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.clone()))
        .collect()
}
