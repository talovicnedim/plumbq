//! MCP tools, one file per core module. Every tool is a thin adapter named `<resource>_<verb>`.

mod cookies;
mod environments;
mod examples;
mod folders;
mod folders_headers;
mod history;
mod import;
mod import_curl;
mod requests;
mod requests_move;
mod runner;
mod send;
mod workspace;
