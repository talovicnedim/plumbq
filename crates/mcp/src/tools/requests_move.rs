use plumbq_core::permissions::{Permission, PermissionContext};
use std::path::Path;

use plumbq_core::requests::{self, IdChange, RequestMove};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{schemars, tool, tool_router};
use serde::Deserialize;

use crate::outline;
use crate::server::{Call, PlumbqMcp, error_text};
use crate::tools::requests::{folder_label, shifted};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MoveParams {
    /// Request id from workspace_tree.
    pub id: String,
    /// Target folder path; empty string for the workspace root.
    pub folder: String,
    /// Position among the folder's children, 0 = first. Omit to put it last.
    pub position: Option<u32>,
    /// The `version` requests_get returned, to refuse the move if the request changed since.
    pub version: Option<String>,
    /// One short sentence for the user on why you want this; shown when they're asked to
    /// approve it.
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MoveStep {
    /// Request id from workspace_tree.
    pub id: String,
    /// Target folder path; empty string for the workspace root.
    pub folder: String,
    /// Position among the folder's children, 0 = first. Omit to put it last.
    pub position: Option<u32>,
    /// The `version` requests_get returned, to refuse the move if the request changed since.
    pub version: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MoveManyParams {
    /// Moves done in this order. Ids are the ones from before this call; ids of requests that
    /// share a multi-request file with a moved one are adjusted for you.
    pub moves: Vec<MoveStep>,
    /// One short sentence for the user on why you want this; shown when they're asked to
    /// approve it.
    pub reason: Option<String>,
}

#[tool_router(router = requests_move_router, vis = "pub(crate)")]
impl PlumbqMcp {
    /// Moves a request to another folder and/or position. Use it to group and reorder. Returns
    /// the new id (it changes when the folder changes).
    #[tool(annotations(
        title = "Move request",
        read_only_hint = false,
        destructive_hint = false
    ))]
    async fn requests_move(&self, Parameters(p): Parameters<MoveParams>) -> Result<String, String> {
        let root = self.root()?;
        self.authorize(Call {
            tool: "requests_move",
            permission: Permission::EditFiles,
            context: PermissionContext::default(),
            action: format!("move {} to {}", p.id, folder_label(&p.folder)),
            detail: move_line(&p.id, &p.folder, p.position),
            environment: None,
            reason: p.reason.clone(),
        })
        .await?;
        let _lock = self.write_lock().await;
        let expected = self.expected_source(&root, &p.id, p.version.as_deref())?;
        let moved = requests::move_to(&root, &p.id, &p.folder, p.position, expected.as_deref())
            .map_err(error_text)?;
        self.follow_history(&root, &moved.changes);
        Ok(format!("Moved to {}{}", moved.id, shifted(&moved.changes)))
    }

    /// Moves many requests in one call, in order, e.g. to regroup a whole workspace. Stops at
    /// the first failure and says which moves were done. Returns `old id -> new id` lines.
    #[tool(annotations(
        title = "Move requests",
        read_only_hint = false,
        destructive_hint = false
    ))]
    async fn requests_move_many(
        &self,
        Parameters(p): Parameters<MoveManyParams>,
    ) -> Result<String, String> {
        let root = self.root()?;
        self.authorize(Call {
            tool: "requests_move_many",
            permission: Permission::EditFiles,
            context: PermissionContext::default(),
            action: format!("move {} requests", p.moves.len()),
            detail: p
                .moves
                .iter()
                .map(|step| move_line(&step.id, &step.folder, step.position))
                .collect::<Vec<_>>()
                .join("\n"),
            environment: None,
            reason: p.reason,
        })
        .await?;
        let _lock = self.write_lock().await;
        let mut done: Vec<(String, String)> = Vec::new();
        let mut changes: Vec<IdChange> = Vec::new();
        for (index, step) in p.moves.iter().enumerate() {
            let Some(id) = current_id(&step.id, &changes) else {
                return Err(format!(
                    "move {} ({}): that request no longer exists",
                    index + 1,
                    step.id
                ));
            };
            match self.move_step(&root, &id, step) {
                Ok(moved) => {
                    self.follow_history(&root, &moved.changes);
                    changes.extend(moved.changes);
                    done.push((id, moved.id));
                }
                Err(err) => {
                    return Err(format!(
                        "move {} ({}) failed: {err}\nDone before it:\n{}",
                        index + 1,
                        step.id,
                        outline::moves(&done)
                    ));
                }
            }
        }
        Ok(outline::moves(&done))
    }
}

impl PlumbqMcp {
    /// One move of a batch; `id` is the step's id after the batch's earlier moves.
    fn move_step(&self, root: &Path, id: &str, step: &MoveStep) -> Result<RequestMove, String> {
        let expected = self.expected_source(root, id, step.version.as_deref())?;
        requests::move_to(root, id, &step.folder, step.position, expected.as_deref())
            .map_err(error_text)
    }
}

/// An id from before a batch, after the changes made so far (moves, and ids shifting in
/// multi-request files). `None` when the request was removed.
pub fn current_id(original: &str, changes: &[IdChange]) -> Option<String> {
    let mut id = original.to_string();
    for change in changes {
        if change.from == id {
            id = change.to.clone()?;
        }
    }
    Some(id)
}

fn move_line(id: &str, folder: &str, position: Option<u32>) -> String {
    let position = position
        .map(|p| format!(" at position {p}"))
        .unwrap_or_default();
    format!("{id} -> {}{position}", folder_label(folder))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_batch_step_refuses_a_request_that_changed_since_it_was_read() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let root = std::env::temp_dir().join(format!("ac-mcp-move-{nanos}"));
        std::fs::create_dir_all(root.join("users")).unwrap();
        std::fs::write(root.join("a.http"), "GET http://x.test/a\n").unwrap();
        let read = requests::version(&requests::get(&root, "a.http").unwrap());
        std::fs::write(root.join("a.http"), "GET http://x.test/changed\n").unwrap();
        let server = PlumbqMcp::test(root.clone());
        let step = |version: String| MoveStep {
            id: "a.http".into(),
            folder: "users".into(),
            position: None,
            version: Some(version),
        };

        let err = server.move_step(&root, "a.http", &step(read)).unwrap_err();
        assert!(err.contains("changed since you read it"), "{err}");

        let fresh = requests::version(&requests::get(&root, "a.http").unwrap());
        let moved = server.move_step(&root, "a.http", &step(fresh)).unwrap();
        assert_eq!(moved.id, "users/a.http");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ids_follow_earlier_changes_of_a_batch() {
        let change = |from: &str, to: &str| IdChange {
            from: from.into(),
            to: Some(to.into()),
        };
        let changes = vec![
            change("all.http#2", "users/two.http"),
            change("all.http#3", "all.http#2"),
        ];
        assert_eq!(
            current_id("all.http#2", &changes).as_deref(),
            Some("users/two.http")
        );
        assert_eq!(
            current_id("all.http#3", &changes).as_deref(),
            Some("all.http#2")
        );
        assert_eq!(
            current_id("all.http", &changes).as_deref(),
            Some("all.http")
        );
        let removed = [IdChange {
            from: "gone.http".into(),
            to: None,
        }];
        assert_eq!(current_id("gone.http", &removed), None);
    }
}
