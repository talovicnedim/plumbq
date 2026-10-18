#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Settings the user entered that can't work, e.g. a reserved port.
    #[error("{0}")]
    Invalid(String),
}

impl Error {
    /// The stable id sent to the UI.
    pub fn kind(&self) -> &'static str {
        match self {
            Error::Io(_) => "io",
            Error::Json(_) => "json",
            Error::Invalid(_) => "invalid",
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
