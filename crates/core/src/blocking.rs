//! Keychain calls and big file reads block their thread. Async code runs them here, so the
//! runtime keeps serving other requests meanwhile.

use crate::{Error, Result};

pub async fn run<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|err| Error::Io(std::io::Error::other(err)))?
}
