//! Native macOS menu. The default one binds ⌘W to "Close Window", which would close the whole
//! app instead of a tab, so we define our own (without a Close Window item, whose predefined
//! version claims ⌘W too) and forward app actions to the UI as `menu` events. On Windows/Linux there's no menu bar; the UI handles the same shortcuts itself.

use tauri::menu::{AboutMetadata, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Runtime};

const OPEN_WORKSPACE: &str = "open-workspace";
const SETTINGS: &str = "settings";
const NEW_REQUEST: &str = "new-request";
const CLOSE_TAB: &str = "close-tab";

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    Menu::with_items(
        app,
        &[
            &app_menu(app)?,
            &file_menu(app)?,
            &edit_menu(app)?,
            &window_menu(app)?,
        ],
    )
}

fn app_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    Submenu::with_items(
        app,
        "Plumbq",
        true,
        &[
            &PredefinedMenuItem::about(app, None, Some(about(app)))?,
            &MenuItem::with_id(app, SETTINGS, "Settings…", true, Some("CmdOrCtrl+,"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )
}

fn file_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    Submenu::with_items(
        app,
        "File",
        true,
        &[
            &MenuItem::with_id(
                app,
                OPEN_WORKSPACE,
                "Open Workspace…",
                true,
                Some("CmdOrCtrl+O"),
            )?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, NEW_REQUEST, "New Request", true, Some("CmdOrCtrl+N"))?,
            &MenuItem::with_id(app, CLOSE_TAB, "Close Tab", true, Some("CmdOrCtrl+W"))?,
        ],
    )
}

/// Needed on macOS for copy/paste to work in text inputs at all.
fn edit_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )
}

fn window_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Submenu<R>> {
    Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
            &PredefinedMenuItem::fullscreen(app, None)?,
        ],
    )
}

fn about<R: Runtime>(app: &AppHandle<R>) -> AboutMetadata<'static> {
    AboutMetadata {
        authors: Some(vec!["Nedim Talovic".into()]),
        comments: Some("The free API client your AI assistant can work in.".into()),
        copyright: Some("© 2026 Nedim Talovic · MIT License".into()),
        license: Some("MIT".into()),
        website: Some("https://github.com/talovicnedim/plumbq".into()),
        // Without a bundle (dev builds) macOS would show a generic folder icon.
        icon: app
            .default_window_icon()
            .map(|icon| icon.clone().to_owned()),
        ..Default::default()
    }
}

/// Forwards our own menu items to the UI; predefined items are handled natively.
pub fn forward<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    let id = event.id().as_ref();
    if [OPEN_WORKSPACE, NEW_REQUEST, CLOSE_TAB, SETTINGS].contains(&id)
        && let Err(err) = app.emit("menu", id)
    {
        tracing::warn!("failed to forward menu event {id}: {err}");
    }
}
