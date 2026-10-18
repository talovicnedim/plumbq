use super::*;
use crate::http_file::Header;

fn sent() -> SentRequest {
    SentRequest {
        method: "POST".into(),
        url: "https://api.test/users".into(),
        headers: vec![
            Header {
                name: "Content-Type".into(),
                value: "application/json".into(),
            },
            Header {
                name: "Authorization".into(),
                value: "Bearer {{token}}".into(),
            },
        ],
        body: r#"{"name":"Ada's"}"#.into(),
    }
}

#[test]
fn fetch_snippet() {
    let code = generate(&sent(), CodeLanguage::Fetch);
    assert!(code.starts_with("const response = await fetch(\"https://api.test/users\", {"));
    assert!(code.contains("\"Authorization\": \"Bearer {{token}}\","));
    assert!(code.contains(r#"body: "{\"name\":\"Ada's\"}","#));
}

#[test]
fn python_snippet() {
    let code = generate(&sent(), CodeLanguage::Python);
    assert!(code.contains("requests.request(\n    \"POST\",\n    \"https://api.test/users\","));
    assert!(code.contains(r#"data="{\"name\":\"Ada's\"}""#));
}

#[test]
fn laravel_snippet_escapes_single_quotes() {
    let code = generate(&sent(), CodeLanguage::Laravel);
    assert!(code.contains("use Illuminate\\Support\\Facades\\Http;"));
    assert!(code.contains(r#"->withBody('{"name":"Ada\'s"}', 'application/json')"#));
    assert!(code.ends_with("->send('POST', 'https://api.test/users');"));
}

#[test]
fn go_and_httpie_snippets() {
    let go = generate(&sent(), CodeLanguage::Go);
    assert!(
        go.contains("http.NewRequest(\"POST\", \"https://api.test/users\", strings.NewReader(")
    );
    assert!(go.contains("\nreq.Header.Set(\"Content-Type\", \"application/json\")\n"));
    let httpie = generate(&sent(), CodeLanguage::Httpie);
    assert!(
        httpie.starts_with("http POST 'https://api.test/users' 'Content-Type:application/json'")
    );
    assert!(httpie.ends_with(r#"--raw '{"name":"Ada'\''s"}'"#));
}

#[test]
fn get_without_body_or_headers() {
    let get = SentRequest {
        method: "GET".into(),
        url: "https://api.test".into(),
        headers: Vec::new(),
        body: String::new(),
    };
    assert!(!generate(&get, CodeLanguage::Fetch).contains("body:"));
    assert!(generate(&get, CodeLanguage::Go).contains(", nil)"));
    assert_eq!(
        generate(&get, CodeLanguage::Httpie),
        "http GET 'https://api.test'"
    );
}

fn upload() -> SentRequest {
    SentRequest {
        method: "POST".into(),
        url: "https://api.test/upload".into(),
        headers: vec![Header {
            name: "Content-Type".into(),
            value: "multipart/form-data; boundary=B".into(),
        }],
        body: "--B\nContent-Disposition: form-data; name=\"title\"\n\nHoliday\n--B\n\
               Content-Disposition: form-data; name=\"photo\"; filename=\"beach.png\"\n\
               Content-Type: image/png\n\n< ./beach.png\n--B--"
            .into(),
    }
}

fn file_body() -> SentRequest {
    SentRequest {
        method: "PUT".into(),
        url: "https://api.test/data".into(),
        headers: vec![Header {
            name: "Content-Type".into(),
            value: "application/json".into(),
        }],
        body: "< ./data.json".into(),
    }
}

#[test]
fn curl_uses_form_fields_and_data_binary() {
    let multipart = generate(&upload(), CodeLanguage::Curl);
    assert!(multipart.contains("-F 'title=Holiday'"));
    assert!(multipart.contains("-F 'photo=@./beach.png;type=image/png'"));
    assert!(
        !multipart.contains("Content-Type"),
        "curl sets its own boundary"
    );
    assert!(generate(&file_body(), CodeLanguage::Curl).contains("--data-binary '@./data.json'"));
}

#[test]
fn snippets_use_each_clients_file_api() {
    let fetch = generate(&upload(), CodeLanguage::Fetch);
    assert!(fetch.contains(
        "form.append(\"photo\", new Blob([await readFile(\"./beach.png\")]), \"beach.png\");"
    ));
    assert!(fetch.contains("body: form,"));

    let python = generate(&upload(), CodeLanguage::Python);
    assert!(python.contains("data={\"title\": \"Holiday\"}"));
    assert!(python.contains("files={\"photo\": (\"beach.png\", open(\"./beach.png\", \"rb\"))}"));

    let laravel = generate(&upload(), CodeLanguage::Laravel);
    assert!(laravel.contains(
        "['name' => 'photo', 'contents' => fopen('./beach.png', 'r'), 'filename' => 'beach.png'],"
    ));

    let go = generate(&upload(), CodeLanguage::Go);
    assert!(go.contains("writer.CreateFormFile(\"photo\", \"beach.png\")"));
    assert!(go.contains("writer.FormDataContentType()"));

    let httpie = generate(&upload(), CodeLanguage::Httpie);
    assert!(httpie.starts_with("http --form POST"));
    assert!(httpie.ends_with("'title=Holiday' 'photo@./beach.png'"));

    assert!(
        generate(&file_body(), CodeLanguage::Python).contains("data=open(\"./data.json\", \"rb\")")
    );
    assert!(generate(&file_body(), CodeLanguage::Httpie).ends_with("< './data.json'"));
    assert!(generate(&file_body(), CodeLanguage::Go).contains("os.Open(\"./data.json\")"));
}

#[test]
fn readable_basic_auth_uses_each_clients_basic_auth_option() {
    let mut sent = sent();
    sent.headers[1].value = "Basic ada:{{pw}}".into();
    let code = |language| generate(&sent, language);

    assert!(code(CodeLanguage::Curl).contains("-u 'ada:{{pw}}'"));
    assert!(code(CodeLanguage::Httpie).contains(" -a 'ada:{{pw}}'"));
    assert!(
        code(CodeLanguage::Fetch).contains("\"Authorization\": \"Basic \" + btoa(\"ada:{{pw}}\"),")
    );
    assert!(code(CodeLanguage::Python).contains("auth=(\"ada\", \"{{pw}}\")"));
    assert!(code(CodeLanguage::Go).contains("req.SetBasicAuth(\"ada\", \"{{pw}}\")\n"));
    assert!(code(CodeLanguage::Laravel).contains("->withBasicAuth('ada', '{{pw}}')"));
    for language in [
        CodeLanguage::Curl,
        CodeLanguage::Httpie,
        CodeLanguage::Python,
        CodeLanguage::Go,
        CodeLanguage::Laravel,
    ] {
        assert!(!code(language).contains("Basic ada"), "{language:?}");
    }
}

#[test]
fn shell_commands_quote_odd_methods() {
    let mut sent = sent();
    sent.method = "X;id".into();
    assert!(generate(&sent, CodeLanguage::Curl).starts_with("curl \\\n  -X 'X;id' \\\n"));
    assert!(
        generate(&sent, CodeLanguage::Httpie).starts_with("http 'X;id' 'https://api.test/users'")
    );
    sent.method = "PURGE".into();
    assert!(generate(&sent, CodeLanguage::Curl).contains("-X PURGE \\\n"));
    assert!(generate(&sent, CodeLanguage::Httpie).starts_with("http PURGE "));
}

#[test]
fn export_resolves_file_variables() {
    let root = crate::workspace::tests::temp_workspace("export-file-vars");
    crate::workspace::tests::write(
        &root,
        "a.http",
        "@host = api.test\n\n### A\nGET https://{{host}}/a\n",
    );
    let request = crate::requests::get(&root, "a.http").unwrap();
    let code = export(&root, "a.http", &request, None, CodeLanguage::Curl).unwrap();
    assert_eq!(code, "curl \\\n  'https://api.test/a'");
}
