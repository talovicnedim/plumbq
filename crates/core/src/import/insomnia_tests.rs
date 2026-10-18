use super::*;
use crate::import;

const V4: &str = r#"{
  "_type": "export",
  "__export_format": 4,
  "resources": [
    { "_id": "wrk_1", "_type": "workspace", "name": "Billing" },
    { "_id": "env_base", "_type": "environment", "parentId": "wrk_1", "name": "Base", "data": { "baseUrl": "https://billing.test", "retries": 3 } },
    { "_id": "env_stg", "_type": "environment", "parentId": "env_base", "name": "Staging", "data": { "baseUrl": "https://stg.billing.test" } },
    { "_id": "fld_1", "_type": "request_group", "parentId": "wrk_1", "name": "Invoices", "metaSortKey": -10 },
    { "_id": "req_2", "_type": "request", "parentId": "fld_1", "name": "Pay invoice", "method": "POST", "metaSortKey": 2,
      "url": "{{ _.baseUrl }}/invoices/1/pay",
      "body": { "mimeType": "application/json", "text": "{\"amount\": {{_.amount}}}" },
      "headers": [{ "name": "X-Off", "value": "1", "disabled": true }],
      "authentication": { "type": "bearer", "token": "{{ _.token }}" } },
    { "_id": "req_1", "_type": "request", "parentId": "fld_1", "name": "List invoices", "method": "GET", "metaSortKey": 1,
      "url": "{{ _.baseUrl }}/invoices", "body": {}, "headers": [] },
    { "_id": "req_3", "_type": "request", "parentId": "wrk_1", "name": "Login", "method": "POST", "metaSortKey": 5,
      "url": "{{ _.baseUrl }}/login",
      "body": { "mimeType": "application/x-www-form-urlencoded", "params": [{ "name": "user", "value": "ada" }] },
      "authentication": { "type": "basic", "username": "ada", "password": "pw" } }
  ]
}"#;

const V5: &str = r#"
type: collection.insomnia.rest/5.0
name: Notes
collection:
  - name: Notes
    children:
      - name: List notes
        method: GET
        url: "{{ _.baseUrl }}/notes"
  - name: Ping
    url: https://notes.test/ping
environments:
  name: Base Environment
  data:
    baseUrl: https://notes.test
"#;

fn request(item: &ImportedItem) -> &Request {
    match item {
        ImportedItem::Request(request) => request,
        ImportedItem::Folder { .. } => panic!("expected request"),
    }
}

#[test]
fn v4_builds_tree_in_sort_order_and_converts_variables() {
    let collection = import::parse(V4).unwrap();
    assert_eq!(collection.format, ImportFormat::Insomnia);
    assert_eq!(collection.name, "Billing");

    let ImportedItem::Folder { name, items, .. } = &collection.items[0] else {
        panic!("expected folder")
    };
    assert_eq!(name, "Invoices");
    assert_eq!(request(&items[0]).name, "List invoices");

    let pay = request(&items[1]);
    assert_eq!(pay.url, "{{baseUrl}}/invoices/1/pay");
    assert_eq!(pay.body, "{\"amount\": {{amount}}}");
    assert_eq!(pay.headers[0].value, "application/json");
    assert_eq!(pay.headers[1].value, "Bearer {{token}}");

    let login = request(&collection.items[1]);
    assert_eq!(login.body, "user=ada");
    assert_eq!(login.headers[1].value, "Basic YWRhOnB3");
}

#[test]
fn v4_sub_environments_inherit_base_data() {
    let collection = import::parse(V4).unwrap();
    assert_eq!(collection.environments.len(), 1);
    let staging = &collection.environments[0];
    assert_eq!(staging.name, "Staging");
    assert_eq!(staging.variables["baseUrl"], "https://stg.billing.test");
    assert_eq!(staging.variables["retries"], "3");
}

#[test]
fn v5_yaml_folders_and_base_environment() {
    let collection = import::parse(V5).unwrap();
    assert_eq!(collection.name, "Notes");
    let ImportedItem::Folder { items, .. } = &collection.items[0] else {
        panic!("expected folder")
    };
    assert_eq!(request(&items[0]).url, "{{baseUrl}}/notes");
    assert_eq!(request(&collection.items[1]).method, "GET");
    assert_eq!(
        collection.environments[0].variables["baseUrl"],
        "https://notes.test"
    );
}

#[test]
fn convert_keeps_plain_variables_and_template_tags() {
    assert_eq!(
        convert("{{plain}} {% now 'iso' %}"),
        "{{plain}} {% now 'iso' %}"
    );
}
