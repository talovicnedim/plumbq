//! The safety rules over a real MCP client, on the server as the app runs it: hidden
//! environments, approvals that match what is sent, and what agents may send.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../common/mod.rs"]
mod common;
mod harness;

mod approvals;
mod environments;
mod imports;
mod sends;
