//! What shapes the client for one send: the workspace `[settings]` and the request's directives.
//! Settings: `docs/formats.md` (Workspace HTTP settings).

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::guard::AgentRedirects;
use crate::request_settings::RequestSettings;
use crate::workspace;
use crate::{Result, client_pfx};

const DEFAULT_TIMEOUT_SECS: u32 = 30;

/// What a send uses where the workspace and the request say nothing, for the settings forms.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HttpDefaults {
    pub timeout_secs: u32,
}

pub fn defaults() -> HttpDefaults {
    HttpDefaults {
        timeout_secs: DEFAULT_TIMEOUT_SECS,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HttpSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connect_timeout_secs: Option<u32>,
    /// Defaults to true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow_redirects: Option<bool>,
    /// Defaults to true. False accepts invalid and self-signed certificates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify_tls: Option<bool>,
    /// `""` uses the system proxy (environment variables and OS settings), `"none"` connects
    /// directly, anything else is a proxy URL (`http://`, `https://`, `socks5://`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub proxy: String,
    /// Extra trusted CA certificate(s), PEM.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ca_cert: String,
    /// Client certificate for mutual TLS, PEM; may contain the private key too.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub client_cert: String,
    /// Private key for `client_cert`, PEM, when it's in its own file.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub client_key: String,
    /// Client certificate as PKCS#12 (`.pfx` / `.p12`) instead of PEM; its password is kept in
    /// the keychain (see [`crate::client_pfx`]).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub client_pfx: String,
}

impl HttpSettings {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}

/// Which redirects to another origin (scheme, host, port) are followed. Secrets must not go
/// along: reqwest forwards every header but `Authorization` and `Cookie`, and a 307 or 308
/// sends the body again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossOriginRedirects {
    Follow,
    /// The body holds a secret: stop at a 307 or 308.
    StopBodyResends,
    /// Headers hold a secret: stop at every redirect to another origin.
    Stop,
}

/// Everything that shapes the client for one send.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientOptions {
    pub timeout: Duration,
    pub connect_timeout: Option<Duration>,
    pub follow_redirects: bool,
    pub verify_tls: bool,
    pub proxy: String,
    pub ca_cert: Option<PathBuf>,
    pub client_cert: Option<PathBuf>,
    pub client_key: Option<PathBuf>,
    pub client_pfx: Option<PathBuf>,
    pub client_pfx_password: String,
    /// A stopped redirect returns the redirect response itself.
    pub cross_origin_redirects: CrossOriginRedirects,
    /// Set for an AI agent's send: redirects to another origin these don't allow are stopped,
    /// and names that resolve to a private address are refused.
    pub agent_redirects: Option<AgentRedirects>,
}

/// The workspace's HTTP settings, from the root `folder.toml`.
pub fn workspace_settings(root: &Path) -> Result<HttpSettings> {
    Ok(workspace::read_settings(root)?.settings)
}

pub fn options(
    root: &Path,
    workspace: &HttpSettings,
    request: &RequestSettings,
) -> Result<ClientOptions> {
    let secs = |s: u32| Duration::from_secs(u64::from(s));
    Ok(ClientOptions {
        timeout: secs(
            request
                .timeout_secs
                .or(workspace.timeout_secs)
                .unwrap_or(DEFAULT_TIMEOUT_SECS),
        ),
        connect_timeout: request
            .connection_timeout_secs
            .or(workspace.connect_timeout_secs)
            .map(secs),
        follow_redirects: !request.no_redirect && workspace.follow_redirects.unwrap_or(true),
        verify_tls: !request.insecure && workspace.verify_tls.unwrap_or(true),
        proxy: workspace.proxy.trim().to_string(),
        ca_cert: file_setting(root, &workspace.ca_cert)?,
        client_cert: file_setting(root, &workspace.client_cert)?,
        client_key: file_setting(root, &workspace.client_key)?,
        client_pfx: file_setting(root, &workspace.client_pfx)?,
        // Only read the keychain when a PFX is configured.
        client_pfx_password: match workspace.client_pfx.trim().is_empty() {
            true => String::new(),
            false => client_pfx::password(root)?,
        },
        cross_origin_redirects: CrossOriginRedirects::Follow,
        agent_redirects: None,
    })
}

/// A settings path: absolute, or relative to the workspace (and then inside it).
fn file_setting(root: &Path, value: &str) -> Result<Option<PathBuf>> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return Ok(Some(path.to_path_buf()));
    }
    let path = workspace::resolve(root, value)?;
    // A missing file fails when the client is built; an existing one must really be inside the
    // workspace, not a symlink to somewhere else.
    match path.exists() {
        true => workspace::resolve_existing(root, value).map(Some),
        false => Ok(Some(path)),
    }
}
