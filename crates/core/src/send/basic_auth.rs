//! `Authorization: Basic` with readable credentials, `user:password` or `user password` (as in
//! the VS Code REST Client and JetBrains), so they can use `{{variables}}`. Base64 is applied
//! only on the wire.

use base64::Engine;

use crate::environments::Values;
use crate::http_file::Header;
use crate::{Result, variables};

/// Fills in an `Authorization` value; readable Basic credentials become `Basic user:password`.
/// The form is read from the template, so a secret with a space or `:` in it can't change it.
pub(super) fn substitute(template: &str, values: &Values) -> Result<String> {
    let Some(credentials) = credentials(template) else {
        return variables::substitute(template, values);
    };
    let value = match space_split(credentials) {
        Some(at) => {
            let (user, password) = credentials.split_at(at);
            let user = variables::substitute(user, values)?;
            let password = variables::substitute(password.trim_start(), values)?;
            format!("{user}:{password}")
        }
        None => variables::substitute(credentials, values)?,
    };
    Ok(format!("Basic {value}"))
}

/// Base64-encodes readable credentials right before sending.
pub(super) fn encode(headers: &mut [Header]) {
    for header in headers.iter_mut() {
        if let Some(credentials) = readable(header) {
            let encoded = base64::engine::general_purpose::STANDARD.encode(credentials);
            header.value = format!("Basic {encoded}");
        }
    }
}

/// `user:password` of an `Authorization: Basic user:password` header. Base64 never contains a
/// `:`, so encoded credentials are never taken for readable ones.
pub(crate) fn readable(header: &Header) -> Option<&str> {
    if !header.name.eq_ignore_ascii_case("authorization") {
        return None;
    }
    credentials(&header.value).filter(|c| c.contains(':'))
}

fn credentials(value: &str) -> Option<&str> {
    let value = value.trim_start();
    let (scheme, rest) = value.split_at_checked(5)?;
    let starts_with_space = rest.starts_with(|c: char| c.is_ascii_whitespace());
    (scheme.eq_ignore_ascii_case("basic") && starts_with_space).then(|| rest.trim())
}

/// Where `user password` splits: the first space outside `{{variables}}`, unless a `:` comes
/// first (then it is `user:password`, and a space belongs to the password).
fn space_split(credentials: &str) -> Option<usize> {
    let mut plain = credentials.as_bytes().to_vec();
    let mut from = 0;
    while let Some(start) = credentials[from..].find("{{").map(|i| from + i) {
        let Some(len) = credentials[start..].find("}}") else {
            break;
        };
        plain[start..start + len + 2].fill(b'x');
        from = start + len + 2;
    }
    let at = plain
        .iter()
        .position(|b| *b == b':' || b.is_ascii_whitespace())?;
    (plain[at] != b':').then_some(at)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn values(password: &str) -> Values {
        Values {
            variables: BTreeMap::from([("user".into(), "ada".into())]),
            secrets: BTreeMap::from([("pw".into(), password.into())]),
            ..Default::default()
        }
    }

    #[test]
    fn both_readable_forms_become_user_colon_password() {
        let spaced = values("correct horse");
        assert_eq!(
            substitute("Basic {{user}} {{pw}}", &spaced).unwrap(),
            "Basic ada:correct horse"
        );
        assert_eq!(
            substitute("Basic {{user}}:{{pw}}", &spaced).unwrap(),
            "Basic ada:correct horse"
        );
        let colon = values("a:b c");
        assert_eq!(
            substitute("Basic {{user}} {{pw}}", &colon).unwrap(),
            "Basic ada:a:b c"
        );
    }

    #[test]
    fn other_values_are_only_substituted() {
        let values = values("hunter22");
        assert_eq!(
            substitute("Basic YWRhOnB3", &values).unwrap(),
            "Basic YWRhOnB3"
        );
        assert_eq!(
            substitute("Bearer {{pw}}", &values).unwrap(),
            "Bearer hunter22"
        );
        assert_eq!(
            substitute("Basically {{user}}", &values).unwrap(),
            "Basically ada"
        );
    }

    #[test]
    fn only_readable_credentials_are_encoded() {
        let header = |name: &str, value: &str| Header {
            name: name.into(),
            value: value.into(),
        };
        let mut headers = vec![
            header("Authorization", "Basic ada:pw"),
            header("Authorization", "Basic YWRhOnB3"),
            header("X-Note", "Basic ada:pw"),
        ];
        encode(&mut headers);
        assert_eq!(headers[0].value, "Basic YWRhOnB3");
        assert_eq!(headers[1].value, "Basic YWRhOnB3");
        assert_eq!(headers[2].value, "Basic ada:pw");
    }
}
