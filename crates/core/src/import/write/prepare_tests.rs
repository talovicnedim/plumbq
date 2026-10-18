use super::*;
use crate::import;
use crate::workspace::tests::temp_workspace;

const POSTMAN: &str = r#"{
  "info": { "name": "Shop", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json" },
  "item": [
    { "name": "First", "request": { "method": "get", "url": "https://shop.test/1" } },
    { "name": "Second", "request": { "method": "post", "url": "https://shop.test/2",
        "header": [{ "key": "", "value": "" }, { "key": "X-Trace", "value": "1" }] } }
  ]
}"#;

#[test]
fn bad_headers_are_left_out_and_lowercase_methods_import() {
    let root = temp_workspace("import-prepare");
    let summary = write(&root, "", import::parse(POSTMAN).unwrap()).unwrap();

    assert_eq!(summary.added, 2);
    assert!(
        summary.notes.iter().any(|n| n.contains("Second")),
        "{:?}",
        summary.notes
    );
    assert_eq!(
        requests::get(&root, "shop/first.http").unwrap().method,
        "GET"
    );
    let second = requests::get(&root, "shop/second.http").unwrap();
    assert_eq!(second.method, "POST");
    assert_eq!(second.headers.len(), 1);
}

#[test]
fn an_invalid_request_fails_the_import_before_any_file_is_written() {
    let root = temp_workspace("import-invalid");
    let mut bad = Request::new("Bad", "GET", "/x");
    bad.comments.push("not a comment".into());
    let collection = ImportedCollection {
        format: ImportFormat::Postman,
        notes: Vec::new(),
        name: "Shop".into(),
        headers: BTreeMap::new(),
        items: vec![
            ImportedItem::Request(Request::new("Good", "GET", "/ok")),
            ImportedItem::Request(bad),
        ],
        environments: Vec::new(),
        credentials: Vec::new(),
    };

    assert!(write(&root, "", collection).is_err());
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
}

#[test]
fn re_imports_ignore_files_whose_names_ids_cannot_hold() {
    let root = temp_workspace("import-hash");
    write(&root, "", import::parse(POSTMAN).unwrap()).unwrap();
    std::fs::write(root.join("shop/c#.http"), "GET /c\n").unwrap();

    let again = write(&root, "", import::parse(POSTMAN).unwrap()).unwrap();
    assert!(again.merged);
    assert_eq!(again.unchanged, 2);
}

#[test]
fn a_collection_named_like_a_hidden_folder_gets_a_visible_one() {
    let root = temp_workspace("import-hidden-name");
    let source = POSTMAN.replace("\"Shop\"", "\"Environments\"");

    let first = write(&root, "", import::parse(&source).unwrap()).unwrap();
    assert_eq!(first.folder, "environments-requests");
    let again = write(&root, "", import::parse(&source).unwrap()).unwrap();
    assert!(again.merged);
    assert_eq!(again.folder, "environments-requests");
}
