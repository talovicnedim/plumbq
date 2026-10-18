#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Settings or input that can't work, e.g. a provider without a key.
    #[error("{0}")]
    Invalid(String),

    /// The user has to agree first, e.g. to what goes to a cloud provider.
    #[error("{0}")]
    NeedsConfirmation(String),

    /// The provider couldn't be reached, rejected the key, or stopped mid-answer.
    #[error("{0}")]
    Provider(String),

    /// The in-process MCP connection failed.
    #[error("{0}")]
    Tools(String),

    #[error(transparent)]
    Core(#[from] plumbq_core::Error),

    #[error("failed to access assistant data: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid json: {0}")]
    Json(#[from] serde_json::Error),
}

impl Error {
    /// Stable machine-readable identifier, sent to the UI alongside the message.
    pub fn kind(&self) -> &'static str {
        match self {
            Error::Invalid(_) => "invalid",
            Error::NeedsConfirmation(_) => "needsConfirmation",
            Error::Provider(_) => "provider",
            Error::Tools(_) => "tools",
            Error::Core(err) => err.kind(),
            Error::Io(_) => "io",
            Error::Json(_) => "json",
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
