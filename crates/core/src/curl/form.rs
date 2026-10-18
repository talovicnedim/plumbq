//! The parts of cURL import that build bodies: `-F` / `--form` multipart fields and
//! `--data-urlencode` values, turned into our `.http` syntax (see [`crate::multipart`]).

/// One `-F` field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FormField {
    name: String,
    value: FieldValue,
    content_type: Option<String>,
    filename: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FieldValue {
    Text(String),
    /// `name=@path`: a file upload.
    File(String),
    /// `name=<path`: a text field whose value is the file's contents.
    FileContents(String),
}

/// Parses `name=value`, `name=@file;type=…;filename=…` or `name=<file`. With `literal`
/// (`--form-string`) the value is always text.
pub(crate) fn field(spec: &str, literal: bool) -> Option<FormField> {
    let (name, rest) = spec.split_once('=')?;
    let mut field = FormField {
        name: name.trim().to_string(),
        value: FieldValue::Text(rest.to_string()),
        content_type: None,
        filename: None,
    };
    if literal {
        return Some(field);
    }
    let (kind, rest) = match rest.chars().next() {
        Some('@') => ('@', &rest[1..]),
        Some('<') => ('<', &rest[1..]),
        _ => return Some(field),
    };
    let mut pieces = rest.split(';');
    let path = pieces
        .next()
        .unwrap_or_default()
        .trim()
        .trim_matches('"')
        .to_string();
    for piece in pieces {
        match piece.trim().split_once('=') {
            Some(("type", value)) => field.content_type = Some(value.trim().to_string()),
            Some(("filename", value)) => {
                field.filename = Some(value.trim().trim_matches('"').to_string())
            }
            _ => {}
        }
    }
    field.value = match kind {
        '@' => {
            if field.filename.is_none() {
                field.filename = path.rsplit(['/', '\\']).next().map(str::to_string);
            }
            FieldValue::File(path)
        }
        _ => FieldValue::FileContents(path),
    };
    Some(field)
}

/// A multipart body for `fields`: the Content-Type header value and the body. Also says whether
/// a file path is absolute (it was relative to the shell, and must move into the workspace).
pub(crate) fn multipart(fields: &[FormField]) -> (String, String, bool) {
    let boundary = format!(
        "----plumbq{}",
        &uuid::Uuid::new_v4().simple().to_string()[..16]
    );
    let mut lines = Vec::new();
    let mut absolute = false;
    for field in fields {
        lines.push(format!("--{boundary}"));
        let mut disposition = format!("Content-Disposition: form-data; name=\"{}\"", field.name);
        if let Some(filename) = &field.filename {
            disposition.push_str(&format!("; filename=\"{filename}\""));
        }
        lines.push(disposition);
        if let Some(content_type) = &field.content_type {
            lines.push(format!("Content-Type: {content_type}"));
        }
        lines.push(String::new());
        match &field.value {
            FieldValue::Text(text) => lines.push(text.clone()),
            FieldValue::File(path) | FieldValue::FileContents(path) => {
                absolute |= is_absolute(path);
                lines.push(format!("< {}", relative(path)));
            }
        }
    }
    lines.push(format!("--{boundary}--"));
    (
        format!("multipart/form-data; boundary={boundary}"),
        lines.join("\n"),
        absolute,
    )
}

/// `--data-urlencode` semantics: `content`, `=content`, and `name=content` encode the content.
/// Like curl, whichever of `=` and `@` comes first decides; `@file` / `name@file` read a file
/// in curl, which an import can't, so they are kept as written.
pub(crate) fn urlencode(spec: &str) -> String {
    match spec.find(['=', '@']).map(|i| (i, &spec[i..i + 1])) {
        Some((0, "=")) => encode(&spec[1..]),
        Some((i, "=")) => format!("{}={}", &spec[..i], encode(&spec[i + 1..])),
        Some(_) => spec.to_string(),
        None => encode(spec),
    }
}

/// Percent-encodes everything but unreserved characters; spaces become `+` like curl's form
/// encoding. `{{variables}}` are kept so they still resolve.
fn encode(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(start) = rest.find("{{")
            && let Some(len) = rest[start..].find("}}")
        {
            out.push_str(&encode_plain(&rest[..start]));
            out.push_str(&rest[start..start + len + 2]);
            rest = &rest[start + len + 2..];
            continue;
        }
        out.push_str(&encode_plain(rest));
        break;
    }
    out
}

fn encode_plain(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            b' ' => "+".to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn is_absolute(path: &str) -> bool {
    path.starts_with('/') || path.starts_with('~') || path.get(1..3) == Some(":\\")
}

/// Paths in `.http` files are relative to the file; curl's are relative to the shell. Plain
/// relative paths get a `./` so the line reads as a file reference.
fn relative(path: &str) -> String {
    if is_absolute(path) || path.starts_with("./") || path.starts_with("../") {
        path.to_string()
    } else {
        format!("./{path}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_text_file_and_contents_fields() {
        assert_eq!(
            field("title=Holiday", false).unwrap().value,
            FieldValue::Text("Holiday".into())
        );
        let photo = field("photo=@pics/beach.png;type=image/png", false).unwrap();
        assert_eq!(photo.value, FieldValue::File("pics/beach.png".into()));
        assert_eq!(photo.filename.as_deref(), Some("beach.png"));
        assert_eq!(photo.content_type.as_deref(), Some("image/png"));
        let named = field("f=@a.bin;filename=\"b.bin\"", false).unwrap();
        assert_eq!(named.filename.as_deref(), Some("b.bin"));
        assert_eq!(
            field("note=<note.txt", false).unwrap().value,
            FieldValue::FileContents("note.txt".into())
        );
        assert_eq!(
            field("raw=@not-a-file", true).unwrap().value,
            FieldValue::Text("@not-a-file".into())
        );
        assert!(field("no-equals", false).is_none());
    }

    #[test]
    fn builds_a_multipart_body() {
        let fields = [
            field("title=Holiday", false).unwrap(),
            field("photo=@beach.png;type=image/png", false).unwrap(),
        ];
        let (content_type, body, absolute) = multipart(&fields);
        let boundary = content_type.split("boundary=").nth(1).unwrap();

        assert!(!absolute);
        let parts = crate::multipart::parts(&body, boundary);
        assert_eq!(parts.len(), 2);
        assert_eq!(
            parts[0].value,
            crate::multipart::PartValue::Text("Holiday".into())
        );
        assert_eq!(
            parts[1].value,
            crate::multipart::PartValue::File("./beach.png".into())
        );
        assert_eq!(parts[1].filename.as_deref(), Some("beach.png"));
        assert!(multipart(&[field("f=@/tmp/x.png", false).unwrap()]).2);
    }

    #[test]
    fn urlencodes_like_curl() {
        assert_eq!(urlencode("q=hello world&more"), "q=hello+world%26more");
        assert_eq!(urlencode("=a b"), "a+b");
        assert_eq!(urlencode("a/b c"), "a%2Fb+c");
        assert_eq!(urlencode("name=x {{id}} y"), "name=x+{{id}}+y");
        assert_eq!(urlencode("name@file.txt"), "name@file.txt");
        assert_eq!(urlencode("@file.txt"), "@file.txt");
    }
}
