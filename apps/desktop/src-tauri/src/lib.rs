mod assistant;
mod commands;
mod dialogs;
mod error;
mod mcp;
#[cfg(target_os = "macos")]
mod menu;
mod state;
mod watch;

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{DragDropEvent, Emitter, Manager, RunEvent, WindowEvent};

use crate::mcp::McpState;
use crate::state::AppState;

/// Event asking the UI to confirm quitting (it may have unsaved tabs); the UI answers with the
/// `app_quit` command.
pub const QUIT_REQUESTED: &str = "quit-requested";

/// Set by `app_quit` once the UI agreed, so the next exit request goes through.
pub static QUIT_CONFIRMED: AtomicBool = AtomicBool::new(false);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    plumbq_core::http_client::install_crypto();
    let builder = commands::builder();

    #[cfg(debug_assertions)]
    if let Err(err) = commands::export_bindings(&builder) {
        tracing::error!("failed to export typescript bindings: {err}");
    }

    let app = tauri::Builder::default()
        .plugin(logging())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init());
    #[cfg(target_os = "macos")]
    let app = app.menu(menu::build).on_menu_event(menu::forward);

    let result = app
        .on_window_event(grant_dropped_files)
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            setup(app)
        })
        .build(tauri::generate_context!());

    match result {
        Ok(app) => app.run(ask_before_quitting),
        Err(err) => {
            tracing::error!("error while running the app: {err}");
            std::process::exit(1);
        }
    }
}

/// Files dropped on the window may be imported: the user chose them, like in a dialog.
fn grant_dropped_files(window: &tauri::Window, event: &WindowEvent) {
    if let WindowEvent::DragDrop(DragDropEvent::Drop { paths, .. }) = event {
        let state = window.state::<AppState>();
        for path in paths {
            state.grant(path.clone());
        }
    }
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = app.path().app_data_dir()?;
    let state = AppState::new(&data_dir);
    let mcp = McpState::new(app.handle(), &data_dir, state.shared_root());
    mcp.start();
    app.manage(state);
    app.manage(mcp);
    app.manage(assistant::AssistantState::default());
    // Only signed release builds carry updater config (tauri.updater.conf.json); without it
    // the plugin would fail to start, so dev and unsigned builds simply have no updater.
    if app.config().plugins.0.contains_key("updater") {
        app.handle()
            .plugin(tauri_plugin_updater::Builder::new().build())?;
    }
    Ok(())
}

/// ⌘Q and closing the last window land here; the UI checks for unsaved tabs first.
fn ask_before_quitting(app: &tauri::AppHandle, event: RunEvent) {
    if let RunEvent::ExitRequested { api, .. } = event
        && !QUIT_CONFIRMED.load(Ordering::SeqCst)
    {
        api.prevent_exit();
        if let Err(err) = app.emit(QUIT_REQUESTED, ()) {
            tracing::warn!("failed to ask the UI before quitting: {err}");
        }
    }
}

/// The log file's name in the OS log folder, without `.log`.
pub(crate) const LOG_FILE: &str = "plumbq";

/// Log file in the OS log folder. Rotates at 5 MB, keeping three files. Never logs request or
/// response contents, which may hold credentials.
fn logging() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    use tauri_plugin_log::{RotationStrategy, Target, TargetKind};
    tauri_plugin_log::Builder::new()
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir {
                file_name: Some(LOG_FILE.into()),
            }),
        ])
        .level(log::LevelFilter::Info)
        .level_for("hyper", log::LevelFilter::Warn)
        .level_for("reqwest", log::LevelFilter::Warn)
        .level_for("rmcp", log::LevelFilter::Warn)
        .max_file_size(5_000_000)
        .rotation_strategy(RotationStrategy::KeepSome(3))
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    static LINES: Mutex<Vec<String>> = Mutex::new(Vec::new());

    /// Stands in for the log plugin, which is a `log` logger too.
    struct Capture;

    impl log::Log for Capture {
        fn enabled(&self, _: &log::Metadata<'_>) -> bool {
            true
        }

        fn log(&self, record: &log::Record<'_>) {
            LINES.lock().unwrap().push(record.args().to_string());
        }

        fn flush(&self) {}
    }

    fn files(dir: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .unwrap()
            .flat_map(|entry| {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    files(&path)
                } else {
                    vec![path]
                }
            })
            .collect()
    }

    #[test]
    fn warnings_from_core_reach_the_log_file() {
        log::set_logger(&Capture).unwrap();
        log::set_max_level(log::LevelFilter::Info);
        let history = std::env::temp_dir().join(format!(
            "ac-test-desktop-log-{}",
            &plumbq_mcp::settings::new_token()[..12]
        ));
        let root = Path::new("/ws");
        let response = serde_json::from_value(serde_json::json!({
            "status": 200, "statusText": "OK", "headers": [], "body": "", "size": 0,
            "binary": false, "bodyBase64": "", "truncated": false, "durationMs": 1,
            "sent": {"method": "GET", "url": "http://x.test", "headers": [], "body": ""},
            "assertions": []
        }))
        .unwrap();
        plumbq_core::history::add(&history, root, "a.http", None, response).unwrap();
        std::fs::write(&files(&history)[0], "{ broken").unwrap();

        assert!(
            plumbq_core::history::list(&history, root, "a.http")
                .unwrap()
                .is_empty()
        );
        let lines = LINES.lock().unwrap();
        assert!(
            lines
                .iter()
                .any(|l| l.contains("skipping unreadable history entry")),
            "{lines:?}"
        );
    }
}
