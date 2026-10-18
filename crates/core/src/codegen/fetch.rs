use super::{basic_auth, code_headers, file_name, file_parts, js_string, text_parts};
use crate::multipart::{self, BodyShape};
use crate::send::SentRequest;

pub(super) fn generate(sent: &SentRequest) -> String {
    let shape = multipart::shape(&sent.headers, &sent.body);
    let mut setup = Vec::new();
    let mut options = vec![format!("  method: {},", js_string(&sent.method))];
    let mut headers: Vec<String> = code_headers(sent, &shape)
        .iter()
        .map(|h| format!("    {}: {},", js_string(&h.name), js_string(&h.value)))
        .collect();
    if let Some((user, password)) = basic_auth(sent) {
        let credentials = js_string(&format!("{user}:{password}"));
        headers.push(format!(
            "    \"Authorization\": \"Basic \" + btoa({credentials}),"
        ));
    }
    if !headers.is_empty() {
        options.push(format!("  headers: {{\n{}\n  }},", headers.join("\n")));
    }
    match &shape {
        BodyShape::Text if !sent.body.is_empty() => {
            options.push(format!("  body: {},", js_string(&sent.body)))
        }
        BodyShape::Text => {}
        BodyShape::File(path) => {
            setup.push("import { readFile } from 'node:fs/promises';\n".to_string());
            options.push(format!("  body: await readFile({}),", js_string(path)));
        }
        BodyShape::Multipart(parts) => {
            setup = form_setup(parts);
            options.push("  body: form,".to_string());
        }
    }
    format!(
        "{}const response = await fetch({}, {{\n{}\n}});\nconst data = await response.text();",
        setup.iter().map(|l| format!("{l}\n")).collect::<String>(),
        js_string(&sent.url),
        options.join("\n")
    )
}

/// The lines that build a multipart body as `form`.
fn form_setup(parts: &[multipart::Part]) -> Vec<String> {
    let mut setup = vec![
        "import { readFile } from 'node:fs/promises';\n".to_string(),
        "const form = new FormData();".to_string(),
    ];
    for (name, value) in text_parts(parts) {
        setup.push(format!(
            "form.append({}, {});",
            js_string(name),
            js_string(value)
        ));
    }
    for (part, path) in file_parts(parts) {
        setup.push(format!(
            "form.append({}, new Blob([await readFile({})]), {});",
            js_string(&part.name),
            js_string(path),
            js_string(file_name(part, path))
        ));
    }
    setup.push(String::new());
    setup
}
