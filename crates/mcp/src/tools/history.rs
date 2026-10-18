use std::path::Path;

use plumbq_core::history::HistorySummary;
use plumbq_core::{compare, history};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{schemars, tool, tool_router};
use serde::Deserialize;

use crate::format;
use crate::server::{PlumbqMcp, error_text};
use crate::settings::is_hidden;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListParams {
    /// Request id from workspace_tree.
    pub request_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetParams {
    /// Request id from workspace_tree.
    pub request_id: String,
    /// Entry id from history_list.
    pub id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CompareParams {
    /// Request id from workspace_tree.
    pub request_id: String,
    /// Older history entry id. Omit to use the second newest.
    pub from: Option<String>,
    /// Newer history entry id. Omit to use the newest.
    pub to: Option<String>,
}

#[tool_router(router = history_router, vis = "pub(crate)")]
impl PlumbqMcp {
    /// Past sends of a request, newest first (from the app and from this server): id, time,
    /// status and duration. Useful to compare with an earlier response.
    #[tool(annotations(title = "Request history", read_only_hint = true))]
    async fn history_list(&self, Parameters(p): Parameters<ListParams>) -> Result<String, String> {
        let root = self.root()?;
        let Some(dir) = &self.history_dir else {
            return Ok("History is unavailable on this machine.".into());
        };
        let rows = self.visible_history(dir, &root, &p.request_id)?;
        Ok(format::history(&rows))
    }

    /// One past response in full (body capped), as recorded when it was sent.
    #[tool(annotations(title = "Past response", read_only_hint = true))]
    async fn history_get(&self, Parameters(p): Parameters<GetParams>) -> Result<String, String> {
        let root = self.root()?;
        let Some(dir) = &self.history_dir else {
            return Err("history is unavailable on this machine".into());
        };
        let entry = history::get(dir, &root, &p.request_id, &p.id).map_err(error_text)?;
        self.check_visible(entry.environment.as_deref())?;
        Ok(format::history_entry(&entry))
    }

    /// What changed between two sends of a request: status, timing and the body, field by field
    /// for JSON. By default compares the two most recent sends, e.g. "did my change break
    /// anything?".
    #[tool(annotations(title = "Compare responses", read_only_hint = true))]
    async fn history_compare(
        &self,
        Parameters(p): Parameters<CompareParams>,
    ) -> Result<String, String> {
        let root = self.root()?;
        let Some(dir) = &self.history_dir else {
            return Err("history is unavailable on this machine".into());
        };
        let rows = self.visible_history(dir, &root, &p.request_id)?;
        let pick = |wanted: &Option<String>, fallback: usize| -> Result<String, String> {
            match wanted {
                Some(id) => Ok(id.clone()),
                None => rows
                    .get(fallback)
                    .map(|row| row.id.clone())
                    .ok_or_else(|| "this request needs at least two sends to compare".to_string()),
            }
        };
        let (from, to) = (pick(&p.from, 1)?, pick(&p.to, 0)?);
        let older = history::get(dir, &root, &p.request_id, &from).map_err(error_text)?;
        let newer = history::get(dir, &root, &p.request_id, &to).map_err(error_text)?;
        self.check_visible(older.environment.as_deref())?;
        self.check_visible(newer.environment.as_deref())?;
        let (a, b) = (&older.response, &newer.response);
        Ok(format!(
            "From {} ({}, {} ms)\nTo   {} ({}, {} ms)\n\n{}",
            older.sent_at,
            a.status,
            a.duration_ms,
            newer.sent_at,
            b.status,
            b.duration_ms,
            format::comparison(&compare::bodies(&a.body, &b.body))
        ))
    }
}

impl PlumbqMcp {
    /// History rows without sends to environments hidden from AI.
    fn visible_history(
        &self,
        dir: &Path,
        root: &Path,
        request_id: &str,
    ) -> Result<Vec<HistorySummary>, String> {
        let hidden = self.hidden_environments();
        let mut rows = history::list(dir, root, request_id).map_err(error_text)?;
        rows.retain(|row| {
            row.environment
                .as_ref()
                .is_none_or(|e| !is_hidden(&hidden, e))
        });
        Ok(rows)
    }
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
