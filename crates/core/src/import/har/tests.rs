use serde_json::json;

use super::*;
use crate::import::camel;

fn entry(method: &str, url: &str, mime: &str) -> Value {
    json!({
        "request": {
            "method": method,
            "url": url,
            "headers": [
                {"name": ":authority", "value": "api.test"},
                {"name": "accept", "value": "application/json"},
                {"name": "sec-fetch-mode", "value": "cors"},
                {"name": "cookie", "value": "session=abc"},
                {"name": "authorization", "value": "Bearer t"}
            ]
        },
        "response": {"content": {"mimeType": mime}}
    })
}

#[test]
fn keeps_api_calls_and_drops_assets_duplicates_and_noise_headers() {
    let mut post = entry("POST", "https://api.test/users", "application/json");
    post["request"]["postData"] = json!({"mimeType": "application/json", "text": "{\"a\":1}"});
    let doc = json!({"log": {"entries": [
        entry("GET", "https://api.test/users?page=1", "application/json"),
        entry("GET", "https://api.test/users?page=1", "application/json"),
        entry("GET", "https://cdn.test/logo.png", "image/png"),
        entry("GET", "https://app.test/", "text/html"),
        post
    ]}});

    let collection = parse(&doc);
    assert_eq!(collection.name, "api.test");
    assert_eq!(collection.items.len(), 2);
    let ImportedItem::Request(get) = &collection.items[0] else {
        panic!("expected request")
    };
    assert_eq!(get.name, "GET /users");
    assert_eq!(get.url, "{{baseUrl}}/users?page=1");
    assert_eq!(get.comments, ["# @source GET https://api.test/users"]);
    assert_eq!(collection.environments.len(), 1);
    assert_eq!(collection.environments[0].name, "api.test");
    assert_eq!(
        collection.environments[0].variables["baseUrl"],
        "https://api.test"
    );
    let names: Vec<_> = get.headers.iter().map(|h| h.name.as_str()).collect();
    assert_eq!(names, ["accept", "authorization"]);
    let ImportedItem::Request(created) = &collection.items[1] else {
        panic!("expected request")
    };
    assert_eq!(created.body, "{\"a\":1}");
    assert_eq!(
        created.headers.last().map(|h| h.value.as_str()),
        Some("application/json")
    );
    assert_eq!(collection.notes.len(), 3);
}

#[test]
fn groups_several_hosts_into_folders() {
    let doc = json!({"log": {"entries": [
        entry("GET", "https://a.test/x", "application/json"),
        entry("GET", "https://b.test/y", "application/json")
    ]}});
    let collection = parse(&doc);
    assert_eq!(collection.name, "Recorded requests");
    let names: Vec<_> = collection
        .items
        .iter()
        .map(|item| match item {
            ImportedItem::Folder { name, .. } => name.clone(),
            ImportedItem::Request(r) => r.name.clone(),
        })
        .collect();
    assert_eq!(names, ["a.test", "b.test"]);

    let variables = &collection.environments[0].variables;
    assert_eq!(variables["aTestUrl"], "https://a.test");
    assert_eq!(variables["bTestUrl"], "https://b.test");
    let ImportedItem::Folder { items, .. } = &collection.items[1] else {
        panic!("expected folder")
    };
    let ImportedItem::Request(request) = &items[0] else {
        panic!("expected request")
    };
    assert_eq!(request.url, "{{bTestUrl}}/y");
}

#[test]
fn same_host_on_two_ports_gets_two_variables() {
    let doc = json!({"log": {"entries": [
        entry("GET", "http://localhost:3000/a", "application/json"),
        entry("GET", "http://localhost:8080/b", "application/json")
    ]}});
    let variables = &parse(&doc).environments[0].variables;
    assert_eq!(variables["localhost3000Url"], "http://localhost:3000");
    assert_eq!(variables["localhost8080Url"], "http://localhost:8080");
}

#[test]
fn reimport_merges_by_recorded_url() {
    let root = crate::workspace::tests::temp_workspace("har-merge");
    let doc = json!({"log": {"entries": [
        entry("GET", "https://api.test/users?page=1", "application/json")
    ]}});

    let first = crate::import::write::write(&root, "", parse(&doc)).unwrap();
    let second = crate::import::write::write(&root, "", parse(&doc)).unwrap();

    assert_eq!(first.added, 1);
    assert!(second.merged);
    assert_eq!((second.added, second.unchanged), (0, 1));
    let id = format!("{}/get-users.http", first.folder);
    let request = crate::requests::get(&root, &id).unwrap();
    assert_eq!(request.url, "{{baseUrl}}/users?page=1");
    assert_eq!(
        crate::environments::get(&root, "api-test")
            .unwrap()
            .variables["baseUrl"],
        "https://api.test"
    );
}

#[test]
fn camel_cases_hosts() {
    assert_eq!(camel("api.example.com"), "apiExampleCom");
    assert_eq!(camel("my-api.test:8080"), "myApiTest8080");
}

#[test]
fn form_params_become_urlencoded_body() {
    let mut login = entry("POST", "https://api.test/login", "application/json");
    login["request"]["postData"] = json!({
        "mimeType": "application/x-www-form-urlencoded",
        "params": [{"name": "user", "value": "ada"}, {"name": "pw", "value": "x"}]
    });
    let collection = parse(&json!({"log": {"entries": [login]}}));
    let ImportedItem::Request(request) = &collection.items[0] else {
        panic!("expected request")
    };
    assert_eq!(request.body, "user=ada&pw=x");
}
