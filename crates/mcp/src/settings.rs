//! MCP server settings, saved as JSON in the app data dir, where both the app's server and the
//! stdio server (`plumbq-mcp`) read them.

use std::fs;
use std::path::{Path, PathBuf};

use plumbq_core::fs_util;
use plumbq_core::permissions::AgentPermissions;
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Picked to be unlikely to clash with dev servers and sibling apps (Tugdeck uses 7457);
/// changeable in settings.
pub const DEFAULT_PORT: u16 = 7459;

const FILE: &str = "mcp.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct McpSettings {
    /// Run the server inside the app.
    pub enabled: bool,
    pub port: u16,
    /// Secret part of the server URL. Anyone with the URL can use the server, so it's random and
    /// can be regenerated.
    pub token: String,
    /// What agents may do without asking, after asking, or never.
    #[serde(default)]
    pub permissions: AgentPermissions,
    /// "Read-only" in the AI dialog: the AI can look at everything but change nothing, for the
    /// chat and AI apps alike. Overrides `permissions`.
    #[serde(default)]
    pub read_only: bool,
    /// Environments (by name, in every workspace) the AI can't see or use.
    #[serde(default)]
    pub hidden_environments: Vec<String>,
    /// "Turn AI off": no chat, no MCP server in the app, and the stdio server refuses every
    /// call, until turned back on. `enabled` stays as it was, so turning AI on brings the
    /// server back too.
    #[serde(default)]
    pub turned_off: bool,
}

impl Default for McpSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_PORT,
            token: new_token(),
            permissions: AgentPermissions::default(),
            read_only: false,
            hidden_environments: Vec::new(),
            turned_off: false,
        }
    }
}

pub fn path(data_dir: &Path) -> PathBuf {
    data_dir.join(FILE)
}

/// Saved settings, or defaults (with a fresh token) when there are none yet. Saved settings
/// the server can't run with are an error, like a file that doesn't parse.
pub fn load(path: &Path) -> Result<McpSettings> {
    match fs::read_to_string(path) {
        Ok(text) => {
            let settings = serde_json::from_str(&text)?;
            check(&settings)?;
            Ok(settings)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(McpSettings::default()),
        Err(err) => Err(err.into()),
    }
}

/// Ports below 1024 need admin rights on most systems, so the server couldn't start there.
const FIRST_FREE_PORT: u16 = 1024;
/// The token is the only credential, so it must be hard to guess: [`new_token`] makes 64.
const MIN_TOKEN_CHARS: usize = 32;

/// Refuses settings the server can't run with.
pub fn check(settings: &McpSettings) -> Result<()> {
    if settings.port < FIRST_FREE_PORT {
        return Err(Error::Invalid(format!(
            "pick a port from {FIRST_FREE_PORT} to 65535"
        )));
    }
    let token = &settings.token;
    if token.len() < MIN_TOKEN_CHARS || !token.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(Error::Invalid(format!(
            "the server address needs a token of at least {MIN_TOKEN_CHARS} letters and \
             digits; regenerate the address in AI → AI apps"
        )));
    }
    Ok(())
}

/// Saves readable by the user only: the token gives access to their workspaces.
pub fn save(path: &Path, settings: &McpSettings) -> Result<()> {
    fs_util::write_private(path, serde_json::to_string_pretty(settings)?)?;
    Ok(())
}

/// Whether `name` is one of the `hidden` environments, in any letter case: macOS and Windows
/// open `prod.toml` for `PROD` too. The one rule for MCP tools and the built-in chat.
pub fn is_hidden(hidden: &[String], name: &str) -> bool {
    let name = name.to_lowercase();
    hidden.iter().any(|h| h.to_lowercase() == name)
}

/// 256 random bits as 64 hex characters.
pub fn new_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!("ac-test-mcp-{name}-{}", &new_token()[..12]))
            .join(FILE)
    }

    #[test]
    fn missing_settings_are_defaults_with_a_fresh_token() {
        let settings = load(&temp_file("missing")).unwrap();
        assert!(!settings.enabled);
        assert_eq!(settings.port, DEFAULT_PORT);
        assert_eq!(settings.token.len(), 64);
        assert_ne!(settings.token, McpSettings::default().token);
    }

    #[test]
    fn saves_privately_and_loads_back() {
        let file = temp_file("save");
        let settings = McpSettings {
            enabled: true,
            port: 9000,
            ..McpSettings::default()
        };
        save(&file, &settings).unwrap();
        assert_eq!(load(&file).unwrap(), settings);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn reserved_ports_are_refused() {
        let low = McpSettings {
            port: 80,
            ..McpSettings::default()
        };
        assert!(matches!(check(&low), Err(Error::Invalid(_))));
        assert!(check(&McpSettings::default()).is_ok());
    }

    #[test]
    fn settings_without_permissions_get_defaults() {
        let file = temp_file("old");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        let token = new_token();
        fs::write(
            &file,
            format!(r#"{{"enabled":true,"port":7459,"token":"{token}"}}"#),
        )
        .unwrap();
        assert_eq!(
            load(&file).unwrap().permissions,
            AgentPermissions::default()
        );
    }

    #[test]
    fn short_or_path_like_tokens_are_refused() {
        for token in ["", "t", "../../evil", &format!("{}/x", new_token())] {
            let settings = McpSettings {
                token: token.to_string(),
                ..McpSettings::default()
            };
            assert!(check(&settings).is_err(), "{token:?} is refused");
            let file = temp_file("bad-token");
            save(&file, &settings).unwrap();
            assert!(
                load(&file).is_err(),
                "{token:?} in mcp.json is a settings problem"
            );
        }
    }
}
