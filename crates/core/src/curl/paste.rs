//! "Paste cURL" in the app: the user typed the credentials on purpose, so they are kept, but as
//! secrets of the selected environment in the keychain, never in the request file.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use super::parse::parse;
use crate::http_file::Request;
use crate::import::credentials;
use crate::{Result, environments};

/// A pasted command with its credentials as `{{secret}}` placeholders, and their values.
/// No `Debug`: the values must never reach a log.
pub struct CurlPaste {
    pub request: Request,
    values: BTreeMap<String, String>,
}

/// What happened to the pasted credentials.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CurlSecrets {
    /// Stored as secrets of the environment.
    pub saved: Vec<String>,
    /// Already secrets of the environment, so the old value stays.
    pub kept: Vec<String>,
    /// Not stored because no environment was selected; the user sets them later.
    pub unsaved: Vec<String>,
}

pub fn paste(command: &str) -> Result<CurlPaste> {
    let mut request = parse(command)?;
    let mut found = credentials::Found::default();
    credentials::request(&mut request, &mut found);
    Ok(CurlPaste {
        request,
        values: found.into_values(),
    })
}

impl CurlPaste {
    pub(super) fn names(&self) -> Vec<String> {
        self.values.keys().cloned().collect()
    }
}

/// Stores the pasted credentials as secrets of `environment`. Without an environment nothing is
/// stored. A secret the environment already has is left as it is, so a paste never overwrites a
/// value the user set before.
pub fn keep_secrets(
    root: &Path,
    environment: Option<&str>,
    paste: &CurlPaste,
) -> Result<CurlSecrets> {
    let mut report = CurlSecrets::default();
    let Some(environment) = environment else {
        report.unsaved = paste.names();
        return Ok(report);
    };
    let existing = environments::secret_names(root, environment)?;
    for (name, value) in &paste.values {
        if existing.contains(name) {
            report.kept.push(name.clone());
            continue;
        }
        environments::set_secret(root, environment, name, value)?;
        report.saved.push(name.clone());
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::tests::{temp_workspace, write};

    const COMMAND: &str =
        "curl -H 'Cookie: sid=abc' -d 'client_secret=s3cr3t' https://x.test/login";

    #[test]
    fn a_paste_holds_the_values_and_the_request_only_placeholders() {
        let pasted = paste(COMMAND).unwrap();

        assert_eq!(pasted.names(), ["Cookie", "client_secret"]);
        assert_eq!(pasted.values["Cookie"], "sid=abc");
        assert_eq!(pasted.request.body, "client_secret={{client_secret}}");
        assert_eq!(pasted.request.headers[0].value, "{{Cookie}}");
    }

    #[test]
    fn without_an_environment_nothing_is_stored() {
        let root = temp_workspace("curl-paste-none");
        let report = keep_secrets(&root, None, &paste(COMMAND).unwrap()).unwrap();

        assert_eq!(report.unsaved, ["Cookie", "client_secret"]);
        assert!(report.saved.is_empty() && report.kept.is_empty());
    }

    #[test]
    fn an_unknown_environment_stores_nothing() {
        let root = temp_workspace("curl-paste-unknown");
        let err = keep_secrets(&root, Some("local"), &paste(COMMAND).unwrap()).unwrap_err();
        assert_eq!(err.kind(), "notFound");
    }

    #[test]
    fn secrets_the_environment_already_has_are_not_overwritten() {
        let root = temp_workspace("curl-paste-kept");
        write(
            &root,
            "environments/local.toml",
            "secrets = [\"Cookie\", \"client_secret\"]\n",
        );
        let report = keep_secrets(&root, Some("local"), &paste(COMMAND).unwrap()).unwrap();

        assert_eq!(report.kept, ["Cookie", "client_secret"]);
        assert!(report.saved.is_empty() && report.unsaved.is_empty());
    }
}
