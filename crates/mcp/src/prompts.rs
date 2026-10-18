//! Prompt templates MCP clients show as ready-made tasks (e.g. Claude's `/` menu).

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{PromptMessage, Role};
use rmcp::{prompt, prompt_router, schemars};
use serde::Deserialize;

use crate::server::PlumbqMcp;

/// Argument names a client can fill from what the user has open (the built-in chat does).
pub const FOLDER_ARG: &str = "folder";
pub const REQUEST_ARG: &str = "id";

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FolderArgs {
    /// Folder path, or empty for the whole workspace.
    #[serde(default)]
    pub folder: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RequestArgs {
    /// Request id, e.g. `users/create-user.http`.
    pub id: String,
    /// Environment to use.
    pub environment: Option<String>,
}

fn user(text: String) -> Vec<PromptMessage> {
    vec![PromptMessage::new_text(Role::User, text)]
}

fn scope(folder: &str) -> String {
    match folder {
        "" => "the whole workspace".to_string(),
        folder => format!("folder `{folder}`"),
    }
}

#[prompt_router(vis = "pub(crate)")]
impl PlumbqMcp {
    /// Group requests into folders by resource and order them in a sensible way.
    #[prompt(name = "organize_workspace")]
    async fn organize_workspace(
        &self,
        Parameters(args): Parameters<FolderArgs>,
    ) -> Vec<PromptMessage> {
        user(format!(
            "Organize {}. Read it with workspace_tree, then group requests into folders by \
             resource (users, orders, auth, …), order each folder CRUD-style (list, get, create, \
             update, delete) with folders_reorder, and give requests clear \"Verb resource\" \
             names. Use requests_move_many for the moves. Don't delete anything; say what you \
             changed at the end.",
            scope(&args.folder)
        ))
    }

    /// Add `# @assert` checks to requests that have none.
    #[prompt(name = "write_assertions")]
    async fn write_assertions(
        &self,
        Parameters(args): Parameters<FolderArgs>,
    ) -> Vec<PromptMessage> {
        user(format!(
            "Add assertions to the requests in {} that have none. For each, read it with \
             requests_get, look at a recent response (history_list / history_get) or a saved \
             example (examples_list), and add `# @assert` lines before the request line: the \
             expected status, a duration limit, and one or two body checks such as \
             `body.$.id exists`. Update with requests_update, passing the version you read. \
             Then run them with folders_run and fix any assertion that's wrong.",
            scope(&args.folder)
        ))
    }

    /// Write docs for requests from their saved examples and responses.
    #[prompt(name = "document_api")]
    async fn document_api(&self, Parameters(args): Parameters<FolderArgs>) -> Vec<PromptMessage> {
        user(format!(
            "Document the requests in {}. For each, write two or three lines of markdown docs \
             as `#` comment lines above the request line: what it does, required variables, and \
             what the response contains (use examples_list, or history_get for a recent \
             response). Where a request has no example, keep a good recent response with \
             examples_create. Keep the rest of the request unchanged.",
            scope(&args.folder)
        ))
    }

    /// Find out why a request fails and propose a fix.
    #[prompt(name = "debug_failing_request")]
    async fn debug_failing_request(
        &self,
        Parameters(args): Parameters<RequestArgs>,
    ) -> Vec<PromptMessage> {
        let env = args
            .environment
            .map(|e| format!(" with environment `{e}`"))
            .unwrap_or_default();
        user(format!(
            "Request `{}` fails{env}. Check what it sends with requests_preview, compare its \
             latest responses with history_compare, and look at the folder's shared headers \
             (folders_get) and the environment (environments_list). Explain the most likely \
             cause, then propose the smallest fix as a requests_update; if a secret or a value \
             only the user knows is missing, ask them instead of guessing.",
            args.id
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompts_name_their_arguments_like_the_constants() {
        let names: Vec<String> = PlumbqMcp::prompt_router()
            .list_all()
            .into_iter()
            .flat_map(|prompt| prompt.arguments.unwrap_or_default())
            .map(|argument| argument.name)
            .collect();
        assert!(names.iter().any(|n| n == FOLDER_ARG));
        assert!(names.iter().any(|n| n == REQUEST_ARG));
    }
}
