use super::*;

#[test]
fn credential_names_are_secrets() {
    for name in [
        "token",
        "token2",
        "accessToken",
        "apiKey",
        "API_KEY",
        "x-api-key",
        "client_secret",
        "DB_PASSWORD",
        "stripeKey",
        "sessionId",
        "sessionid",
        "accesstoken",
        "apitoken",
        "TOKEN_VALUE",
        "AUTH_HEADER",
        "PRIVATE_KEY_PEM",
        "DBPASS",
        "AWS_ACCESS_KEY_ID",
        "X-CSRF-Token",
        "x-auth",
        "oauth",
        "Authorization",
        "Cookie",
        "Set-Cookie",
        "GITHUB_PAT",
        "SENTRY_DSN",
        "DB_PWD",
        "PASSWORD_URL",
        "PRIVATE_KEY_PATH",
        "clientSecretName",
    ] {
        assert!(looks_secret(name), "{name}");
    }
}

#[test]
fn names_that_describe_a_credential_keep_their_values() {
    for name in [
        "baseUrl",
        "userId",
        "keyboard",
        "author",
        "cookieDomain",
        "sessionTimeout",
        "tokenUrl",
        "token_type",
        "authUrl",
        "PATH",
        "HOME",
        "pathname",
    ] {
        assert!(!looks_secret(name), "{name}");
    }
}

/// The three rules this one replaced (chain paths, `$processEnv` names and imports), kept here
/// so the new rule can be checked to never mask less than they did.
mod old {
    pub fn chain(name: &str) -> bool {
        let lower = name.to_lowercase();
        ["token", "secret", "password", "key", "session", "cookie"]
            .iter()
            .any(|hint| lower.contains(hint))
    }

    pub fn dynamic(name: &str) -> bool {
        let upper = name.to_uppercase();
        [
            "TOKEN", "KEY", "SECRET", "PASS", "PWD", "AUTH", "CRED", "DSN", "PRIVATE", "CERT",
            "SESSION",
        ]
        .iter()
        .any(|hint| upper.contains(hint))
            || upper.split('_').any(|part| part == "PAT")
    }

    pub fn credentials(name: &str) -> bool {
        let lower = name.to_ascii_lowercase();
        [
            "token",
            "secret",
            "password",
            "passwd",
            "apikey",
            "api_key",
            "api-key",
            "credential",
            "private",
            "session",
            "csrf",
            "cookie",
        ]
        .iter()
        .any(|hint| lower.contains(hint))
            || ["key", "auth"].iter().any(|end| lower.ends_with(end))
    }
}

/// Names the old rules masked and this one deliberately does not.
const INTENDED_UNMASKED: [&str; 8] = [
    "cookieDomain",
    "sessionTimeout",
    "tokenUrl",
    "authUrl",
    "token_type",
    "author",
    "keyboard",
    "sessionExpires",
];

#[test]
fn only_the_named_names_are_masked_less_than_before() {
    for name in INTENDED_UNMASKED {
        let was_secret = old::chain(name) || old::dynamic(name) || old::credentials(name);
        assert!(was_secret && !looks_secret(name), "{name}");
    }
}

#[test]
fn nothing_else_the_old_rules_masked_is_unmasked() {
    let hints = [
        "token",
        "secret",
        "password",
        "passwd",
        "pass",
        "pwd",
        "key",
        "session",
        "cookie",
        "auth",
        "cred",
        "credential",
        "dsn",
        "private",
        "cert",
        "csrf",
        "apikey",
        "api_key",
        "api-key",
        "PAT",
    ];
    let before = ["", "my", "X-", "db_", "API_", "stripe", "ACCESS_"];
    let after = [
        "", "Id", "_id", "_value", "Header", "s", "_pem", "2", "_X", "Value", "_FILE",
    ];
    let mut checked = 0;
    for hint in hints {
        for prefix in before {
            for suffix in after {
                let name = format!("{prefix}{hint}{suffix}");
                for name in [name.clone(), name.to_uppercase(), name.to_lowercase()] {
                    let was_secret =
                        old::chain(&name) || old::dynamic(&name) || old::credentials(&name);
                    let intended = INTENDED_UNMASKED
                        .iter()
                        .any(|named| named.eq_ignore_ascii_case(&name));
                    if was_secret && !intended {
                        assert!(looks_secret(&name), "{name}");
                    }
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 1000);
}
