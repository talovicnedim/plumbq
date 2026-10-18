//! Per-request HTTP settings as JetBrains directives (`# @timeout 60`, …). Syntax:
//! `docs/formats.md` (Request settings).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
/// Missing fields mean "off" / "unset", so the UI can send only what it changes.
#[serde(rename_all = "camelCase", default)]
pub struct RequestSettings {
    /// Don't follow redirects; the 3xx response is returned as is.
    pub no_redirect: bool,
    /// Neither send nor store cookies from the cookie jar.
    pub no_cookie_jar: bool,
    /// Accept invalid or self-signed TLS certificates.
    pub insecure: bool,
    /// Whole-request timeout; the workspace default (or 30 s) when unset.
    pub timeout_secs: Option<u32>,
    pub connection_timeout_secs: Option<u32>,
}

impl RequestSettings {
    /// Applies a directive (the comment text after `#`, e.g. `@timeout 60`). Returns false when
    /// it isn't a settings directive, so the caller keeps it as a comment.
    pub(crate) fn read(&mut self, directive: &str) -> bool {
        let (name, arg) = directive
            .split_once(char::is_whitespace)
            .map_or((directive, ""), |(n, a)| (n, a.trim()));
        match name {
            "@no-redirect" => self.no_redirect = true,
            "@no-cookie-jar" => self.no_cookie_jar = true,
            "@insecure" => self.insecure = true,
            "@timeout" => match parse_seconds(arg) {
                Some(secs) => self.timeout_secs = Some(secs),
                None => return false,
            },
            "@connection-timeout" => match parse_seconds(arg) {
                Some(secs) => self.connection_timeout_secs = Some(secs),
                None => return false,
            },
            _ => return false,
        }
        true
    }

    /// The directive lines for this request, in a fixed order.
    pub(crate) fn lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if self.no_redirect {
            lines.push("# @no-redirect".to_string());
        }
        if self.no_cookie_jar {
            lines.push("# @no-cookie-jar".to_string());
        }
        if self.insecure {
            lines.push("# @insecure".to_string());
        }
        if let Some(secs) = self.timeout_secs {
            lines.push(format!("# @timeout {secs}"));
        }
        if let Some(secs) = self.connection_timeout_secs {
            lines.push(format!("# @connection-timeout {secs}"));
        }
        lines
    }
}

impl RequestSettings {
    /// Which setting a directive sets (`@timeout 500 ms` → `@timeout`), if it is one.
    pub(crate) fn key_of(directive: &str) -> Option<&'static str> {
        let mut probe = RequestSettings::default();
        if !probe.read(directive) {
            return None;
        }
        let name = directive.split_whitespace().next().unwrap_or_default();
        KEYS.iter().copied().find(|key| *key == name)
    }

    /// Whether a directive line says what these settings say for its key, so it can be kept
    /// exactly as written.
    pub(crate) fn agrees(&self, directive: &str) -> bool {
        let mut probe = RequestSettings::default();
        if !probe.read(directive) {
            return false;
        }
        match Self::key_of(directive) {
            Some("@no-redirect") => self.no_redirect == probe.no_redirect,
            Some("@no-cookie-jar") => self.no_cookie_jar == probe.no_cookie_jar,
            Some("@insecure") => self.insecure == probe.insecure,
            Some("@timeout") => self.timeout_secs == probe.timeout_secs,
            Some("@connection-timeout") => {
                self.connection_timeout_secs == probe.connection_timeout_secs
            }
            _ => false,
        }
    }

    /// The directive line for one setting, or `None` when it is off.
    pub(crate) fn line_for(&self, key: &str) -> Option<String> {
        self.lines().into_iter().find(|line| {
            line.strip_prefix("# ")
                .and_then(|d| d.split_whitespace().next())
                == Some(key)
        })
    }
}

const KEYS: [&str; 5] = [
    "@no-redirect",
    "@no-cookie-jar",
    "@insecure",
    "@timeout",
    "@connection-timeout",
];

/// `60`, `60 s`, `500ms` (rounded up to a second), `2 m`.
fn parse_seconds(text: &str) -> Option<u32> {
    let digits_end = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    let value: u32 = text[..digits_end].parse().ok()?;
    match text[digits_end..].trim() {
        "" | "s" | "sec" => Some(value),
        "ms" => Some(value.div_ceil(1000)),
        "m" | "min" => value.checked_mul(60),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_jetbrains_directives() {
        let mut settings = RequestSettings::default();
        for directive in [
            "@no-redirect",
            "@no-cookie-jar",
            "@insecure",
            "@timeout 2 m",
            "@connection-timeout 500ms",
        ] {
            assert!(settings.read(directive), "{directive}");
        }
        assert_eq!(
            settings,
            RequestSettings {
                no_redirect: true,
                no_cookie_jar: true,
                insecure: true,
                timeout_secs: Some(120),
                connection_timeout_secs: Some(1),
            }
        );
    }

    #[test]
    fn leaves_other_directives_alone() {
        let mut settings = RequestSettings::default();
        assert!(!settings.read("@no-log"));
        assert!(!settings.read("@timeout soon"));
        assert_eq!(settings, RequestSettings::default());
    }

    #[test]
    fn knows_keys_and_whether_a_line_still_agrees() {
        assert_eq!(RequestSettings::key_of("@timeout 500 ms"), Some("@timeout"));
        assert_eq!(RequestSettings::key_of("@no-log"), None);
        let settings = RequestSettings {
            timeout_secs: Some(1),
            ..Default::default()
        };
        assert!(settings.agrees("@timeout 500 ms"));
        assert!(!settings.agrees("@timeout 2"));
        assert_eq!(
            settings.line_for("@timeout").as_deref(),
            Some("# @timeout 1")
        );
        assert_eq!(settings.line_for("@insecure"), None);
    }

    #[test]
    fn writes_directives_back() {
        let settings = RequestSettings {
            no_redirect: true,
            timeout_secs: Some(60),
            ..Default::default()
        };
        assert_eq!(settings.lines(), ["# @no-redirect", "# @timeout 60"]);
    }
}
