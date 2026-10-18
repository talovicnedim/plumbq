//! Secret variable values in the OS keychain (macOS Keychain, Windows Credential Manager,
//! Secret Service on Linux). Workspace files only ever hold the secret's name.

use std::path::Path;

use crate::{Error, Result};

const SERVICE: &str = "plumbq";
/// Starts the environment or name of the app's own entries (`$auth:github`, `$workspace`); a
/// workspace secret can't start with it, so the two never meet.
const RESERVED: char = '$';

/// A workspace secret, or `None` when it was never set on this machine.
pub fn get(root: &Path, environment: &str, name: &str) -> Result<Option<String>> {
    read(&user_account(root, environment, name)?, name)
}

pub fn set(root: &Path, environment: &str, name: &str, value: &str) -> Result<()> {
    write(&user_account(root, environment, name)?, name, value)
}

pub fn remove(root: &Path, environment: &str, name: &str) -> Result<()> {
    delete(&user_account(root, environment, name)?, name)
}

/// The app's own entries in the same keychain service: OAuth tokens (`$auth:<client>`), the
/// PFX password (`$workspace`) and the chat's keys (root `$app`).
pub(crate) mod internal {
    use std::path::Path;

    use super::{delete, internal_account, read, write};
    use crate::Result;

    pub(crate) fn get(root: &Path, scope: &str, name: &str) -> Result<Option<String>> {
        read(&internal_account(root, scope, name)?, name)
    }

    pub(crate) fn set(root: &Path, scope: &str, name: &str, value: &str) -> Result<()> {
        write(&internal_account(root, scope, name)?, name, value)
    }

    pub(crate) fn remove(root: &Path, scope: &str, name: &str) -> Result<()> {
        delete(&internal_account(root, scope, name)?, name)
    }
}

/// A workspace secret's account: a real (absolute) workspace path, and an environment and name
/// that don't start with [`RESERVED`].
fn user_account(root: &Path, environment: &str, name: &str) -> Result<String> {
    if environment.starts_with(RESERVED) || name.starts_with(RESERVED) {
        return Err(Error::Invalid(format!(
            "secret {name} in {environment}: names starting with {RESERVED} are reserved"
        )));
    }
    if !root.is_absolute() {
        return Err(Error::Invalid(format!(
            "secret {name}: the workspace path {} isn't absolute",
            root.display()
        )));
    }
    account(root, environment, name)
}

/// One of the app's own accounts: everything [`user_account`] refuses.
fn internal_account(root: &Path, scope: &str, name: &str) -> Result<String> {
    if user_account(root, scope, name).is_ok() {
        return Err(Error::Invalid(format!(
            "{name} in {scope} is a workspace secret, not one of the app's own entries"
        )));
    }
    account(root, scope, name)
}

/// `{root}|{environment}|{name}`, keyed by workspace path so two checkouts of the same repo keep
/// separate secrets. Neither the environment nor the name may hold a `|`, so every account
/// reads back as exactly one of each, even when the path has one.
fn account(root: &Path, environment: &str, name: &str) -> Result<String> {
    if environment.contains('|') || name.contains('|') {
        return Err(Error::Invalid(format!(
            "secret {name} in {environment}: names can't contain |"
        )));
    }
    Ok(format!("{}|{environment}|{name}", root.display()))
}

fn read(account: &str, name: &str) -> Result<Option<String>> {
    match entry(account, name)?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(keychain_error(name, err)),
    }
}

fn write(account: &str, name: &str, value: &str) -> Result<()> {
    entry(account, name)?
        .set_password(value)
        .map_err(|err| keychain_error(name, err))
}

fn delete(account: &str, name: &str) -> Result<()> {
    match entry(account, name)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(keychain_error(name, err)),
    }
}

fn entry(account: &str, name: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, account).map_err(|err| keychain_error(name, err))
}

fn keychain_error(name: &str, err: keyring::Error) -> Error {
    Error::Keychain(format!(
        "failed to access secret {name} in the keychain: {err}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Absolute on every system: on Windows that needs a drive.
    #[cfg(not(windows))]
    const ROOT: &str = "/work/api";
    #[cfg(windows)]
    const ROOT: &str = r"C:\work\api";

    #[test]
    fn workspace_secrets_cant_reach_the_apps_own_entries() {
        let root = Path::new(ROOT);
        assert!(user_account(root, "dev", "token").is_ok());
        assert!(user_account(root, "dev", "$auth:github").is_err());
        assert!(user_account(root, "$workspace", "clientPfxPassword").is_err());
        assert!(user_account(Path::new("$app"), "assistant", "anthropic").is_err());
    }

    #[test]
    fn no_part_can_shift_into_another() {
        let root = Path::new(ROOT);
        assert!(user_account(root, "dev|prod", "token").is_err());
        assert!(user_account(root, "dev", "a|token").is_err());
        assert!(internal_account(root, "dev", "$auth:a|b").is_err());
        // The other two parts never hold a `|`, so one in the path can't be misread.
        let root = Path::new(ROOT).join("w|x");
        assert_eq!(
            user_account(&root, "dev", "token").unwrap(),
            format!("{}|dev|token", root.display())
        );
    }

    #[test]
    fn the_apps_own_entries_stay_out_of_workspace_secrets() {
        let root = Path::new(ROOT);
        assert!(internal_account(root, "dev", "$auth:github").is_ok());
        assert!(internal_account(root, "$workspace", "clientPfxPassword").is_ok());
        assert!(internal_account(Path::new("$app"), "assistant", "anthropic").is_ok());
        assert!(internal_account(root, "dev", "token").is_err());
    }
}
