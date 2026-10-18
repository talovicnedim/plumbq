use super::{basic_auth, code_headers, file_name, file_parts, js_string, text_parts};
use crate::multipart::{self, BodyShape, Part};
use crate::send::SentRequest;

pub(super) fn generate(sent: &SentRequest) -> String {
    let shape = multipart::shape(&sent.headers, &sent.body);
    let mut setup = String::new();
    let body = match &shape {
        BodyShape::Text if sent.body.is_empty() => "nil".to_string(),
        BodyShape::Text => format!("strings.NewReader({})", go_string(&sent.body)),
        BodyShape::File(path) => {
            setup = format!(
                "file, err := os.Open({})\nif err != nil {{\n\tpanic(err)\n}}\ndefer file.Close()\n",
                go_string(path)
            );
            "file".to_string()
        }
        BodyShape::Multipart(parts) => {
            setup = go_multipart(parts);
            "&body".to_string()
        }
    };
    let mut headers: String = code_headers(sent, &shape)
        .iter()
        .map(|h| {
            format!(
                "req.Header.Set({}, {})\n",
                go_string(&h.name),
                go_string(&h.value)
            )
        })
        .collect();
    if matches!(shape, BodyShape::Multipart(_)) {
        headers.push_str("req.Header.Set(\"Content-Type\", writer.FormDataContentType())\n");
    }
    if let Some((user, password)) = basic_auth(sent) {
        headers.push_str(&format!(
            "req.SetBasicAuth({}, {})\n",
            go_string(user),
            go_string(password)
        ));
    }
    format!(
        "{setup}req, err := http.NewRequest({}, {}, {body})\nif err != nil {{\n\tpanic(err)\n}}\n{headers}resp, err := http.DefaultClient.Do(req)\nif err != nil {{\n\tpanic(err)\n}}\ndefer resp.Body.Close()",
        go_string(&sent.method),
        go_string(&sent.url)
    )
}

fn go_multipart(parts: &[Part]) -> String {
    let mut out = String::from("var body bytes.Buffer\nwriter := multipart.NewWriter(&body)\n");
    for (name, value) in text_parts(parts) {
        out.push_str(&format!(
            "writer.WriteField({}, {})\n",
            go_string(name),
            go_string(value)
        ));
    }
    for (part, path) in file_parts(parts) {
        out.push_str(&format!(
            "if part, err := writer.CreateFormFile({}, {}); err == nil {{\n\tdata, _ := os.ReadFile({})\n\tpart.Write(data)\n}}\n",
            go_string(&part.name),
            go_string(file_name(part, path)),
            go_string(path)
        ));
    }
    out.push_str("writer.Close()\n");
    out
}

fn go_string(text: &str) -> String {
    js_string(text)
}
