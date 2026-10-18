//! Native file dialogs opened from Rust. Commands that read or write files outside the
//! workspace only ever use paths the user picked in one of these (or dropped on the window), so
//! a script running in the webview can't make the app touch arbitrary files.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder, FilePath};
use tokio::sync::oneshot;

/// What an open dialog is for; decides its title, filters and whether it picks a folder.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum OpenKind {
    /// A folder to open as the workspace.
    Workspace,
    /// The folder a new workspace is created in.
    WorkspaceParent,
    /// A Postman, OpenAPI, Swagger, Insomnia, HAR or single Bruno file.
    ImportFile,
    /// A Bruno collection folder.
    ImportFolder,
    /// A CSV or JSON file with one row per run iteration.
    DataFile,
}

/// Shows an open dialog of `kind`; `None` when cancelled.
pub async fn open(app: &AppHandle, kind: OpenKind) -> Option<PathBuf> {
    let dialog = app.dialog().file();
    let (title, folder) = match kind {
        OpenKind::Workspace => ("Open workspace folder", true),
        OpenKind::WorkspaceParent => ("Where should the new workspace folder go?", true),
        OpenKind::ImportFile => ("Import a collection", false),
        OpenKind::ImportFolder => ("Choose a Bruno collection folder", true),
        OpenKind::DataFile => ("Choose a CSV or JSON data file", false),
    };
    let dialog = match kind {
        OpenKind::ImportFile => dialog.add_filter(
            "Postman, OpenAPI, Swagger, Insomnia, HAR, Bruno",
            &["json", "yaml", "yml", "har", "bru"],
        ),
        OpenKind::DataFile => dialog.add_filter("CSV or JSON", &["csv", "json"]),
        _ => dialog,
    }
    .set_title(title);
    pick(dialog, folder).await
}

/// Shows an open dialog for a file to send in a request body, starting in the workspace
/// `root`; `None` when cancelled.
pub async fn open_body_file(app: &AppHandle, root: &Path) -> Option<PathBuf> {
    let dialog = app
        .dialog()
        .file()
        .set_title("Choose a file to send")
        .set_directory(root);
    pick(dialog, false).await
}

async fn pick(dialog: FileDialogBuilder<tauri::Wry>, folder: bool) -> Option<PathBuf> {
    let (tx, rx) = oneshot::channel();
    let send = move |path: Option<FilePath>| {
        // The receiver only goes away if the command was dropped; nothing to do then.
        let _ = tx.send(path);
    };
    if folder {
        dialog.pick_folder(send);
    } else {
        dialog.pick_file(send);
    }
    into_path(rx.await.ok().flatten())
}

/// Shows a save dialog suggesting `file_name`, limited to `extensions`; `None` when cancelled.
pub async fn save(
    app: &AppHandle,
    title: &str,
    file_name: &str,
    filter: (&str, &[&str]),
) -> Option<PathBuf> {
    let (tx, rx) = oneshot::channel();
    app.dialog()
        .file()
        .set_title(title)
        .set_file_name(file_name)
        .add_filter(filter.0, filter.1)
        .save_file(move |path| {
            let _ = tx.send(path);
        });
    into_path(rx.await.ok().flatten())
}

fn into_path(path: Option<FilePath>) -> Option<PathBuf> {
    path.and_then(|p| p.into_path().ok())
}
