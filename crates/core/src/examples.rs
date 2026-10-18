//! Saved response examples, next to the request in `<file>.examples.json` (committed with it,
//! unlike history), keyed by the request's index in its file.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::http_file::Header;
use crate::send::Response;
use crate::workspace::{parse_request_id, resolve};
use crate::{Error, Result, fs_util, time};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ResponseExample {
    pub name: String,
    pub status: u16,
    pub headers: Vec<Header>,
    pub body: String,
    /// RFC 3339.
    pub saved_at: String,
}

/// Examples per request. They are the user's files, so a new one over the limit is refused
/// instead of an old one being dropped.
const KEEP: usize = 20;

type ExamplesFile = BTreeMap<String, Vec<ResponseExample>>;

pub fn list(root: &Path, id: &str) -> Result<Vec<ResponseExample>> {
    let (path, key) = location(root, id)?;
    Ok(read(&path)?.remove(&key).unwrap_or_default())
}

/// Keeps a response (already masked by `send`) as example `name`, replacing an example with the
/// same name.
pub fn create(root: &Path, id: &str, name: &str, response: &Response) -> Result<ResponseExample> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Invalid("an example needs a name".into()));
    }
    let example = ResponseExample {
        name: name.to_string(),
        status: response.status,
        // Only headers that describe the content; dates and CDN noise would churn diffs.
        headers: response
            .headers
            .iter()
            .filter(|h| h.name.eq_ignore_ascii_case("content-type"))
            .cloned()
            .collect(),
        body: response.body.clone(),
        saved_at: time::format(time::now()),
    };
    let (path, key) = location(root, id)?;
    let mut file = read(&path)?;
    let examples = file.entry(key).or_default();
    examples.retain(|e| e.name != example.name);
    if examples.len() >= KEEP {
        return Err(Error::Invalid(format!(
            "a request keeps at most {KEEP} examples; remove one first"
        )));
    }
    examples.push(example.clone());
    write(&path, &file)?;
    Ok(example)
}

pub fn remove(root: &Path, id: &str, name: &str) -> Result<()> {
    let (path, key) = location(root, id)?;
    let mut file = read(&path)?;
    let examples = file.entry(key).or_default();
    let before = examples.len();
    examples.retain(|e| e.name != name);
    if examples.len() == before {
        return Err(Error::NotFound(format!("example {name}")));
    }
    write(&path, &file)
}

/// Puts examples taken with [`take`] back under a request's new id, after a move.
pub(crate) fn put(root: &Path, id: &str, examples: Vec<ResponseExample>) -> Result<()> {
    if examples.is_empty() {
        return Ok(());
    }
    let (path, key) = location(root, id)?;
    let mut file = read(&path)?;
    file.insert(key, examples);
    write(&path, &file)
}

/// Drops a removed request's examples and shifts the keys of the requests after it, so
/// indexes in a multi-request file stay right.
pub(crate) fn removed(root: &Path, id: &str) -> Result<()> {
    let (path, key) = location(root, id)?;
    let Ok(index) = key.parse::<usize>() else {
        return Ok(());
    };
    let file = read(&path)?;
    let shifted: ExamplesFile = file
        .into_iter()
        .filter_map(|(k, v)| match k.parse::<usize>() {
            Ok(i) if i == index => None,
            Ok(i) if i > index => Some(((i - 1).to_string(), v)),
            _ => Some((k, v)),
        })
        .collect();
    write(&path, &shifted)
}

/// Removes and returns a request's examples, e.g. before it moves.
pub(crate) fn take(root: &Path, id: &str) -> Result<Vec<ResponseExample>> {
    let (path, key) = location(root, id)?;
    let mut file = read(&path)?;
    let examples = file.remove(&key).unwrap_or_default();
    if !examples.is_empty() {
        write(&path, &file)?;
    }
    Ok(examples)
}

// The examples file for a request and the key of the request in it.
fn location(root: &Path, id: &str) -> Result<(PathBuf, String)> {
    let (rel, index) = parse_request_id(id)?;
    let file = resolve(root, rel)?;
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().trim_end_matches(".http").to_string())
        .ok_or_else(|| Error::Invalid(format!("invalid request id {id}")))?;
    Ok((
        file.with_file_name(format!("{name}.examples.json")),
        index.to_string(),
    ))
}

fn read(path: &Path) -> Result<ExamplesFile> {
    if !path.exists() {
        return Ok(ExamplesFile::new());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

// Writes the file, or deletes it when no examples are left.
fn write(path: &Path, file: &ExamplesFile) -> Result<()> {
    let file: ExamplesFile = file
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if file.is_empty() {
        if path.exists() {
            fs::remove_file(path)?;
        }
        return Ok(());
    }
    fs_util::write_atomic(path, serde_json::to_string_pretty(&file)? + "\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::send::SentRequest;
    use crate::workspace::tests::{temp_workspace, write as write_file};

    fn response(body: &str) -> Response {
        Response {
            status: 201,
            status_text: "Created".into(),
            headers: vec![
                Header {
                    name: "Content-Type".into(),
                    value: "application/json".into(),
                },
                Header {
                    name: "Date".into(),
                    value: "today".into(),
                },
            ],
            body: body.into(),
            size: 0,
            binary: false,
            body_base64: String::new(),
            truncated: false,
            duration_ms: 1,
            sent: SentRequest {
                method: "POST".into(),
                url: "http://x.test".into(),
                headers: Vec::new(),
                body: String::new(),
            },
            assertions: Vec::new(),
            masked_secrets: Vec::new(),
        }
    }

    #[test]
    fn creates_lists_replaces_and_removes() {
        let root = temp_workspace("examples");
        write_file(&root, "users/create.http", "POST /users\n");

        create(
            &root,
            "users/create.http",
            "Created",
            &response("{\"id\":1}"),
        )
        .unwrap();
        create(
            &root,
            "users/create.http",
            "Created",
            &response("{\"id\":2}"),
        )
        .unwrap();
        create(&root, "users/create.http", "Invalid", &response("{}")).unwrap();

        let examples = list(&root, "users/create.http").unwrap();
        assert_eq!(examples.len(), 2);
        assert_eq!(examples[0].body, "{\"id\":2}");
        assert_eq!(
            examples[0].headers.len(),
            1,
            "only content headers are kept"
        );
        assert!(root.join("users/create.examples.json").is_file());

        remove(&root, "users/create.http", "Created").unwrap();
        remove(&root, "users/create.http", "Invalid").unwrap();
        assert!(!root.join("users/create.examples.json").exists());
    }

    #[test]
    fn examples_follow_moves_and_removals() {
        let root = temp_workspace("examples-move");
        write_file(
            &root,
            "all.http",
            "### One\nGET /1\n### Two\nGET /2\n### Three\nGET /3\n",
        );
        std::fs::create_dir(root.join("moved")).unwrap();
        create(&root, "all.http#2", "two", &response("2")).unwrap();
        create(&root, "all.http#3", "three", &response("3")).unwrap();

        let new_id = crate::requests::move_to(&root, "all.http#2", "moved", None, None)
            .unwrap()
            .id;

        assert_eq!(list(&root, &new_id).unwrap()[0].name, "two");
        // "Three" is now the second request in all.http and keeps its example.
        assert_eq!(list(&root, "all.http#2").unwrap()[0].name, "three");
    }

    #[test]
    fn rejects_empty_names() {
        let root = temp_workspace("examples-name");
        write_file(&root, "a.http", "GET /a\n");
        assert!(create(&root, "a.http", "  ", &response("")).is_err());
    }

    #[test]
    fn refuses_more_than_keep_examples_but_still_replaces() {
        let root = temp_workspace("examples-keep");
        write_file(&root, "a.http", "GET /a\n");
        for n in 0..KEEP {
            create(&root, "a.http", &format!("e{n}"), &response("")).unwrap();
        }
        assert!(create(&root, "a.http", "one more", &response("")).is_err());
        create(&root, "a.http", "e0", &response("new")).unwrap();
        assert_eq!(list(&root, "a.http").unwrap().len(), KEEP);
    }
}
