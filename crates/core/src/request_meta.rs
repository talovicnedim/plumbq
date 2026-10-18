//! Tags (`# @tag`) and markdown docs kept in a request's `.http` comments. Syntax:
//! `docs/formats.md` (Tags and docs).

use crate::http_file::Request;

const TAG_DIRECTIVE: &str = "@tag";

/// The request's tags, in order, without duplicates.
pub fn tags(request: &Request) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    for line in &request.comments {
        let Some(list) = comment_text(line).and_then(|text| tag_list(text)) else {
            continue;
        };
        for tag in list
            .split([',', ' '])
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            if !tags.iter().any(|t| t == tag) {
                tags.push(tag.to_string());
            }
        }
    }
    tags
}

/// The REST Client reference name from `# @name login`: what `{{login.response…}}` uses in VS
/// Code. Not the display name (`### Log in`), though without a title the two are the same.
pub fn reference_name(request: &Request) -> Option<String> {
    request.comments.iter().find_map(|line| {
        let text = comment_text(line)?.trim();
        crate::http_file::read::name_directive(text)
            .filter(|name| !name.is_empty())
            .map(str::to_string)
    })
}

/// The request's documentation: its plain comment lines, markers removed.
pub fn docs(request: &Request) -> String {
    request
        .comments
        .iter()
        .filter_map(|line| doc_text(line))
        .collect::<Vec<_>>()
        .join("\n")
        .trim_end()
        .to_string()
}

/// `comments` with one `# @tag …` line for `tags`, where the first tag line was (or at the
/// end); no tag line when `tags` is empty.
pub fn with_tags(comments: &[String], tags: &[String]) -> Vec<String> {
    let is_tag_line = |line: &String| comment_text(line).and_then(tag_list).is_some();
    let first = comments.iter().position(is_tag_line);
    let mut rest: Vec<String> = comments
        .iter()
        .filter(|l| !is_tag_line(l))
        .cloned()
        .collect();
    let clean: Vec<&str> = tags
        .iter()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect();
    if clean.is_empty() {
        return rest;
    }
    let at = first.unwrap_or(rest.len()).min(rest.len());
    rest.insert(at, format!("# {TAG_DIRECTIVE} {}", clean.join(", ")));
    rest
}

/// `comments` with `docs` as `# ` lines first, then the directives in their order.
pub fn with_docs(comments: &[String], docs: &str) -> Vec<String> {
    let mut lines: Vec<String> = match docs.trim().is_empty() {
        true => Vec::new(),
        false => docs
            .trim_end()
            .lines()
            .map(|line| match line.trim().is_empty() {
                true => "#".to_string(),
                false => format!("# {}", line.trim_end()),
            })
            .collect(),
    };
    lines.extend(
        comments
            .iter()
            .filter(|line| doc_text(line).is_none())
            .cloned(),
    );
    lines
}

/// The text of a `#` or `//` comment line, without the marker.
fn comment_text(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    trimmed
        .strip_prefix('#')
        .or_else(|| trimmed.strip_prefix("//"))
}

/// The comma-separated list of an `@tag` directive.
fn tag_list(text: &str) -> Option<&str> {
    let rest = text.trim().strip_prefix(TAG_DIRECTIVE)?;
    // `@tags` or `@tagged` aren't the tag directive.
    (rest.is_empty() || rest.starts_with(char::is_whitespace)).then_some(rest)
}

/// A documentation line's text: a comment that isn't a `@directive`, with one space after the
/// marker removed so indentation inside the docs survives.
fn doc_text(line: &str) -> Option<&str> {
    let text = comment_text(line)?;
    if text.trim_start().starts_with('@') {
        return None;
    }
    Some(text.strip_prefix(' ').unwrap_or(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_file;

    fn request(text: &str) -> Request {
        http_file::parse(text).remove(0)
    }

    #[test]
    fn reads_tags_from_directives() {
        let r = request("# @tag smoke, auth\n// @tag users auth\nGET /x\n");
        assert_eq!(tags(&r), ["smoke", "auth", "users"]);
        assert!(tags(&request("# @tags not-a-tag\nGET /x\n")).is_empty());
    }

    #[test]
    fn reads_reference_names() {
        let r = request("### Log in\n# @name login\nPOST /login\n");
        assert_eq!(reference_name(&r).as_deref(), Some("login"));
        assert_eq!(r.name, "Log in");
        assert_eq!(reference_name(&request("# @namespace x\nGET /x\n")), None);
    }

    #[test]
    fn reads_docs_without_directives() {
        let r =
            request("### Login\n# Logs in.\n#\n#   - returns a token\n# @tag auth\nPOST /login\n");
        assert_eq!(docs(&r), "Logs in.\n\n  - returns a token");
    }

    fn lines(text: &[&str]) -> Vec<String> {
        text.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn writes_one_tag_line_where_the_first_was() {
        let comments = lines(&["# Docs", "# @tag old", "# @no-redirect", "// @tag more"]);
        let tags = lines(&["smoke", " ", "auth"]);
        assert_eq!(
            with_tags(&comments, &tags),
            ["# Docs", "# @tag smoke, auth", "# @no-redirect"]
        );
        assert_eq!(with_tags(&comments, &[]), ["# Docs", "# @no-redirect"]);
        assert_eq!(with_tags(&[], &tags), ["# @tag smoke, auth"]);
    }

    #[test]
    fn writes_docs_before_the_directives() {
        let comments = lines(&["# Old docs", "# @tag auth"]);
        assert_eq!(
            with_docs(&comments, "Logs in.\n\n  - returns a token\n"),
            ["# Logs in.", "#", "#   - returns a token", "# @tag auth"]
        );
        assert_eq!(with_docs(&comments, "  "), ["# @tag auth"]);
    }
}
