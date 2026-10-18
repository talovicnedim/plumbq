use super::dir::parse_dir;
use super::*;
use crate::workspace::tests::{temp_workspace, write};

fn request(item: &ImportedItem) -> &Request {
    match item {
        ImportedItem::Request(request) => request,
        ImportedItem::Folder { .. } => panic!("expected request"),
    }
}

fn header<'a>(request: &'a Request, name: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(name))
        .map(|h| h.value.as_str())
}

fn bru_collection() -> std::path::PathBuf {
    let dir = temp_workspace("bruno");
    write(
        &dir,
        "bruno.json",
        r#"{"version": "1", "name": "Petstore", "type": "collection"}"#,
    );
    write(
        &dir,
        "collection.bru",
        "headers {\n  X-Client: bruno\n}\n\nauth {\n  mode: bearer\n}\n\nauth:bearer {\n  token: {{token}}\n}\n",
    );
    write(
        &dir,
        "List pets.bru",
        "meta {\n  name: List pets\n  type: http\n  seq: 2\n}\n\nget {\n  url: {{baseUrl}}/pets\n  body: none\n  auth: inherit\n}\n\nparams:query {\n  limit: 10\n  ~debug: 1\n}\n",
    );
    write(
        &dir,
        "Create pet.bru",
        "meta {\n  name: Create pet\n  seq: 1\n}\n\npost {\n  url: {{baseUrl}}/pets/:shop\n  body: json\n  auth: basic\n}\n\nparams:path {\n  shop: main\n}\n\nheaders {\n  Accept: application/json\n  ~X-Off: 1\n  X-Key: {{process.env.API_KEY}}\n}\n\nauth:basic {\n  username: ada\n  password: pw\n}\n\nbody:json {\n  {\n    \"name\": \"Rex\"\n  }\n}\n\ntests {\n  test(\"created\", function() {});\n}\n",
    );
    write(
        &dir,
        "orders/folder.bru",
        "meta {\n  name: Orders\n  seq: 3\n}\n",
    );
    write(
        &dir,
        "orders/Pay.bru",
        "meta {\n  name: Pay\n  seq: 1\n}\n\npost {\n  url: {{baseUrl}}/pay\n  body: formUrlEncoded\n  auth: none\n}\n\nbody:form-urlencoded {\n  amount: 5\n  ~note: x\n}\n",
    );
    write(
        &dir,
        "environments/Local.bru",
        "vars {\n  baseUrl: http://localhost:3000\n  ~old: 1\n}\nvars:secret [\n  token\n]\n",
    );
    dir
}

#[test]
fn imports_a_bru_collection_folder_in_seq_order() {
    let collection = parse_dir(&bru_collection()).unwrap();

    assert_eq!(collection.name, "Petstore");
    assert_eq!(collection.format, ImportFormat::Bruno);
    assert_eq!(collection.headers["X-Client"], "bruno");
    assert_eq!(collection.headers["Authorization"], "Bearer {{token}}");

    let names: Vec<_> = collection
        .items
        .iter()
        .map(|item| match item {
            ImportedItem::Folder { name, .. } => name.clone(),
            ImportedItem::Request(r) => r.name.clone(),
        })
        .collect();
    assert_eq!(names, ["Create pet", "List pets", "Orders"]);

    let create = request(&collection.items[0]);
    assert_eq!(create.method, "POST");
    assert_eq!(create.url, "{{baseUrl}}/pets/main");
    assert_eq!(create.body, "{\n  \"name\": \"Rex\"\n}");
    assert_eq!(header(create, "X-Off"), None);
    assert_eq!(header(create, "X-Key"), Some("{{$processEnv API_KEY}}"));
    assert_eq!(header(create, "Authorization"), Some("Basic YWRhOnB3"));
    assert_eq!(header(create, "Content-Type"), Some("application/json"));

    let list = request(&collection.items[1]);
    assert_eq!(list.url, "{{baseUrl}}/pets?limit=10");
    assert_eq!(header(list, "Authorization"), None);

    let ImportedItem::Folder { items, .. } = &collection.items[2] else {
        panic!("expected folder")
    };
    assert_eq!(request(&items[0]).body, "amount=5");

    let env = &collection.environments[0];
    assert_eq!(env.name, "Local");
    assert_eq!(env.variables["baseUrl"], "http://localhost:3000");
    assert!(!env.variables.contains_key("old"));
    assert_eq!(env.secrets, ["token"]);
}

#[test]
fn imports_yaml_collection_folder() {
    let dir = temp_workspace("bruno-yaml");
    write(&dir, "opencollection.yml", "info:\n  name: Yaml API\n");
    write(
        &dir,
        "get-user.yml",
        "info:\n  name: Get user\n  type: http\n  seq: 1\nhttp:\n  method: get\n  url: \"{{baseUrl}}/users/1\"\n  headers:\n    - name: Accept\n      value: application/json\n    - name: X-Off\n      value: \"1\"\n      disabled: true\n  auth:\n    type: bearer\n    token: \"{{token}}\"\n",
    );
    write(
        &dir,
        "environments/dev.yml",
        "variables:\n  - name: baseUrl\n    value: http://dev.test\n  - name: token\n    value: \"\"\n    secret: true\n",
    );

    let collection = parse_dir(&dir).unwrap();
    assert_eq!(collection.name, "Yaml API");
    let get_user = request(&collection.items[0]);
    assert_eq!(get_user.method, "GET");
    assert_eq!(get_user.headers.len(), 2);
    assert_eq!(header(get_user, "Authorization"), Some("Bearer {{token}}"));
    assert_eq!(collection.environments[0].secrets, ["token"]);
    assert_eq!(
        collection.environments[0].variables["baseUrl"],
        "http://dev.test"
    );
}

#[test]
fn single_bru_file_and_graphql_body() {
    let text = "meta {\n  name: Query\n}\n\npost {\n  url: https://gql.test\n  body: graphql\n}\n\nbody:graphql {\n  { users { id } }\n}\n\nbody:graphql:vars {\n  {\"limit\": 2}\n}\n";
    let collection = parse_bru_file(Path::new("query.bru"), text).unwrap();
    let query = request(&collection.items[0]);
    let body: Value = serde_json::from_str(&query.body).unwrap();
    assert_eq!(body["query"], "{ users { id } }");
    assert_eq!(body["variables"]["limit"], 2);
}

#[test]
fn rejects_folders_that_are_not_bruno_collections() {
    assert!(parse_dir(&temp_workspace("not-bruno")).is_err());
}

#[test]
fn converts_process_env_variables() {
    assert_eq!(
        convert("{{process.env.A}}/{{b}}/{{ process.env.C}}"),
        "{{$processEnv A}}/{{b}}/{{ process.env.C}}"
    );
}

#[cfg(unix)]
#[test]
fn symlinks_in_a_collection_are_not_followed() {
    let dir = temp_workspace("bruno-links");
    let outside = temp_workspace("bruno-links-outside");
    write(&dir, "bruno.json", r#"{"name": "Links"}"#);
    write(
        &dir,
        "list.bru",
        "meta {\n  name: List\n}\n\nget {\n  url: https://x.test\n}\n",
    );
    write(
        &outside,
        "secret.bru",
        "meta {\n  name: Secret\n}\n\nget {\n  url: https://evil.test\n}\n",
    );
    std::os::unix::fs::symlink(&dir, dir.join("loop")).unwrap();
    std::os::unix::fs::symlink(outside.join("secret.bru"), dir.join("secret.bru")).unwrap();

    let collection = parse_dir(&dir).unwrap();
    assert_eq!(collection.items.len(), 1);
    assert_eq!(request(&collection.items[0]).name, "List");
}
