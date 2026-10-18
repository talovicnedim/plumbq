//! Where per-machine data lives, shared by the desktop app and the MCP server so both see the
//! same history. Matches Tauri's `app_data_dir()` for the app's identifier.

use std::env;
use std::path::PathBuf;

/// The desktop app's bundle identifier (`tauri.conf.json`).
const IDENTIFIER: &str = "dev.plumbq.desktop";

/// `~/Library/Application Support/<id>` on macOS, `$XDG_DATA_HOME/<id>` (or
/// `~/.local/share/<id>`) on Linux, `%APPDATA%\<id>` on Windows.
pub fn data_dir() -> Option<PathBuf> {
    let base = if cfg!(target_os = "macos") {
        PathBuf::from(env::var_os("HOME")?).join("Library/Application Support")
    } else if cfg!(windows) {
        PathBuf::from(env::var_os("APPDATA")?)
    } else {
        match env::var_os("XDG_DATA_HOME") {
            Some(dir) => PathBuf::from(dir),
            None => PathBuf::from(env::var_os("HOME")?).join(".local/share"),
        }
    };
    Some(base.join(IDENTIFIER))
}

pub fn history_dir() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join("history"))
}

pub fn cookie_dir() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join("cookies"))
}
