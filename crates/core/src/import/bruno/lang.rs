//! The Bru markup of Bruno collections: dictionary, text and list blocks. See
//! `docs/formats.md` (Bru markup).

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// Enabled entries only; disabled (`~`) ones are dropped.
    Dict(Vec<(String, String)>),
    Text(String),
    List(Vec<String>),
}

/// Blocks in file order by name, e.g. `("headers", Dict(…))`.
pub fn parse(text: &str) -> Vec<(String, Block)> {
    let mut blocks = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_end();
        let (name, closing, list) = match () {
            _ if trimmed.ends_with('{') => (trimmed.trim_end_matches('{').trim(), "}", false),
            _ if trimmed.ends_with('[') => (trimmed.trim_end_matches('[').trim(), "]", true),
            _ => continue,
        };
        if name.is_empty() || line.starts_with(char::is_whitespace) {
            continue;
        }
        let body: Vec<&str> = lines
            .by_ref()
            .take_while(|l| l.trim_end() != closing)
            .collect();
        let block = match (list, is_text(name)) {
            (true, _) => Block::List(
                body.iter()
                    .map(|l| l.trim().trim_end_matches(',').to_string())
                    .filter(|l| !l.is_empty() && !l.starts_with('~'))
                    .collect(),
            ),
            (false, true) => Block::Text(dedent(&body)),
            (false, false) => Block::Dict(dict(&body)),
        };
        blocks.push((name.to_string(), block));
    }
    blocks
}

/// Blocks whose content is free text rather than `key: value` pairs.
fn is_text(name: &str) -> bool {
    [
        "body:json",
        "body:text",
        "body:xml",
        "body:sparql",
        "body:graphql",
        "body:graphql:vars",
    ]
    .contains(&name)
        || name.starts_with("script")
        || name == "tests"
        || name == "docs"
}

fn dict(lines: &[&str]) -> Vec<(String, String)> {
    lines
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('~'))
        .filter_map(split_entry)
        .collect()
}

/// `key: value`. Newer Bruno versions quote keys containing special characters, which may
/// include `:` themselves.
fn split_entry(line: &str) -> Option<(String, String)> {
    if let Some(rest) = line.strip_prefix('"') {
        let (key, after) = rest.split_once('"')?;
        let value = after.trim_start().strip_prefix(':')?;
        return Some((key.to_string(), value.trim().to_string()));
    }
    let (key, value) = line.split_once(':')?;
    Some((key.trim().to_string(), value.trim().to_string()))
}

/// Text block lines are indented by two spaces.
fn dedent(lines: &[&str]) -> String {
    lines
        .iter()
        .map(|l| l.strip_prefix("  ").unwrap_or(l))
        .collect::<Vec<_>>()
        .join("\n")
        .trim_end()
        .to_string()
}

/// The dictionary named `name`, or an empty one.
pub fn dict_of<'a>(blocks: &'a [(String, Block)], name: &str) -> &'a [(String, String)] {
    blocks
        .iter()
        .find_map(|(n, b)| match b {
            Block::Dict(entries) if n == name => Some(entries.as_slice()),
            _ => None,
        })
        .unwrap_or_default()
}

pub fn text_of(blocks: &[(String, Block)], name: &str) -> Option<String> {
    blocks.iter().find_map(|(n, b)| match b {
        Block::Text(text) if n == name => Some(text.clone()),
        _ => None,
    })
}

pub fn list_of(blocks: &[(String, Block)], name: &str) -> Vec<String> {
    blocks
        .iter()
        .find_map(|(n, b)| match b {
            Block::List(items) if n == name => Some(items.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

pub fn get(entries: &[(String, String)], key: &str) -> String {
    entries
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BRU: &str = "meta {\n  name: Create user\n  seq: 2\n}\n\npost {\n  url: {{baseUrl}}/users\n  body: json\n  auth: bearer\n}\n\nheaders {\n  Accept: application/json\n  ~X-Debug: 1\n}\n\nbody:json {\n  {\n    \"name\": \"Ada\"\n  }\n}\n\nvars:secret [\n  token,\n  ~old\n]\n\ntests {\n  test(\"ok\", () => {});\n}\n";

    #[test]
    fn parses_dict_text_and_list_blocks() {
        let blocks = parse(BRU);
        let names: Vec<_> = blocks.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            [
                "meta",
                "post",
                "headers",
                "body:json",
                "vars:secret",
                "tests"
            ]
        );
        assert_eq!(get(dict_of(&blocks, "meta"), "seq"), "2");
        assert_eq!(get(dict_of(&blocks, "post"), "url"), "{{baseUrl}}/users");
        assert_eq!(dict_of(&blocks, "headers").len(), 1);
        assert_eq!(
            text_of(&blocks, "body:json").unwrap(),
            "{\n  \"name\": \"Ada\"\n}"
        );
        assert_eq!(list_of(&blocks, "vars:secret"), ["token"]);
    }

    #[test]
    fn quoted_keys_lose_their_quotes() {
        let blocks = parse("headers {\n  \"X-Weird:Name\": v\n}\n");
        assert_eq!(
            dict_of(&blocks, "headers")[0],
            ("X-Weird:Name".to_string(), "v".to_string())
        );
    }
}
