#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Input rejected before touching anything, e.g. a path escaping the workspace.
    #[error("{0}")]
    Invalid(String),

    #[error("{0} not found")]
    NotFound(String),

    /// The request targets a production environment and needs the user's explicit OK.
    #[error("{0}")]
    NeedsConfirmation(String),

    /// `{{$prompt name}}` variables without an answer; holds their names.
    #[error("values needed for: {}", .0.join(", "))]
    NeedsInput(Vec<String>),

    /// An OAuth client needs the user to sign in in the browser first; holds the client name.
    #[error("sign in to {0} first")]
    NeedsAuth(String),

    /// The file changed on disk since the caller read it (e.g. an editor or agent inserted a
    /// request), so writing would overwrite or delete the wrong request.
    #[error("{0}")]
    Conflict(String),

    #[error("{0}")]
    Http(String),

    #[error("{0}")]
    Keychain(String),

    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("invalid toml: {0}")]
    TomlRead(#[from] toml::de::Error),

    #[error("failed to write toml: {0}")]
    TomlWrite(#[from] toml::ser::Error),

    #[error("invalid json: {0}")]
    Json(#[from] serde_json::Error),
}

impl Error {
    /// Stable machine-readable identifier, sent to the UI and MCP clients alongside the message.
    pub fn kind(&self) -> &'static str {
        match self {
            Error::Invalid(_) => "invalid",
            Error::NotFound(_) => "notFound",
            Error::NeedsConfirmation(_) => "needsConfirmation",
            Error::NeedsInput(_) => "needsInput",
            Error::NeedsAuth(_) => "needsAuth",
            Error::Conflict(_) => "conflict",
            Error::Http(_) => "http",
            Error::Keychain(_) => "keychain",
            Error::Io(_) => "io",
            Error::TomlRead(_) | Error::TomlWrite(_) => "toml",
            Error::Json(_) => "json",
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
