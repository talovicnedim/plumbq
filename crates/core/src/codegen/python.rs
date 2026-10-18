use super::{basic_auth, code_headers, file_name, file_parts, js_string, text_parts};
use crate::multipart::{self, BodyShape};
use crate::send::SentRequest;

pub(super) fn generate(sent: &SentRequest) -> String {
    let shape = multipart::shape(&sent.headers, &sent.body);
    let mut args = vec![py_string(&sent.url)];
    let headers: Vec<String> = code_headers(sent, &shape)
        .iter()
        .map(|h| format!("        {}: {},", py_string(&h.name), py_string(&h.value)))
        .collect();
    if !headers.is_empty() {
        args.push(format!("headers={{\n{}\n    }}", headers.join("\n")));
    }
    if let Some((user, password)) = basic_auth(sent) {
        args.push(format!(
            "auth=({}, {})",
            py_string(user),
            py_string(password)
        ));
    }
    match &shape {
        BodyShape::Text if !sent.body.is_empty() => {
            args.push(format!("data={}", py_string(&sent.body)))
        }
        BodyShape::Text => {}
        BodyShape::File(path) => args.push(format!("data=open({}, \"rb\")", py_string(path))),
        BodyShape::Multipart(parts) => {
            let data: Vec<String> = text_parts(parts)
                .map(|(name, value)| format!("{}: {}", py_string(name), py_string(value)))
                .collect();
            if !data.is_empty() {
                args.push(format!("data={{{}}}", data.join(", ")));
            }
            let files: Vec<String> = file_parts(parts)
                .map(|(part, path)| {
                    format!(
                        "{}: ({}, open({}, \"rb\"))",
                        py_string(&part.name),
                        py_string(file_name(part, path)),
                        py_string(path)
                    )
                })
                .collect();
            args.push(format!("files={{{}}}", files.join(", ")));
        }
    }
    format!(
        "import requests\n\nresponse = requests.request(\n    {},\n    {},\n)\nprint(response.status_code, response.text)",
        py_string(&sent.method),
        args.join(",\n    ")
    )
}

fn py_string(text: &str) -> String {
    js_string(text)
}
