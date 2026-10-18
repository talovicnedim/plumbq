use super::{basic_auth, code_headers, file_name, file_parts, text_parts};
use crate::multipart::{self, BodyShape};
use crate::send::SentRequest;

/// Laravel's `Http` client, since a lot of API work happens in PHP.
pub(super) fn generate(sent: &SentRequest) -> String {
    let shape = multipart::shape(&sent.headers, &sent.body);
    let mut chain = String::from("$response = Http::");
    let headers: Vec<String> = code_headers(sent, &shape)
        .iter()
        .map(|h| format!("    {} => {},", php_string(&h.name), php_string(&h.value)))
        .collect();
    chain.push_str(&format!("withHeaders([\n{}\n])", headers.join("\n")));
    if let Some((user, password)) = basic_auth(sent) {
        chain.push_str(&format!(
            "\n    ->withBasicAuth({}, {})",
            php_string(user),
            php_string(password)
        ));
    }
    let content_type = multipart::content_type(&sent.headers).unwrap_or("text/plain");
    let mut options = String::new();
    match &shape {
        BodyShape::Text if !sent.body.is_empty() => chain.push_str(&format!(
            "\n    ->withBody({}, {})",
            php_string(&sent.body),
            php_string(content_type)
        )),
        BodyShape::Text => {}
        BodyShape::File(path) => chain.push_str(&format!(
            "\n    ->withBody(file_get_contents({}), {})",
            php_string(path),
            php_string(content_type)
        )),
        BodyShape::Multipart(parts) => options = multipart_options(parts),
    }
    chain.push_str(&format!(
        "\n    ->send({}, {}{options});",
        php_string(&sent.method),
        php_string(&sent.url)
    ));
    format!("use Illuminate\\Support\\Facades\\Http;\n\n{chain}")
}

/// The `send` options for a multipart body.
fn multipart_options(parts: &[multipart::Part]) -> String {
    let mut rows: Vec<String> = text_parts(parts)
        .map(|(name, value)| {
            format!(
                "        ['name' => {}, 'contents' => {}],",
                php_string(name),
                php_string(value)
            )
        })
        .collect();
    rows.extend(file_parts(parts).map(|(part, path)| {
        format!(
            "        ['name' => {}, 'contents' => fopen({}, 'r'), 'filename' => {}],",
            php_string(&part.name),
            php_string(path),
            php_string(file_name(part, path))
        )
    }));
    format!(", [\n    'multipart' => [\n{}\n    ],\n]", rows.join("\n"))
}

/// Single-quoted PHP strings only interpret `\\` and `\'`.
fn php_string(text: &str) -> String {
    format!("'{}'", text.replace('\\', "\\\\").replace('\'', "\\'"))
}
