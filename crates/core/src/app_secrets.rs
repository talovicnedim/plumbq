//! The chat's provider keys in the OS keychain: same service as workspace secrets, under a scope
//! no workspace path or environment name can produce.

use std::path::Path;

use crate::{Result, secrets};

/// Not an absolute path, so no workspace's keychain entries can reach it (see
/// [`secrets::internal`]).
const SCOPE: &str = "$app";
const KIND: &str = "assistant";

pub fn get(name: &str) -> Result<Option<String>> {
    secrets::internal::get(Path::new(SCOPE), KIND, name)
}

/// Stores `value`; an empty value removes the entry.
pub fn set(name: &str, value: &str) -> Result<()> {
    if value.is_empty() {
        return remove(name);
    }
    secrets::internal::set(Path::new(SCOPE), KIND, name, value)
}

pub fn remove(name: &str) -> Result<()> {
    secrets::internal::remove(Path::new(SCOPE), KIND, name)
}

/// Whether a value is stored; an unreadable keychain counts as "no".
pub fn has(name: &str) -> bool {
    matches!(get(name), Ok(Some(_)))
}
