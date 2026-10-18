use std::path::{Path, PathBuf};

use plumbq_core::environments::EnvironmentTag;
use plumbq_core::import::{ImportEnvironmentChange, ImportedCollection};
use plumbq_core::permissions::{Permission, PermissionContext};
use plumbq_core::{Error, blocking, guard, import};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{schemars, tool, tool_router};
use serde::Deserialize;

use crate::format;
use crate::server::{Call, PlumbqMcp, error_text, shown};
use crate::settings::is_hidden;
use crate::tools::requests::folder_label;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FileParams {
    /// Folder path to import into; empty string for the workspace root.
    pub folder: String,
    /// Path of a Postman collection, OpenAPI / Swagger spec, Insomnia export, HAR file or Bruno
    /// collection folder. It must be inside the workspace (or the server's --import-dir);
    /// relative paths are relative to the workspace.
    pub path: String,
    /// One short sentence for the user on why you want this; shown when they're asked to
    /// approve it.
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UrlParams {
    /// Folder path to import into; empty string for the workspace root.
    pub folder: String,
    /// http(s) URL of an OpenAPI / Swagger spec or a collection file, e.g.
    /// `https://petstore3.swagger.io/api/v3/openapi.json`.
    pub url: String,
    /// One short sentence for the user on why you want this; shown when they're asked to
    /// approve it.
    pub reason: Option<String>,
}

#[tool_router(router = import_router, vis = "pub(crate)")]
impl PlumbqMcp {
    /// Imports a collection file (format detected from its content) as folders of `.http`
    /// requests plus environments. Importing the same collection again updates the earlier
    /// import. Returns a summary, including secrets the user needs to set.
    #[tool(annotations(
        title = "Import collection",
        read_only_hint = false,
        destructive_hint = true
    ))]
    async fn import_file(&self, Parameters(p): Parameters<FileParams>) -> Result<String, String> {
        let root = self.root()?;
        let path = self.importable(&root, &p.path)?;
        let collection = blocking::run(move || import::read_file(&path))
            .await
            .map_err(|_| unreadable(&p.path))?;
        let action = format!(
            "import {} into {}",
            shown(&p.path),
            shown(&folder_label(&p.folder))
        );
        self.import(
            &root,
            &p.folder,
            collection,
            "import_file",
            action,
            p.reason,
        )
        .await
    }

    /// Imports a spec or collection from a URL. Importing the same spec again updates the
    /// earlier import instead of duplicating it. Returns a summary, including secrets the user
    /// needs to set.
    #[tool(annotations(
        title = "Import from URL",
        read_only_hint = false,
        destructive_hint = true,
        open_world_hint = true
    ))]
    async fn import_url(&self, Parameters(p): Parameters<UrlParams>) -> Result<String, String> {
        let root = self.root()?;
        // A GET of a URL the agent picked: the user is asked unless the host is a known, public
        // one. They see what it imports before anything is written.
        self.authorize(Call {
            tool: "import_url",
            permission: Permission::SendSafe,
            context: guard::assess_download(&root, &p.url).map_err(error_text)?,
            action: format!("download {} to import it", shown(&p.url)),
            detail: format!("GET {}", shown(&p.url)),
            environment: None,
            reason: p.reason.clone(),
        })
        .await?;
        let redirects = guard::AgentRedirects::for_downloads(&root, &p.url).map_err(error_text)?;
        let collection =
            import::download(&p.url, Some(redirects))
                .await
                .map_err(|err| match err {
                    Error::Http(_) => error_text(err),
                    _ => unreadable(&p.url),
                })?;
        let action = format!(
            "import {} into {}",
            shown(&p.url),
            shown(&folder_label(&p.folder))
        );
        self.import(&root, &p.folder, collection, "import_url", action, p.reason)
            .await
    }
}

impl PlumbqMcp {
    /// Asks the user with what the import does to environments, then writes it. Agents never
    /// add to a production or hidden environment: its URLs decide where its secrets may go.
    async fn import(
        &self,
        root: &Path,
        folder: &str,
        collection: ImportedCollection,
        tool: &'static str,
        action: String,
        reason: Option<String>,
    ) -> Result<String, String> {
        let changes = import::environment_changes(root, folder, &collection).map_err(error_text)?;
        self.authorize(Call {
            tool,
            permission: Permission::EditFiles,
            context: PermissionContext {
                forbidden: self.protected(&changes),
                ..PermissionContext::default()
            },
            action,
            detail: import_detail(&changes),
            environment: None,
            reason,
        })
        .await?;
        let _lock = self.write_lock().await;
        if import::environment_changes(root, folder, &collection).map_err(error_text)? != changes {
            return Err("the environments changed while the user was asked; import again".into());
        }
        let summary = import::write(root, folder, collection).map_err(error_text)?;
        summary_text(&summary)
    }

    /// Why the import may not run for an agent, if it would add to a protected environment.
    fn protected(&self, changes: &[ImportEnvironmentChange]) -> Option<String> {
        let hidden = self.hidden_environments();
        let change = changes.iter().find(|c| {
            c.existing && (c.tag == EnvironmentTag::Production || is_hidden(&hidden, &c.name))
        })?;
        let which = match is_hidden(&hidden, &change.name) {
            true => "an environment hidden from AI".to_string(),
            false => format!("production environment {}", change.name),
        };
        Some(format!(
            "this import would add variables to {which}; ask the user to import it in the app"
        ))
    }
}

/// What the user approves: the requests in general, the environments in full.
fn import_detail(changes: &[ImportEnvironmentChange]) -> String {
    let mut out = "Creates a folder of requests (or updates an earlier import of the same \
                   collection)."
        .to_string();
    for change in changes {
        let (mark, what) = match change.existing {
            true => ('~', "adds to"),
            false => ('+', "creates"),
        };
        let tag = format!("{:?}", change.tag).to_lowercase();
        out.push_str(&format!(
            "\n{mark} {what} environment {} ({tag})",
            shown(&change.name)
        ));
        for (name, value) in &change.variables {
            out.push_str(&format!("\n    {} = {}", shown(name), shown(value)));
        }
        if !change.secrets.is_empty() {
            out.push_str(&format!(
                "\n    secrets: {}",
                shown(&change.secrets.join(", "))
            ));
        }
    }
    out
}

/// The import summary as JSON, cut when an import reports many notes or stale requests.
fn summary_text(summary: &import::ImportSummary) -> Result<String, String> {
    let json = serde_json::to_string_pretty(summary).map_err(|err| err.to_string())?;
    Ok(format::cap_with(
        &json,
        "the app shows the full import summary",
    ))
}

/// What the agent hears when a file can't be imported: never the parser's message, which can
/// quote the file.
fn unreadable(source: &str) -> String {
    format!(
        "{source} couldn't be read as a Postman, OpenAPI, Insomnia, HAR or Bruno file; the user \
         can import it in the app to see why"
    )
}

impl PlumbqMcp {
    /// The file to import, if it's inside the workspace or the import folder and isn't an
    /// environment or `.env` file. An agent can't read (or probe for) other files on the user's
    /// machine.
    fn importable(&self, root: &Path, path: &str) -> Result<PathBuf, String> {
        let extra: Vec<&Path> = self.import_dir.as_deref().into_iter().collect();
        let path = import::allowed_path(root, path, &extra).map_err(|err| match err {
            Error::Invalid(message) => format!(
                "{message}; ask the user to copy it into the workspace, or to start the server \
                 with --import-dir"
            ),
            other => error_text(other),
        })?;
        match guard::private_file(root, &path) {
            Some(reason) => Err(reason),
            None => Ok(path),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use plumbq_core::workspace;

    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ac-mcp-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        workspace::canonical_root(&dir).unwrap()
    }

    #[test]
    fn imports_only_from_the_workspace_or_the_import_dir() {
        let root = temp("import-root");
        let downloads = temp("import-downloads");
        let elsewhere = temp("import-elsewhere");
        for dir in [&root, &downloads, &elsewhere] {
            fs::write(dir.join("spec.json"), "{}").unwrap();
        }
        let server = PlumbqMcp::test(root.clone()).with_import_dir(Some(downloads.clone()));

        assert_eq!(
            server.importable(&root, "spec.json").unwrap(),
            root.join("spec.json")
        );
        let outside = downloads.join("spec.json");
        assert_eq!(
            server
                .importable(&root, &outside.display().to_string())
                .unwrap(),
            outside
        );
        let refused = server.importable(&root, &elsewhere.join("spec.json").display().to_string());
        assert!(refused.unwrap_err().contains("outside the workspace"));
        assert!(server.importable(&root, "../x/spec.json").is_err());
    }
}
