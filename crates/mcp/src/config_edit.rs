//! Reading and changing our entry in an AI app's own MCP config file: JSON for most apps,
//! TOML for Codex.

use std::fs;
use std::path::{Path, PathBuf};

use plumbq_core::{Error, Result, fs_util};
use serde_json::{Map, Value, json};

use crate::apps::{McpApp, McpTarget, NAME};

pub(crate) enum Config<'a> {
    Json(&'a mut Value),
    Toml(&'a mut toml_edit::DocumentMut),
}

/// Reads the config (or starts an empty one), applies `change`, backs the original up, and
/// writes it back. A config that doesn't parse is left alone.
pub(crate) fn edit(
    app: McpApp,
    path: &Path,
    change: impl FnOnce(Config<'_>) -> Result<()>,
) -> Result<()> {
    let original = match fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => return Err(err.into()),
    };
    let text = original.clone().unwrap_or_default();
    let updated = if app == McpApp::Codex {
        let mut doc: toml_edit::DocumentMut = text
            .parse()
            .map_err(|err| Error::Invalid(format!("couldn't read {}: {err}", path.display())))?;
        change(Config::Toml(&mut doc))?;
        doc.to_string()
    } else {
        let mut value: Value = if text.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(&text)?
        };
        change(Config::Json(&mut value))?;
        format!("{}\n", serde_json::to_string_pretty(&value)?)
    };
    let backup = path.with_extension(format!(
        "{}.plumbq-backup",
        path.extension().and_then(|e| e.to_str()).unwrap_or("bak")
    ));
    if let Some(original) = &original {
        fs_util::write_private(&backup, original)?;
    }
    fs_util::write_keep_mode(&real_path(path), updated)?;
    Ok(())
}

/// Where a config really is: users who keep dotfiles in a repo link them, and replacing the
/// link with a file would cut that tie.
fn real_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// One of our entries in an app's config.
pub(crate) struct Entry {
    /// The whole entry as text; the server's address is somewhere in it.
    pub text: String,
    /// The program the app starts (Claude Desktop and Codex run the bridge).
    pub command: Option<String>,
}

/// Our entries in the app's config. A missing config means none; a config that can't be read
/// or parsed is an error.
pub(crate) fn entries(app: McpApp, path: &Path) -> std::result::Result<Vec<Entry>, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(format!("failed to read {}: {err}", path.display())),
    };
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let invalid = |err: String| format!("failed to read {}: {err}", path.display());
    if app == McpApp::Codex {
        let doc = text
            .parse::<toml_edit::DocumentMut>()
            .map_err(|err| invalid(err.to_string()))?;
        let entry = doc.get("mcp_servers").and_then(|servers| servers.get(NAME));
        return Ok(entry
            .map(|entry| Entry {
                text: entry.to_string(),
                command: entry
                    .get("command")
                    .and_then(toml_edit::Item::as_str)
                    .map(str::to_string),
            })
            .into_iter()
            .collect());
    }
    let value = serde_json::from_str::<Value>(&text).map_err(|err| invalid(err.to_string()))?;
    let mut entries: Vec<&Value> = value
        .get(app.json_section())
        .and_then(Value::as_object)
        .and_then(|servers| servers.get(NAME))
        .into_iter()
        .collect();
    if app == McpApp::ClaudeCode {
        entries.extend(
            project_servers(&value)
                .into_iter()
                .filter_map(|servers| servers.get(NAME))
                .filter(|entry| is_ours(entry)),
        );
    }
    Ok(entries
        .into_iter()
        .map(|entry| Entry {
            text: entry.to_string(),
            command: entry
                .get("command")
                .and_then(Value::as_str)
                .map(str::to_string),
        })
        .collect())
}

/// Adds (or updates) our entry for `target`. Claude Code entries of ours in a project's scope
/// get the new address too, so no project keeps using an old one.
pub(crate) fn insert(
    app: McpApp,
    path: &Path,
    config: Config<'_>,
    target: &McpTarget<'_>,
) -> Result<()> {
    match config {
        Config::Json(value) => {
            let entry = app.json_entry(target);
            if app == McpApp::ClaudeCode {
                for servers in project_servers_mut(value) {
                    if let Some(ours) = servers.get_mut(NAME).filter(|entry| is_ours(entry)) {
                        *ours = entry.clone();
                    }
                }
            }
            top_servers(app, path, value)?.insert(NAME.to_string(), entry);
            Ok(())
        }
        Config::Toml(doc) => insert_toml(doc, target),
    }
}

/// Removes our entry, for Claude Code from every project's scope too (only entries of ours, not
/// a hand-made `plumbq` server such as `--workspace`); everything else stays.
pub(crate) fn remove(app: McpApp, config: Config<'_>) {
    match config {
        Config::Json(value) => {
            if app == McpApp::ClaudeCode {
                for servers in project_servers_mut(value) {
                    if servers.get(NAME).is_some_and(is_ours) {
                        servers.shift_remove(NAME);
                    }
                }
            }
            if let Some(servers) = value
                .get_mut(app.json_section())
                .and_then(Value::as_object_mut)
            {
                servers.shift_remove(NAME);
            }
        }
        Config::Toml(doc) => {
            if let Some(servers) = doc
                .get_mut("mcp_servers")
                .and_then(toml_edit::Item::as_table_mut)
            {
                servers.remove(NAME);
            }
        }
    }
}

/// The app's own server list, made if missing.
fn top_servers<'a>(
    app: McpApp,
    path: &Path,
    value: &'a mut Value,
) -> Result<&'a mut Map<String, Value>> {
    value
        .as_object_mut()
        .ok_or_else(|| Error::Invalid(format!("{} isn't a JSON object", path.display())))?
        .entry(app.json_section())
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| {
            Error::Invalid(format!(
                "{} in {} isn't an object",
                app.json_section(),
                path.display()
            ))
        })
}

/// Whether a server entry is one this app wrote: the in-app server's address, or the bundled
/// binary run with `--url`. A `plumbq-mcp --workspace` entry someone made by hand is not.
fn is_ours(entry: &Value) -> bool {
    if let Some(url) = entry.get("url").and_then(Value::as_str) {
        return is_server_url(url);
    }
    let bundled = entry
        .get("command")
        .and_then(Value::as_str)
        .and_then(|command| Path::new(command).file_name()?.to_str())
        .is_some_and(|name| name.starts_with("plumbq-mcp"));
    let has_url_flag = entry
        .get("args")
        .and_then(Value::as_array)
        .is_some_and(|args| args.iter().any(|arg| arg.as_str() == Some("--url")));
    bundled && has_url_flag
}

/// `http://127.0.0.1:<port>/mcp/<token>`.
fn is_server_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("http://127.0.0.1:") else {
        return false;
    };
    let Some((port, path)) = rest.split_once("/mcp/") else {
        return false;
    };
    !port.is_empty()
        && port.chars().all(|c| c.is_ascii_digit())
        && !path.is_empty()
        && !path.contains('/')
}

/// Claude Code's servers per project: `claude mcp add` puts a server there unless it gets
/// `--scope user`.
fn project_servers(value: &Value) -> Vec<&Map<String, Value>> {
    let Some(projects) = value.get("projects").and_then(Value::as_object) else {
        return Vec::new();
    };
    projects
        .values()
        .filter_map(|project| project.get("mcpServers")?.as_object())
        .collect()
}

fn project_servers_mut(value: &mut Value) -> Vec<&mut Map<String, Value>> {
    let Some(projects) = value.get_mut("projects").and_then(Value::as_object_mut) else {
        return Vec::new();
    };
    projects
        .values_mut()
        .filter_map(|project| project.get_mut("mcpServers")?.as_object_mut())
        .collect()
}

fn insert_toml(doc: &mut toml_edit::DocumentMut, target: &McpTarget<'_>) -> Result<()> {
    let mut args = toml_edit::Array::new();
    for arg in target.bridge_args() {
        args.push(arg);
    }
    let mut server = toml_edit::Table::new();
    server.insert(
        "command",
        toml_edit::value(target.binary.to_string_lossy().into_owned()),
    );
    server.insert("args", toml_edit::value(args));
    let servers = doc
        .entry("mcp_servers")
        .or_insert(toml_edit::Item::Table({
            let mut t = toml_edit::Table::new();
            t.set_implicit(true);
            t
        }))
        .as_table_mut()
        .ok_or_else(|| Error::Invalid("mcp_servers in config.toml isn't a table".into()))?;
    servers.insert(NAME, toml_edit::Item::Table(server));
    Ok(())
}
