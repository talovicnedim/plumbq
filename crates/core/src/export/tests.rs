use crate::send::{Response, SentRequest};
use crate::workspace::tests::{temp_workspace, write};
use crate::{examples, http_file::Header};

use super::{docs, postman};

fn workspace(name: &str) -> std::path::PathBuf {
    let root = temp_workspace(name);
    write(
        &root,
        "folder.toml",
        "order = [\"users\", \"health.http\"]\n[headers]\nAccept = \"application/json\"\n[variables]\nbaseUrl = \"http://localhost:8000\"\n",
    );
    write(&root, "users/folder.toml", "[headers]\nX-Team = \"core\"\n");
    write(
        &root,
        "users/create.http",
        "### Create user\n# Creates a **user** with `name`.\n# @tag users\n# @assert status == 201\n# @assert body.$.id exists\nPOST {{baseUrl}}/users\nContent-Type: application/json\nX-Team: admins\n\n{\"name\":\"<Ada>\",\"id\":\"{{$uuid}}\"}\n",
    );
    write(
        &root,
        "users/login.http",
        "### Login\nPOST {{baseUrl}}/login\nContent-Type: application/x-www-form-urlencoded\n\nuser=ada&pass={{password}}\n",
    );
    write(&root, "health.http", "### Health\nGET {{baseUrl}}/health\n");
    write(
        &root,
        "environments/staging.toml",
        "tag = \"staging\"\n[variables]\nbaseUrl = \"https://staging.test\"\n",
    );
    root
}

#[test]
fn postman_collection_keeps_folders_order_headers_and_bodies() {
    let root = workspace("export-postman");
    let collection = postman::collection(&root, "").unwrap();

    assert_eq!(
        collection["info"]["schema"],
        "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
    );
    assert_eq!(collection["variable"][0]["key"], "baseUrl");
    let users = &collection["item"][0];
    assert_eq!(users["name"], "users");
    assert_eq!(collection["item"][1]["name"], "Health");

    let create = &users["item"][0];
    assert_eq!(create["request"]["method"], "POST");
    assert_eq!(create["request"]["url"]["raw"], "{{baseUrl}}/users");
    let headers: Vec<(String, String)> = create["request"]["header"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| {
            (
                h["key"].as_str().unwrap().to_string(),
                h["value"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert!(headers.contains(&("Accept".into(), "application/json".into())));
    assert!(
        headers.contains(&("X-Team".into(), "admins".into())),
        "request header wins"
    );
    assert_eq!(
        create["request"]["body"]["options"]["raw"]["language"],
        "json"
    );
    assert!(
        create["request"]["body"]["raw"]
            .as_str()
            .unwrap()
            .contains("{{$guid}}")
    );
    assert!(
        create["request"]["description"]
            .as_str()
            .unwrap()
            .contains("`body.$.id exists`")
    );
    assert_eq!(
        create["event"][0]["script"]["exec"][0],
        "pm.test(\"status == 201\", () => pm.response.to.have.status(201));"
    );

    let login = &users["item"][1];
    assert_eq!(login["request"]["body"]["mode"], "urlencoded");
    assert_eq!(
        login["request"]["body"]["urlencoded"][1]["value"],
        "{{password}}"
    );
}

#[test]
fn postman_folder_export_and_environments() {
    let root = workspace("export-postman-folder");
    let users = postman::collection(&root, "users").unwrap();
    assert_eq!(users["info"]["name"], "users");
    assert_eq!(users["item"].as_array().unwrap().len(), 2);
    assert!(postman::collection(&root, "nope").is_err());

    let envs = postman::environments(&root).unwrap();
    assert_eq!(envs[0].0, "staging.postman_environment.json");
    assert_eq!(envs[0].1["values"][0]["value"], "https://staging.test");
}

#[test]
fn docs_escape_html_and_include_examples_and_environment() {
    let root = workspace("export-docs");
    let response = Response {
        status: 201,
        status_text: "Created".into(),
        headers: vec![Header {
            name: "Content-Type".into(),
            value: "application/json".into(),
        }],
        body: "{\"id\":\"<script>\"}".into(),
        size: 0,
        binary: false,
        body_base64: String::new(),
        truncated: false,
        duration_ms: 1,
        sent: SentRequest {
            method: "POST".into(),
            url: String::new(),
            headers: Vec::new(),
            body: String::new(),
        },
        assertions: Vec::new(),
        masked_secrets: Vec::new(),
    };
    examples::create(&root, "users/create.http", "Created", &response).unwrap();

    let html = docs::html(&root, Some("staging")).unwrap();

    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("<strong>user</strong> with <code>name</code>"));
    assert!(html.contains("&lt;Ada&gt;"));
    assert!(!html.contains("<script>"));
    assert!(html.contains("Example: Created"));
    assert!(html.contains("https://staging.test"));
    assert!(html.contains("<code>body.$.id exists</code>"));
}

#[test]
fn postman_exports_file_and_multipart_bodies() {
    let root = temp_workspace("export-files");
    write(
        &root,
        "uploads/avatar.http",
        "### Avatar\nPOST http://x.test/avatar\nContent-Type: image/png\n\n< ./img/avatar.png\n",
    );
    write(
        &root,
        "uploads/form.http",
        "### Form\nPOST http://x.test/form\nContent-Type: multipart/form-data; boundary=XYZ\n\n\
         --XYZ\nContent-Disposition: form-data; name=\"title\"\n\nHello {{name}}\n\
         --XYZ\nContent-Disposition: form-data; name=\"file\"; filename=\"a.txt\"\n\
         Content-Type: text/plain\n\n< ./a.txt\n--XYZ--\n",
    );

    let collection = postman::collection(&root, "").unwrap();
    let folder = &collection["item"][0]["item"];
    let avatar = &folder[0]["request"];
    assert_eq!(avatar["body"]["mode"], "file");
    assert_eq!(avatar["body"]["file"]["src"], "uploads/img/avatar.png");

    let form = &folder[1]["request"];
    assert_eq!(form["body"]["mode"], "formdata");
    assert_eq!(form["body"]["formdata"][0]["type"], "text");
    assert_eq!(form["body"]["formdata"][0]["value"], "Hello {{name}}");
    assert_eq!(form["body"]["formdata"][1]["type"], "file");
    assert_eq!(form["body"]["formdata"][1]["src"], "uploads/a.txt");
    let headers = form["header"].as_array().unwrap();
    assert!(
        !headers.iter().any(|h| h["key"] == "Content-Type"),
        "postman sets its own boundary"
    );
}

#[test]
fn postman_export_inlines_file_variables() {
    let root = temp_workspace("export-file-vars");
    write(
        &root,
        "api.http",
        "@host = https://api.test\n@api = {{host}}/v2\n\n### List\nGET {{api}}/items?k={{key}}\n\
         X-Host: {{ host }}\n",
    );

    let collection = postman::collection(&root, "").unwrap();

    let request = &collection["item"][0]["request"];
    assert_eq!(request["url"]["raw"], "https://api.test/v2/items?k={{key}}");
    assert_eq!(request["header"][0]["value"], "https://api.test");
}
