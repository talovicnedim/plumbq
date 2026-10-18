//! Tauri commands. One file per core module. Commands that touch files or the keychain are
//! `#[tauri::command(async)]` so they run off the main thread (a keychain prompt or a big
//! import would otherwise freeze the window); every command is a thin adapter over
//! `plumbq-core` named `<resource>_<verb>`.

mod app;
mod assistant;
mod assistant_chats;
mod auth;
mod body_files;
mod compare;
mod cookies;
mod curl;
mod dialogs;
mod environments;
mod examples;
mod export;
mod folders;
mod history;
mod import;
mod json_path;
mod mcp;
mod multipart;
mod requests;
mod responses;
mod runner;
mod workspace;

use tauri_specta::{Builder, collect_commands};

#[cfg(any(debug_assertions, test))]
// From the crate folder, so running from another folder never writes the file elsewhere.
const BINDINGS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/api/bindings.ts");

pub fn builder() -> Builder<tauri::Wry> {
    Builder::new().commands(collect_commands![
        workspace::workspace_open,
        workspace::workspace_create,
        workspace::workspace_open_demo,
        workspace::workspace_current,
        workspace::workspace_rename,
        workspace::workspace_recent,
        workspace::workspace_tree,
        workspace::workspace_warnings,
        workspace::workspace_set_pfx_password,
        workspace::workspace_has_pfx_password,
        workspace::workspace_http_defaults,
        requests::requests_get,
        requests::requests_create,
        requests::requests_update,
        requests::requests_remove,
        requests::requests_move,
        requests::requests_prompts,
        requests::requests_inspect,
        requests::requests_tags_update,
        requests::requests_docs_update,
        requests::requests_send,
        requests::requests_cancel,
        responses::responses_export,
        body_files::body_files_choose,
        multipart::multipart_parse,
        multipart::multipart_format,
        dialogs::dialog_open,
        app::app_quit,
        app::app_log_file,
        json_path::json_path_query,
        requests::requests_export,
        requests::requests_import_curl,
        curl::curl_parse,
        cookies::cookies_list,
        cookies::cookies_clear,
        cookies::cookies_remove,
        auth::auth_sign_in,
        auth::auth_client_new,
        auth::auth_sign_in_needed,
        auth::auth_sign_out,
        folders::folders_create,
        folders::folders_rename,
        folders::folders_duplicate,
        folders::folders_remove,
        folders::folders_reorder,
        folders::folders_get_settings,
        folders::folders_update_settings,
        environments::environments_list,
        environments::environments_warnings,
        environments::environments_create,
        environments::environments_update,
        environments::environments_remove,
        environments::environments_set_secret,
        environments::environments_remove_secret,
        examples::examples_list,
        examples::examples_create,
        examples::examples_remove,
        compare::compare_bodies,
        history::history_list,
        history::history_get,
        import::import_file,
        mcp::mcp_binary,
        mcp::mcp_status,
        mcp::mcp_update,
        mcp::mcp_token_regenerate,
        mcp::mcp_apps_list,
        mcp::mcp_clients_list,
        mcp::mcp_apps_connect,
        mcp::mcp_apps_disconnect,
        mcp::mcp_approvals_list,
        mcp::mcp_approvals_answer,
        mcp::mcp_activity_list,
        mcp::mcp_activity_clear,
        import::import_url,
        runner::runner_preview,
        runner::runner_run,
        runner::runner_stop,
        runner::runner_export_report,
        export::export_postman,
        export::export_docs,
        assistant::assistant_status,
        assistant::assistant_update,
        assistant::assistant_provider_new,
        assistant::assistant_key_set,
        assistant::assistant_models,
        assistant::assistant_test,
        assistant::assistant_quick_actions,
        assistant::assistant_quick_action,
        assistant_chats::assistant_send,
        assistant_chats::assistant_stop,
        assistant_chats::assistant_chats_list,
        assistant_chats::assistant_chats_get,
        assistant_chats::assistant_chats_remove,
    ])
}

/// Regenerates `src/lib/api/bindings.ts` from the command signatures. Dev builds only.
#[cfg(any(debug_assertions, test))]
pub fn export_bindings(builder: &Builder<tauri::Wry>) -> Result<(), specta_typescript::Error> {
    builder.export(
        specta_typescript::Typescript::default().header("// @ts-nocheck\n/* eslint-disable */"),
        BINDINGS_PATH,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Keeps bindings in sync on `cargo test`, so CI catches a stale `bindings.ts` via git diff.
    #[test]
    fn export_typescript_bindings() {
        export_bindings(&builder()).unwrap();
    }
}
