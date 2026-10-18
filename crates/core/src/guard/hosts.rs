use std::collections::{BTreeMap, BTreeSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::Path;

use reqwest::Url;

use crate::environments::{self, EnvironmentTag, Values};
use crate::http_file::Request;
use crate::{Result, file_vars, send, variables};

/// The lowercase host the request would go to, or `None` when it can't be known without
/// resolving secrets or dynamic values (e.g. the host itself comes from a secret).
pub fn destination_host(
    root: &Path,
    id: &str,
    request: &Request,
    environment: Option<&str>,
) -> Result<Option<String>> {
    let values = plain_values(root, id, request, environment)?;
    let mut url = resolve_plain(&request.url, &values)?;
    if !url.contains("://") {
        url = format!("http://{url}");
    }
    Ok(url_host(&url))
}

/// The environment's plain variables and the file's variables. Secrets and dynamic values are
/// left out, so nothing secret is read.
pub(super) fn plain_values(
    root: &Path,
    id: &str,
    request: &Request,
    environment: Option<&str>,
) -> Result<Values> {
    let mut values = Values {
        variables: environments::plain_variables(root, environment)?,
        ..Values::default()
    };
    let text = send::request_text(root, id, request)?;
    // A file variable that can't be resolved only matters where it is used; it then stays a
    // placeholder in `resolve_plain`.
    let _ = file_vars::resolve_into(root, id, request, &text, &BTreeMap::new(), &mut values);
    Ok(values)
}

/// `text` with the variables of [`plain_values`]; any other variable stays a `{{placeholder}}`.
pub(super) fn resolve_plain(text: &str, values: &Values) -> Result<String> {
    let mut values = values.clone();
    for name in variables::used(text) {
        if !values.variables.contains_key(&name) && !values.secrets.contains_key(&name) {
            values
                .secrets
                .insert(name.clone(), format!("{{{{{name}}}}}"));
        }
    }
    variables::substitute(text, &values)
}

/// Hosts of URL-looking variables of `environment` plus the workspace defaults: where secrets
/// may be sent by an agent.
pub fn allowed_hosts(root: &Path, environment: Option<&str>) -> Result<BTreeSet<String>> {
    let values = environments::plain_variables(root, environment)?;
    Ok(values.values().filter_map(|v| url_host(v)).collect())
}

/// Hosts of URL-looking variables of every environment and the workspace defaults.
pub(super) fn known_hosts(root: &Path) -> Result<BTreeSet<String>> {
    let mut hosts = allowed_hosts(root, None)?;
    for env in environments::names(root)? {
        hosts.extend(allowed_hosts(root, Some(&env))?);
    }
    Ok(hosts)
}

/// This machine, a private network, or an address that isn't clearly public: any IP that isn't
/// a public unicast address (IPv4-mapped too), `localhost`, `*.localhost`, `*.internal` and
/// `*.local`.
pub fn is_private_host(host: &str) -> bool {
    let host = normal_host(host);
    match host.parse() {
        Ok(ip) => is_private_ip(ip),
        Err(_) => {
            host == "localhost"
                || [".localhost", ".internal", ".local"]
                    .iter()
                    .any(|end| host.ends_with(end))
        }
    }
}

/// Names nobody can register (RFC 2606 and 6761): `*.test`, `*.example` and `*.invalid`. Only
/// the user's own DNS or hosts file can lead them anywhere.
pub(super) fn is_reserved_name(host: &str) -> bool {
    [".test", ".example", ".invalid"]
        .iter()
        .any(|end| host.ends_with(end))
}

pub(super) fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => !public_v4(ip),
        IpAddr::V6(ip) => match ip.to_ipv4_mapped() {
            Some(v4) => !public_v4(v4),
            None => !public_v6(ip),
        },
    }
}

/// The one form hosts are compared in: lowercase, without IPv6 brackets or a trailing dot.
pub fn normal_host(host: &str) -> String {
    host.trim_start_matches('[')
        .trim_end_matches(']')
        .trim_end_matches('.')
        .to_ascii_lowercase()
}

fn public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    let shared = a == 100 && (64..128).contains(&b);
    let benchmarking = a == 198 && (b == 18 || b == 19);
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_multicast()
        || a == 0
        || a >= 240
        || shared
        || benchmarking)
}

/// Global unicast (`2000::/3`) outside the documentation range.
fn public_v6(ip: Ipv6Addr) -> bool {
    let [first, second, ..] = ip.segments();
    first & 0xe000 == 0x2000 && !(first == 0x2001 && second == 0x0db8)
}

/// Hosts of URL-looking variables in production environment files.
pub(super) fn production_hosts(root: &Path) -> Result<BTreeSet<String>> {
    let mut hosts = BTreeSet::new();
    for env in environments::names(root)? {
        if environments::tag(root, &env)? != EnvironmentTag::Production {
            continue;
        }
        let file_only = environments::own_variables(root, &env)?;
        hosts.extend(file_only.values().filter_map(|v| url_host(v)));
    }
    Ok(hosts)
}

/// The production environment whose URLs point at `host`, if any.
pub(super) fn production_owner(root: &Path, host: &str) -> Result<Option<String>> {
    for env in environments::names(root)? {
        if environments::tag(root, &env)? != EnvironmentTag::Production {
            continue;
        }
        let file_only = environments::own_variables(root, &env)?;
        if file_only
            .values()
            .filter_map(|v| url_host(v))
            .any(|h| h == host)
        {
            return Ok(Some(env));
        }
    }
    Ok(None)
}

/// The host of an `http(s)` URL in [`normal_host`] form; `None` for anything else. Read with
/// reqwest's own parser, so it is the host a send really goes to (`http://a.test@b.test/` goes
/// to `b.test`).
pub fn url_host(value: &str) -> Option<String> {
    let url = Url::parse(value.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = normal_host(url.host_str()?);
    // `https://{{host}}/` parses with a literal `{{host}}` host: unknown, not a real host.
    if host.contains(['{', '}']) {
        return None;
    }
    Some(host)
}
