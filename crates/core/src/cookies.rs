//! A cookie jar like a browser's (RFC 6265 basics), one per workspace and environment, kept on
//! this machine: `<cookie dir>/<workspace>/<environment or _none>.json`.

use std::fs;
use std::path::{Path, PathBuf};

use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Result, fs_util, history, time};

/// Jar file name for "no environment".
const NO_ENVIRONMENT: &str = "_none";
/// Cookies per jar; when a new one arrives over the limit, the oldest is dropped (browsers keep
/// a few hundred per site too).
const KEEP: usize = 300;
const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Cookie {
    pub name: String,
    pub value: String,
    /// Lowercase, without a leading dot.
    pub domain: String,
    /// Sent only to exactly `domain`, not its subdomains (no `Domain` attribute was given).
    pub host_only: bool,
    pub path: String,
    /// RFC 3339; `None` for a session cookie, kept until the jar is cleared.
    pub expires: Option<String>,
    pub secure: bool,
    pub http_only: bool,
}

/// Parses one `Set-Cookie` value received from `url`. Returns `None` for malformed lines and for
/// cookies the server may not set (a `Domain` that doesn't cover the host). An already expired
/// cookie is returned too, so storing it deletes the old one.
pub fn parse(set_cookie: &str, url: &Url, now: i64) -> Option<Cookie> {
    let host = url.host_str()?.to_lowercase();
    let mut parts = set_cookie.split(';');
    let (name, value) = parts.next()?.split_once('=')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let mut cookie = Cookie {
        name: name.to_string(),
        value: value.trim().trim_matches('"').to_string(),
        domain: host.clone(),
        host_only: true,
        path: default_path(url.path()),
        expires: None,
        secure: false,
        http_only: false,
    };
    let mut max_age = None;
    for attribute in parts {
        let (key, val) = attribute.split_once('=').unwrap_or((attribute, ""));
        let val = val.trim();
        match key.trim().to_lowercase().as_str() {
            "domain" if !val.is_empty() => {
                let domain = val.trim_start_matches('.').to_lowercase();
                if !domain_matches(&host, &domain) || is_public_suffix(&domain, &host) {
                    return None;
                }
                cookie.domain = domain;
                cookie.host_only = false;
            }
            "path" if val.starts_with('/') => cookie.path = val.to_string(),
            "expires" => cookie.expires = parse_http_date(val).map(expiry_text),
            "max-age" => max_age = val.parse::<i64>().ok(),
            "secure" => cookie.secure = true,
            "httponly" => cookie.http_only = true,
            _ => {}
        }
    }
    // Max-Age wins over Expires; zero or negative means "delete now".
    if let Some(secs) = max_age {
        cookie.expires = Some(expiry_text(now.saturating_add(secs.max(0))));
    }
    Some(cookie)
}

/// The `Cookie` header value for `url`, most specific path first, or `None` when nothing applies.
pub fn header_for(jar: &[Cookie], url: &Url, now: i64) -> Option<String> {
    let host = url.host_str()?.to_lowercase();
    let https = url.scheme() == "https";
    let mut matching: Vec<&Cookie> = jar
        .iter()
        .filter(|c| !is_expired(c, now) && (https || !c.secure))
        .filter(|c| {
            if c.host_only {
                c.domain == host
            } else {
                domain_matches(&host, &c.domain)
            }
        })
        .filter(|c| path_matches(url.path(), &c.path))
        .collect();
    if matching.is_empty() {
        return None;
    }
    matching.sort_by_key(|c| std::cmp::Reverse(c.path.len()));
    let pairs: Vec<String> = matching
        .iter()
        .map(|c| format!("{}={}", c.name, c.value))
        .collect();
    Some(pairs.join("; "))
}

/// Adds or replaces a cookie (same name, domain and path); an expired one removes it. The jar
/// keeps at most [`KEEP`] cookies, dropping the oldest.
pub fn store(jar: &mut Vec<Cookie>, cookie: Cookie, now: i64) {
    jar.retain(|c| !(c.name == cookie.name && c.domain == cookie.domain && c.path == cookie.path));
    if !is_expired(&cookie, now) {
        jar.push(cookie);
    }
    let over = jar.len().saturating_sub(KEEP);
    jar.drain(..over);
}

/// A session cookie never expires; an expiry that can't be read counts as past.
pub fn is_expired(cookie: &Cookie, now: i64) -> bool {
    cookie
        .expires
        .as_deref()
        .is_some_and(|at| time::parse(at).is_none_or(|at| at <= now))
}

/// The jar for a workspace and environment, without expired cookies. A jar that can't be read
/// (e.g. cut off by a crash) counts as empty rather than breaking every send.
pub fn list(cookie_dir: &Path, root: &Path, environment: Option<&str>) -> Result<Vec<Cookie>> {
    let path = jar_path(cookie_dir, root, environment);
    let legacy = legacy_jar_path(cookie_dir, root, environment);
    let path = match (path.is_file(), legacy.is_file()) {
        (false, true) => legacy,
        (false, false) => return Ok(Vec::new()),
        _ => path,
    };
    let jar = match read_jar(&fs::read_to_string(&path)?) {
        Ok(jar) => jar,
        Err(err) => {
            tracing::warn!("ignoring unreadable cookie jar {}: {err}", path.display());
            Vec::new()
        }
    };
    let now = time::now().0;
    Ok(jar.into_iter().filter(|c| !is_expired(c, now)).collect())
}

/// Jars from older versions stored `expires` as unix seconds.
fn read_jar(text: &str) -> serde_json::Result<Vec<Cookie>> {
    let mut jar: Value = serde_json::from_str(text)?;
    for cookie in jar.as_array_mut().into_iter().flatten() {
        if let Some(expires) = cookie.get_mut("expires")
            && let Some(secs) = expires.as_i64()
        {
            *expires = Value::String(expiry_text(secs));
        }
    }
    serde_json::from_value(jar)
}

pub fn save(
    cookie_dir: &Path,
    root: &Path,
    environment: Option<&str>,
    jar: &[Cookie],
) -> Result<()> {
    let path = jar_path(cookie_dir, root, environment);
    fs_util::write_private(&path, serde_json::to_string_pretty(jar)?)?;
    let legacy = legacy_jar_path(cookie_dir, root, environment);
    if legacy.is_file() {
        fs::remove_file(legacy)?;
    }
    Ok(())
}

pub fn clear(cookie_dir: &Path, root: &Path, environment: Option<&str>) -> Result<()> {
    for path in [
        jar_path(cookie_dir, root, environment),
        legacy_jar_path(cookie_dir, root, environment),
    ] {
        if path.is_file() {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

/// Removes every cookie called `name` for `domain` (any path).
pub fn remove(
    cookie_dir: &Path,
    root: &Path,
    environment: Option<&str>,
    domain: &str,
    name: &str,
) -> Result<()> {
    let mut jar = list(cookie_dir, root, environment)?;
    jar.retain(|c| !(c.domain == domain && c.name == name));
    save(cookie_dir, root, environment, &jar)
}

fn jar_path(cookie_dir: &Path, root: &Path, environment: Option<&str>) -> PathBuf {
    let file = environment.map_or(NO_ENVIRONMENT.to_string(), history::dir_name);
    cookie_dir
        .join(history::dir_name(&root.display().to_string()))
        .join(format!("{file}.json"))
}

// Where older versions kept the jar; read until the first save moves it to [`jar_path`].
fn legacy_jar_path(cookie_dir: &Path, root: &Path, environment: Option<&str>) -> PathBuf {
    let file = environment.map_or(NO_ENVIRONMENT.to_string(), history::flatten);
    cookie_dir
        .join(history::flatten(&root.display().to_string()))
        .join(format!("{file}.json"))
}

// A `Domain` that is a public suffix (`com`, `co.uk`, `github.io`) would send the cookie to
// every site under it; only allowed when it is the host itself (then it acts host-only).
fn is_public_suffix(domain: &str, host: &str) -> bool {
    if domain == host {
        return false;
    }
    !domain.contains('.') || psl::suffix_str(domain) == Some(domain)
}

// `host` is `domain` or one of its subdomains. IP addresses only match exactly.
fn domain_matches(host: &str, domain: &str) -> bool {
    if host == domain {
        return true;
    }
    let is_ip = host.parse::<std::net::IpAddr>().is_ok();
    !is_ip && host.ends_with(domain) && host[..host.len() - domain.len()].ends_with('.')
}

// RFC 6265 path-match: equal, or a prefix ending at a `/`.
fn path_matches(request_path: &str, cookie_path: &str) -> bool {
    request_path == cookie_path
        || (request_path.starts_with(cookie_path)
            && (cookie_path.ends_with('/') || request_path[cookie_path.len()..].starts_with('/')))
}

// The directory of the request path, e.g. `/api/users/1` → `/api/users`.
fn default_path(path: &str) -> String {
    match path.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(i) => path[..i].to_string(),
    }
}

// Parses HTTP dates like `Wed, 21 Oct 2015 07:28:00 GMT` (and the dashed variant
// `Wed, 21-Oct-2015 07:28:00 GMT`). Returns unix seconds.
fn parse_http_date(text: &str) -> Option<i64> {
    let (mut day, mut month, mut year, mut clock) = (None, None, None, None);
    for token in text.split([' ', '-', ',']).filter(|t| !t.is_empty()) {
        let lower = token.to_lowercase();
        if token.contains(':') {
            clock = Some(token);
        } else if let Some(m) = MONTHS.iter().position(|m| lower.starts_with(m)) {
            month = Some(m as i64 + 1);
        } else if let Ok(n) = token.parse::<i64>() {
            match (day, n) {
                (None, 1..=31) => day = Some(n),
                (_, 70..=99) => year = Some(1900 + n),
                (_, 0..=69) => year = Some(2000 + n),
                _ => year = Some(n),
            }
        }
    }
    let mut hms = clock?.split(':').map(|p| p.parse::<i64>().ok());
    let (h, m, s) = (hms.next()??, hms.next()??, hms.next()??);
    let (year, month, day) = (year?, month?, day?);
    // Out-of-range values (a year like 99999999999999) would overflow the arithmetic.
    let valid = (1601..=9999).contains(&year) && h < 24 && m < 60 && s < 61;
    valid.then(|| time::from_civil(year, month, day, h, m, s))
}

/// Kept between 1970 and 2106 (the `u32` range jars used before), so a huge `Max-Age` can't
/// overflow the date math.
fn expiry_text(secs: i64) -> String {
    time::format((secs.clamp(0, i64::from(u32::MAX)), 0))
}

#[cfg(test)]
#[path = "cookies_tests.rs"]
mod tests;
