use plumbq_core::workspace;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{schemars, tool, tool_router};
use serde::Deserialize;

use crate::outline;
use crate::server::{PlumbqMcp, error_text};

/// Skipped files listed in workspace_tree; the rest are counted.
const MAX_WARNINGS: usize = 30;

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct TreeParams {
    /// Only this folder (path from an earlier call); empty or omitted for the whole workspace.
    #[serde(default)]
    pub folder: String,
    /// Folder levels to expand; deeper folders show their request count. Omit for all.
    pub depth: Option<u32>,
}

#[tool_router(router = workspace_router, vis = "pub(crate)")]
impl PlumbqMcp {
    /// The workspace as an outline: folders with their path and requests with their id, method,
    /// URL and tags, in display order, plus files that had to be skipped (not UTF-8, broken
    /// folder.toml). Start here; other tools take these ids and paths. Big workspaces are cut:
    /// narrow with `folder` / `depth`, or use requests_search.
    #[tool(annotations(title = "Show workspace", read_only_hint = true))]
    async fn workspace_tree(
        &self,
        Parameters(p): Parameters<TreeParams>,
    ) -> Result<String, String> {
        let root = self.root()?;
        let (nodes, warnings) = workspace::tree_with_warnings(&root).map_err(error_text)?;
        let Some(part) = outline::subtree(&nodes, p.folder.trim_matches('/')) else {
            return Err(format!("folder {} not found", p.folder));
        };
        let title = match p.folder.is_empty() {
            true => format!("Workspace {}", root.display()),
            false => format!("Folder {}", p.folder),
        };
        let mut out = format!("{title}\n\n{}", outline::tree(part, p.depth));
        if !warnings.is_empty() {
            out.push_str("\nSkipped (fix these files to see their requests):\n");
            let lines: Vec<String> = warnings
                .iter()
                .map(|warning| format!("  {}: {}", warning.path, warning.message))
                .collect();
            out.push_str(&outline::capped_lines(&lines, MAX_WARNINGS));
        }
        Ok(out)
    }
}
