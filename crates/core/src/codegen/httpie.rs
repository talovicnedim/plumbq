use super::{basic_auth, code_headers, file_parts, text_parts};
use crate::curl::shell::{quote, word};
use crate::multipart::{self, BodyShape};
use crate::send::SentRequest;

pub(super) fn generate(sent: &SentRequest) -> String {
    let shape = multipart::shape(&sent.headers, &sent.body);
    let mut parts = vec!["http".to_string()];
    if matches!(shape, BodyShape::Multipart(_)) {
        parts.push("--form".to_string());
    }
    if let Some((user, password)) = basic_auth(sent) {
        parts.push(format!("-a {}", quote(&format!("{user}:{password}"))));
    }
    parts.push(word(&sent.method));
    parts.push(quote(&sent.url));
    for h in code_headers(sent, &shape) {
        parts.push(quote(&format!("{}:{}", h.name, h.value)));
    }
    match &shape {
        BodyShape::Text if sent.body.is_empty() => {}
        BodyShape::Text => parts.push(format!("--raw {}", quote(&sent.body))),
        BodyShape::File(path) => parts.push(format!("< {}", quote(path))),
        BodyShape::Multipart(form) => {
            parts.extend(text_parts(form).map(|(name, value)| quote(&format!("{name}={value}"))));
            parts.extend(
                file_parts(form).map(|(part, path)| quote(&format!("{}@{path}", part.name))),
            );
        }
    }
    parts.join(" ")
}
