use crate::http_file::Request;
use crate::import::camel;
use crate::import::write::SOURCE;

/// `baseUrl` for a single origin; `apiExampleComUrl`, `cdnTestUrl`, … when there are several,
/// so each folder's requests point at their own host.
pub(super) fn variable_names(hosts: &[(String, Vec<Request>)]) -> Vec<String> {
    if hosts.len() == 1 {
        return vec!["baseUrl".to_string()];
    }
    let mut names: Vec<String> = Vec::new();
    for (origin, _) in hosts {
        let base = format!("{}Url", camel(&host_label(origin)));
        let mut name = base.clone();
        let mut n = 2;
        while names.contains(&name) {
            name = format!("{base}{n}");
            n += 1;
        }
        names.push(name);
    }
    names
}

/// Replaces the origin with `{{variable}}`, keeping the recorded URL as the re-import marker.
pub(super) fn with_variable(mut request: Request, origin: &str, variable: &str) -> Request {
    let recorded = request
        .url
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string();
    request
        .comments
        .push(format!("{SOURCE}{} {recorded}", request.method));
    if !origin.is_empty()
        && let Some(rest) = request.url.strip_prefix(origin)
    {
        request.url = format!("{{{{{variable}}}}}{rest}");
    }
    request
}

/// `https://api.test:8080/users?x` → `https://api.test:8080`; empty for URLs without a scheme.
pub(super) fn origin_of(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return String::new();
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    format!("{scheme}://{host}")
}

/// `https://api.test:8080` → `api.test:8080`, used for folder and environment names.
pub(super) fn host_label(origin: &str) -> String {
    origin
        .split_once("://")
        .map_or(origin, |(_, host)| host)
        .to_string()
}
