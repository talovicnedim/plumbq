//! Converts between cURL commands and requests: "paste cURL" to create a request, and "copy as
//! cURL" to run one from a terminal.

mod form;
mod parse;
mod paste;
pub(crate) mod shell;

use std::collections::BTreeMap;
use std::path::Path;

use crate::http_file::Request;
use crate::multipart::{self, BodyShape, PartValue};
use crate::send::{self, SentRequest};
use crate::{Result, dynamic, environments, file_vars};
pub use paste::{CurlPaste, CurlSecrets, keep_secrets, paste};
use shell::{quote, word};

/// A pasted cURL command as a request ready to save.
#[derive(Debug, Clone)]
pub struct CurlImport {
    /// Literal credentials (`-u`, `Authorization`, API keys, secret-looking fields) are
    /// `{{secret}}` placeholders, so they never reach a file.
    pub request: Request,
    /// The names of those secrets, for the user to set.
    pub secrets: Vec<String>,
}

/// "Paste cURL" for MCP: the command as a request, with its credentials replaced by secrets.
/// Only the names come back, never the values.
pub fn import(command: &str) -> Result<CurlImport> {
    let pasted = paste(command)?;
    Ok(CurlImport {
        secrets: pasted.names(),
        request: pasted.request,
    })
}

/// "Copy as cURL": variables and folder headers resolved, secrets left as `{{name}}` so they
/// never end up on the clipboard or in a chat by accident.
pub fn export(
    root: &Path,
    id: &str,
    request: &Request,
    environment: Option<&str>,
) -> Result<String> {
    Ok(to_command(&prepare_for_export(
        root,
        id,
        request,
        environment,
    )?))
}

/// The request as it would be sent, except secrets and dynamic values stay `{{name}}`
/// placeholders. Shared by every "Copy as …" format.
pub fn prepare_for_export(
    root: &Path,
    id: &str,
    request: &Request,
    environment: Option<&str>,
) -> Result<SentRequest> {
    let mut values = environments::values(root, environment)?;
    for (name, value) in values.secrets.iter_mut() {
        *value = format!("{{{{{name}}}}}");
    }
    let text = send::request_text(root, id, request)?;
    dynamic::keep_as_placeholders(&text, &mut values);
    file_vars::resolve_into(root, id, request, &text, &BTreeMap::new(), &mut values)?;
    send::prepare(root, id, request, &values)
}

/// A shell-safe cURL command for a prepared request.
pub fn to_command(sent: &SentRequest) -> String {
    let shape = multipart::shape(&sent.headers, &sent.body);
    let is_multipart = matches!(shape, BodyShape::Multipart(_));
    let mut parts = vec!["curl".to_string()];
    if sent.method != "GET" {
        parts.push(format!("-X {}", word(&sent.method)));
    }
    parts.push(quote(&sent.url));
    for h in &sent.headers {
        // curl writes the multipart Content-Type itself, with its own boundary.
        if is_multipart && h.name.eq_ignore_ascii_case("content-type") {
            continue;
        }
        // curl encodes `-u` itself, so `{{name}}` placeholders stay readable.
        if let Some(credentials) = send::basic_credentials(h) {
            parts.push(format!("-u {}", quote(credentials)));
            continue;
        }
        parts.push(format!("-H {}", quote(&format!("{}: {}", h.name, h.value))));
    }
    match shape {
        BodyShape::Text if sent.body.is_empty() => {}
        BodyShape::Text => parts.push(format!("--data-raw {}", quote(&sent.body))),
        BodyShape::File(path) => {
            parts.push(format!("--data-binary {}", quote(&format!("@{path}"))))
        }
        BodyShape::Multipart(form) => {
            for part in form {
                let value = match &part.value {
                    PartValue::Text(text) => format!("{}={text}", part.name),
                    PartValue::File(path) => {
                        let mut field = format!("{}=@{path}", part.name);
                        if let Some(content_type) = &part.content_type {
                            field.push_str(&format!(";type={content_type}"));
                        }
                        field
                    }
                };
                parts.push(format!("-F {}", quote(&value)));
            }
        }
    }
    parts.join(" \\\n  ")
}

#[cfg(test)]
mod tests {
    use super::parse::{header, parse};
    use super::*;

    #[test]
    fn parses_browser_style_copy() {
        let command = "curl 'https://api.test/users' \\\n  -H 'accept: application/json' \\\n  \
                       -H \"Authorization: Bearer abc\" \\\n  --data-raw '{\"name\":\"Ada\"}'";
        let request = parse(command).unwrap();

        assert_eq!(request.method, "POST");
        assert_eq!(request.url, "https://api.test/users");
        assert_eq!(request.headers[1].value, "Bearer abc");
        assert_eq!(request.body, "{\"name\":\"Ada\"}");
    }

    #[test]
    fn explicit_method_and_json_flag() {
        let request = parse("curl -X put --json '{}' https://x.test/a").unwrap();
        assert_eq!(request.method, "PUT");
        assert_eq!(request.headers[0].value, "application/json");
    }

    #[test]
    fn get_flag_moves_data_to_query() {
        let request = parse("curl -G -d q=rust -d page=2 https://x.test/search").unwrap();
        assert_eq!(request.method, "GET");
        assert_eq!(request.url, "https://x.test/search?q=rust&page=2");
        assert!(request.body.is_empty());
    }

    #[test]
    fn basic_auth_becomes_header() {
        let request = parse("curl -u ada:pw https://x.test").unwrap();
        assert_eq!(request.headers[0].value, "Basic YWRhOnB3");
    }

    #[test]
    fn form_fields_become_a_multipart_body() {
        let request = parse(
            "curl -F title=Holiday -F 'photo=@beach.png;type=image/png' https://x.test/upload",
        )
        .unwrap();
        assert_eq!(request.method, "POST");
        let content_type = &request.headers[0].value;
        assert!(content_type.starts_with("multipart/form-data; boundary="));
        assert!(request.body.contains("name=\"title\"\n\nHoliday"));
        assert!(
            request
                .body
                .contains("filename=\"beach.png\"\nContent-Type: image/png\n\n< ./beach.png")
        );
        assert!(request.comments.is_empty());

        let absolute = parse("curl --form f=@/home/ada/x.png https://x.test").unwrap();
        assert_eq!(
            absolute.comments,
            ["# file path from curl, move the file into the workspace"]
        );
    }

    #[test]
    fn data_urlencode_encodes_values() {
        let request =
            parse("curl --data-urlencode 'q=hello world' --data-urlencode '=a&b' https://x.test")
                .unwrap();
        assert_eq!(request.body, "q=hello+world&a%26b");
    }

    #[test]
    fn imports_keep_literal_credentials_out() {
        let basic = import("curl -u ada:pw https://x.test").unwrap();
        assert_eq!(basic.request.headers[0].value, "Basic {{basicAuth}}");
        assert_eq!(basic.secrets, ["basicAuth"]);

        let bearer = import(
            "curl -H 'Authorization: Bearer abc' -d 'client_secret=s3cr3t' https://x.test/token",
        )
        .unwrap();
        assert_eq!(bearer.request.headers[0].value, "Bearer {{token}}");
        assert_eq!(bearer.request.body, "client_secret={{client_secret}}");
        assert_eq!(bearer.secrets, ["client_secret", "token"]);
    }

    #[test]
    fn rejects_non_curl_and_unclosed_quotes() {
        assert!(parse("wget https://x.test").is_err());
        assert!(parse("curl 'https://x.test").is_err());
    }

    #[test]
    fn command_round_trips_through_parse() {
        let sent = SentRequest {
            method: "POST".into(),
            url: "https://x.test/it's".into(),
            headers: vec![header("Content-Type", "application/json")],
            body: "{\"a\":1}".into(),
        };
        let request = parse(&to_command(&sent)).unwrap();
        assert_eq!(request.url, sent.url);
        assert_eq!(request.body, sent.body);
        assert_eq!(request.headers[0].name, "Content-Type");
    }
}
