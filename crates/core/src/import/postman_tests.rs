use super::*;
use crate::import;

const COLLECTION: &str = r#"{
  "info": { "name": "Shop", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json" },
  "auth": { "type": "bearer", "bearer": [{ "key": "token", "value": "{{token}}" }] },
  "variable": [{ "key": "baseUrl", "value": "https://shop.test" }, { "key": "off", "value": "x", "disabled": true }],
  "item": [
    {
      "name": "Orders",
      "item": [
        {
          "name": "Create order",
          "request": {
            "method": "POST",
            "url": { "raw": "{{baseUrl}}/orders?draft=true", "host": ["{{baseUrl}}"], "path": ["orders"] },
            "header": [{ "key": "X-Trace", "value": "1" }, { "key": "X-Old", "value": "0", "disabled": true }],
            "body": { "mode": "raw", "raw": "{\"sku\": 1}", "options": { "raw": { "language": "json" } } }
          }
        }
      ]
    },
    {
      "name": "Login",
      "request": {
        "method": "POST",
        "url": { "protocol": "https", "host": ["auth", "shop", "test"], "path": ["login"] },
        "auth": { "type": "basic", "basic": [{ "key": "username", "value": "ada" }, { "key": "password", "value": "pw" }] },
        "body": { "mode": "urlencoded", "urlencoded": [{ "key": "remember", "value": "1" }] }
      }
    },
    { "name": "Health", "request": "https://shop.test/health" },
    {
      "name": "Search",
      "request": {
        "method": "POST",
        "url": "{{baseUrl}}/graphql",
        "auth": { "type": "apikey", "apikey": [{ "key": "key", "value": "api_key" }, { "key": "value", "value": "{{apiKey}}" }, { "key": "in", "value": "query" }] },
        "body": { "mode": "graphql", "graphql": { "query": "{ products { id } }", "variables": "{\"first\": 2}" } }
      }
    }
  ]
}"#;

fn request(items: &[ImportedItem], index: usize) -> &Request {
    match &items[index] {
        ImportedItem::Request(request) => request,
        ImportedItem::Folder { .. } => panic!("expected request"),
    }
}

#[test]
fn parses_folders_requests_and_variables() {
    let collection = import::parse(COLLECTION).unwrap();
    assert_eq!(collection.format, ImportFormat::Postman);
    assert_eq!(collection.headers["Authorization"], "Bearer {{token}}");
    assert_eq!(collection.environments[0].variables.len(), 1);
    assert_eq!(
        collection.environments[0].variables["baseUrl"],
        "https://shop.test"
    );

    let ImportedItem::Folder { name, items, .. } = &collection.items[0] else {
        panic!("expected folder")
    };
    assert_eq!(name, "Orders");
    let create = request(items, 0);
    assert_eq!(create.method, "POST");
    assert_eq!(create.url, "{{baseUrl}}/orders?draft=true");
    assert_eq!(create.headers.len(), 2);
    assert_eq!(create.headers[1].value, "application/json");
    assert_eq!(create.body, "{\"sku\": 1}");
}

#[test]
fn converts_auth_bodies_and_url_objects() {
    let collection = import::parse(COLLECTION).unwrap();

    let login = request(&collection.items, 1);
    assert_eq!(login.url, "https://auth.shop.test/login");
    assert_eq!(login.headers[0].value, "Basic YWRhOnB3");
    assert_eq!(login.body, "remember=1");

    let health = request(&collection.items, 2);
    assert_eq!(
        (health.method.as_str(), health.url.as_str()),
        ("GET", "https://shop.test/health")
    );

    let search = request(&collection.items, 3);
    assert_eq!(search.url, "{{baseUrl}}/graphql?api_key={{apiKey}}");
    let body: Value = serde_json::from_str(&search.body).unwrap();
    assert_eq!(body["variables"]["first"], 2);
}
