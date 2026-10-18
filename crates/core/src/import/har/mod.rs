//! HAR recordings ("Save all as HAR" in browser dev tools): API calls become requests, grouped
//! by host. Static assets, page loads, duplicates and browser-managed headers are left out.

mod entry;
mod hosts;

use std::collections::BTreeMap;

use serde_json::Value;

use super::{ImportFormat, ImportedCollection, ImportedEnvironment, ImportedItem, text};
use crate::http_file::Request;
use entry::{has_cookie, to_request};
use hosts::{host_label, origin_of, variable_names, with_variable};

const ASSET_EXTENSIONS: [&str; 15] = [
    "png", "jpg", "jpeg", "gif", "svg", "webp", "ico", "avif", "css", "js", "mjs", "map", "woff",
    "woff2", "ttf",
];
const ASSET_TYPES: [&str; 6] = ["image", "stylesheet", "script", "font", "media", "document"];

pub fn detect(doc: &Value) -> bool {
    doc.pointer("/log/entries").is_some_and(Value::is_array)
}

pub fn parse(doc: &Value) -> ImportedCollection {
    let entries = doc
        .pointer("/log/entries")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let recording = group_by_host(entries);
    let notes = notes(&recording);
    let names = variable_names(&recording.hosts);
    let variables: BTreeMap<String, String> = recording
        .hosts
        .iter()
        .zip(&names)
        .filter(|((origin, _), _)| !origin.is_empty())
        .map(|((origin, _), name)| (name.clone(), origin.clone()))
        .collect();
    let (name, items) = items(recording.hosts, &names);
    let environments = match variables.is_empty() {
        true => Vec::new(),
        false => vec![ImportedEnvironment {
            name: name.clone(),
            variables,
            secrets: Vec::new(),
        }],
    };
    ImportedCollection {
        format: ImportFormat::Har,
        notes,
        name,
        headers: Default::default(),
        items,
        environments,
        credentials: Vec::new(),
    }
}

/// The API calls of a recording per origin, and what was left out.
struct Recording {
    hosts: Vec<(String, Vec<Request>)>,
    assets: usize,
    duplicates: usize,
    cookies: bool,
}

fn group_by_host(entries: &[Value]) -> Recording {
    let mut out = Recording {
        hosts: Vec::new(),
        assets: 0,
        duplicates: 0,
        cookies: false,
    };
    let mut seen: Vec<String> = Vec::new();
    for entry in entries {
        if is_asset(entry) {
            out.assets += 1;
            continue;
        }
        let Some(request) = entry.get("request") else {
            continue;
        };
        let method = text(request, "method").to_uppercase();
        let url = text(request, "url");
        let key = format!("{method} {url}");
        if seen.contains(&key) {
            out.duplicates += 1;
            continue;
        }
        seen.push(key);
        out.cookies |= has_cookie(request);

        let origin = origin_of(&url);
        let request = to_request(request, &method, &url);
        match out.hosts.iter_mut().find(|(o, _)| *o == origin) {
            Some((_, requests)) => requests.push(request),
            None => out.hosts.push((origin, vec![request])),
        }
    }
    out
}

fn notes(recording: &Recording) -> Vec<String> {
    let mut notes = Vec::new();
    if recording.assets > 0 {
        notes.push(format!(
            "skipped {} page loads and static assets",
            recording.assets
        ));
    }
    if recording.duplicates > 0 {
        notes.push(format!(
            "skipped {} repeated requests",
            recording.duplicates
        ));
    }
    if recording.cookies {
        notes.push("left out Cookie headers, which usually hold your session".into());
    }
    notes
}

/// One host: its requests directly; several: a folder per host. Returns the collection name
/// and its items.
fn items(hosts: Vec<(String, Vec<Request>)>, names: &[String]) -> (String, Vec<ImportedItem>) {
    let groups: Vec<(String, Vec<ImportedItem>)> = hosts
        .into_iter()
        .zip(names)
        .map(|((origin, requests), variable)| {
            let items = requests
                .into_iter()
                .map(|r| ImportedItem::Request(with_variable(r, &origin, variable)))
                .collect();
            (host_label(&origin), items)
        })
        .collect();
    if groups.len() == 1 {
        return groups.into_iter().next().unwrap_or_default();
    }
    let folders = groups
        .into_iter()
        .map(|(name, items)| ImportedItem::Folder {
            name,
            headers: Default::default(),
            items,
        })
        .collect();
    ("Recorded requests".to_string(), folders)
}

fn is_asset(entry: &Value) -> bool {
    if ASSET_TYPES.contains(&text(entry, "_resourceType").as_str()) {
        return true;
    }
    let mime = entry
        .pointer("/response/content/mimeType")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let static_mime = [
        "image/",
        "font/",
        "text/css",
        "javascript",
        "text/html",
        "video/",
        "audio/",
    ]
    .iter()
    .any(|m| mime.contains(m));
    let url = entry
        .pointer("/request/url")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let path = url.split(['?', '#']).next().unwrap_or_default();
    let extension = path.rsplit_once('.').map(|(_, ext)| ext.to_lowercase());
    static_mime || extension.is_some_and(|ext| ASSET_EXTENSIONS.contains(&ext.as_str()))
}

#[cfg(test)]
mod tests;
