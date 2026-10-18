//! `multipart/form-data` bodies as written in `.http` files: split into parts for "Copy as
//! code", and read and written as fields for the body editor. Syntax: `docs/formats.md`
//! (Multipart bodies).

use serde::{Deserialize, Serialize};

use crate::http_file::Header;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    pub name: String,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub value: PartValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartValue {
    Text(String),
    /// A `< ./path` line: the file's contents, path relative to the `.http` file.
    File(String),
}

/// One field of a multipart body as the editor shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MultipartField {
    pub name: String,
    /// The text, or with `file` the path of the file to send, relative to the `.http` file.
    pub value: String,
    pub file: bool,
}

/// What a body is, for code generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyShape {
    Text,
    /// The whole body is one `< ./path` line.
    File(String),
    Multipart(Vec<Part>),
}

pub fn shape(headers: &[Header], body: &str) -> BodyShape {
    if let Some(boundary) = content_type(headers).and_then(boundary) {
        return BodyShape::Multipart(parts(body, &boundary));
    }
    let mut lines = body.lines().filter(|l| !l.trim().is_empty());
    match (lines.next().and_then(file_reference), lines.next()) {
        (Some(path), None) => BodyShape::File(path.to_string()),
        _ => BodyShape::Text,
    }
}

pub fn content_type(headers: &[Header]) -> Option<&str> {
    headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("content-type"))
        .map(|h| h.value.as_str())
}

/// The boundary of a `multipart/…; boundary=…` content type.
pub fn boundary(content_type: &str) -> Option<String> {
    if !content_type
        .trim_start()
        .to_lowercase()
        .starts_with("multipart/")
    {
        return None;
    }
    content_type.split(';').find_map(|param| {
        let (key, value) = param.split_once('=')?;
        (key.trim().eq_ignore_ascii_case("boundary"))
            .then(|| value.trim().trim_matches('"').to_string())
    })
}

/// `< ./path` or `<@ ./path` (but not the `<>` response redirect): the referenced path.
pub fn file_reference(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix('<')?;
    let rest = rest.strip_prefix('@').unwrap_or(rest);
    let path = rest.trim();
    (!path.is_empty() && rest.starts_with(char::is_whitespace)).then_some(path)
}

/// `<@ ./path`: the file is a template whose `{{variables}}` are resolved before sending
/// (REST Client semantics); plain `<` sends the file as is.
pub fn is_template_reference(line: &str) -> bool {
    line.trim().starts_with("<@") && file_reference(line).is_some()
}

/// The body's fields for the editor. Their own filename and Content-Type lines are left out:
/// [`format`] writes them from the path.
pub fn parse(body: &str, boundary: &str) -> Vec<MultipartField> {
    parts(body, boundary)
        .into_iter()
        .map(|part| match part.value {
            PartValue::Text(value) => MultipartField {
                name: part.name,
                value,
                file: false,
            },
            PartValue::File(value) => MultipartField {
                name: part.name,
                value,
                file: true,
            },
        })
        .collect()
}

/// A body for `fields`, leaving out those without a name. A file part gets its filename and a
/// Content-Type from the path.
pub fn format(fields: &[MultipartField], boundary: &str) -> String {
    let parts: Vec<String> = fields
        .iter()
        .filter(|field| !field.name.is_empty())
        .map(|field| format_field(field, boundary))
        .collect();
    match parts.is_empty() {
        true => String::new(),
        false => format!("{}\n--{boundary}--", parts.join("\n")),
    }
}

fn format_field(field: &MultipartField, boundary: &str) -> String {
    let disposition = format!(
        "--{boundary}\nContent-Disposition: form-data; name=\"{}\"",
        field.name
    );
    if !field.file {
        return format!("{disposition}\n\n{}", field.value);
    }
    let filename = field.value.rsplit('/').next().unwrap_or(&field.value);
    format!(
        "{disposition}; filename=\"{filename}\"\nContent-Type: {}\n\n< {}",
        content_type_for(&field.value),
        field.value
    )
}

/// A Content-Type for a file to send, from its extension.
pub fn content_type_for(path: &str) -> &'static str {
    let extension = path
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase());
    match extension.as_deref() {
        Some("json") => "application/json",
        Some("xml") => "application/xml",
        Some("txt") => "text/plain",
        Some("csv") => "text/csv",
        Some("html") => "text/html",
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("zip") => "application/zip",
        _ => "application/octet-stream",
    }
}

/// The parts of a multipart body, with their filename and Content-Type lines.
pub fn parts(body: &str, boundary: &str) -> Vec<Part> {
    let delimiter = format!("--{boundary}");
    let mut parts = Vec::new();
    let mut current: Option<Vec<&str>> = None;
    for line in body.lines() {
        let trimmed = line.trim_end_matches('\r');
        if trimmed.starts_with(&delimiter) {
            if let Some(lines) = current.take() {
                parts.extend(part(&lines));
            }
            if trimmed.trim_end() == format!("{delimiter}--") {
                break;
            }
            current = Some(Vec::new());
        } else if let Some(lines) = current.as_mut() {
            lines.push(trimmed);
        }
    }
    if let Some(lines) = current {
        parts.extend(part(&lines));
    }
    parts
}

fn part(lines: &[&str]) -> Option<Part> {
    let blank = lines
        .iter()
        .position(|l| l.trim().is_empty())
        .unwrap_or(lines.len());
    let (headers, content) = lines.split_at(blank);
    let mut name = None;
    let mut filename = None;
    let mut content_type = None;
    for header in headers {
        let Some((key, value)) = header.split_once(':') else {
            continue;
        };
        if key.trim().eq_ignore_ascii_case("content-type") {
            content_type = Some(value.trim().to_string());
        } else if key.trim().eq_ignore_ascii_case("content-disposition") {
            name = param(value, "name");
            filename = param(value, "filename");
        }
    }
    let content: Vec<&str> = content.iter().skip(1).copied().collect();
    let value = match content.as_slice() {
        [single] if file_reference(single).is_some() => {
            PartValue::File(file_reference(single).unwrap_or_default().to_string())
        }
        _ => PartValue::Text(content.join("\n").trim_end().to_string()),
    };
    Some(Part {
        name: name?,
        filename,
        content_type,
        value,
    })
}

/// `name="photo"` from a `Content-Disposition` value.
fn param(value: &str, key: &str) -> Option<String> {
    value.split(';').find_map(|p| {
        let (k, v) = p.split_once('=')?;
        (k.trim() == key).then(|| v.trim().trim_matches('"').to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = "--B\nContent-Disposition: form-data; name=\"title\"\n\nHoliday\nin June\n\
                        --B\nContent-Disposition: form-data; name=\"photo\"; filename=\"beach.png\"\n\
                        Content-Type: image/png\n\n< ./beach.png\n--B--\n";

    fn multipart_headers() -> Vec<Header> {
        vec![Header {
            name: "Content-Type".into(),
            value: "multipart/form-data; boundary=\"B\"".into(),
        }]
    }

    #[test]
    fn parses_text_and_file_parts() {
        let BodyShape::Multipart(parts) = shape(&multipart_headers(), BODY) else {
            panic!("not multipart");
        };
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].value, PartValue::Text("Holiday\nin June".into()));
        assert_eq!(parts[1].filename.as_deref(), Some("beach.png"));
        assert_eq!(parts[1].content_type.as_deref(), Some("image/png"));
        assert_eq!(parts[1].value, PartValue::File("./beach.png".into()));
    }

    #[test]
    fn recognizes_file_bodies_but_not_redirects() {
        assert_eq!(
            shape(&[], "< ./data.json\n"),
            BodyShape::File("./data.json".into())
        );
        assert_eq!(shape(&[], "{\"a\": 1}"), BodyShape::Text);
        assert_eq!(file_reference("<> ./previous.json"), None);
        assert_eq!(
            file_reference("<@ ./template.json"),
            Some("./template.json")
        );
        assert_eq!(file_reference("<not a file"), None);
    }

    #[test]
    fn fields_round_trip_through_the_body() {
        let fields = vec![
            MultipartField {
                name: "title".into(),
                value: "Holiday\nin June".into(),
                file: false,
            },
            MultipartField {
                name: "photo".into(),
                value: "./img/Beach.PNG".into(),
                file: true,
            },
        ];
        let body = format(&fields, "B");
        assert!(
            body.contains("filename=\"Beach.PNG\"\nContent-Type: image/png\n\n< ./img/Beach.PNG")
        );
        assert!(body.ends_with("\n--B--"));
        assert_eq!(parse(&body, "B"), fields);
        assert_eq!(parse(BODY, "B")[1].value, "./beach.png");
    }

    #[test]
    fn fields_without_a_name_are_left_out() {
        let unnamed = MultipartField {
            name: String::new(),
            value: "skip".into(),
            file: false,
        };
        assert_eq!(format(&[unnamed], "B"), "");
        assert_eq!(content_type_for("data"), "application/octet-stream");
        assert_eq!(content_type_for("a.tar.ZIP"), "application/zip");
    }

    #[test]
    fn reads_boundaries() {
        assert_eq!(
            boundary("multipart/form-data; boundary=abc").as_deref(),
            Some("abc")
        );
        assert_eq!(boundary("application/json; boundary=abc"), None);
    }
}
