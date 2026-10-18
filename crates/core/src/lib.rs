//! Workspace, `.http` and HTTP logic shared by the app, MCP server and CLI. One module per
//! concept, plain functions that take the workspace root; files are the truth, nothing is cached.

pub mod app_dirs;
pub mod app_secrets;
pub mod approvals;
pub mod assertions;
pub mod blocking;
pub mod body_files;
pub mod chain;
pub mod client_pfx;
pub mod codegen;
pub mod compare;
pub mod cookies;
pub mod curl;
pub mod demo;
pub mod dynamic;
pub mod environments;
mod error;
pub mod examples;
pub mod export;
pub mod file_vars;
mod folder_file;
pub mod folders;
mod form_encoding;
pub mod fs_util;
pub mod guard;
pub mod history;
pub mod http_client;
pub mod http_file;
pub mod import;
pub mod inspect;
pub mod json_path;
pub mod multipart;
pub mod oauth;
pub mod permissions;
pub mod recent;
pub mod request_meta;
pub mod request_settings;
pub mod requests;
pub mod runner;
pub mod secret_names;
mod secrets;
pub mod send;
pub mod text_diff;
pub mod time;
pub mod variables;
pub mod workspace;

pub use error::{Error, Result};

#[cfg(test)]
mod engine_tests;
