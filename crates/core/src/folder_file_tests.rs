use std::fs;

use super::*;
use crate::workspace::tests::{temp_workspace, write};

#[test]
fn insert_in_order_materializes_current_order() {
    let root = temp_workspace("insert");
    write(&root, "a.http", "GET /a\n");
    write(&root, "b.http", "GET /b\n");

    insert_in_order(&root, "b.http", Some(0)).unwrap();
    assert_eq!(read_settings(&root).unwrap().order, ["b.http", "a.http"]);
}

#[test]
fn folder_toml_writes_keep_comments_and_unknown_keys() {
    let root = temp_workspace("toml-keep");
    write(&root, "a.http", "GET /a\n");
    write(&root, "b.http", "GET /b\n");
    write(
        &root,
        FOLDER_FILE,
        "# my notes\norder = [\"a.http\"]\ndescription = \"keep me\"\n\n# shared\n[headers]\nAccept = \"x\"\n",
    );

    let mut settings = read_settings(&root).unwrap();
    settings.order = vec!["b.http".into(), "a.http".into()];
    write_settings(&root, &settings).unwrap();

    let text = fs::read_to_string(root.join(FOLDER_FILE)).unwrap();
    assert!(text.contains("# my notes"), "{text}");
    assert!(text.contains("description = \"keep me\""), "{text}");
    assert!(text.contains("# shared\n[headers]"), "{text}");
    assert_eq!(read_settings(&root).unwrap().order, ["b.http", "a.http"]);
}

#[test]
fn folder_toml_edits_only_touch_what_changed() {
    let root = temp_workspace("toml-in-place");
    write(&root, "a.http", "GET /a\n");
    write(&root, "b.http", "GET /b\n");
    let original = "order = [\"a.http\"]\n\n[headers]\n# who calls\nX-Client = \"plumbq\" # app\nAccept = \"json\"\n\n[settings]\n# slow server\ntimeoutSecs = 60\nfutureKey = true\n";
    write(&root, FOLDER_FILE, original);

    let mut settings = read_settings(&root).unwrap();
    settings.order = vec!["b.http".into(), "a.http".into()];
    write_settings(&root, &settings).unwrap();
    let text = fs::read_to_string(root.join(FOLDER_FILE)).unwrap();
    let reordered = original.replace("[\"a.http\"]", "[\"b.http\", \"a.http\"]");
    assert_eq!(text, reordered);

    settings.headers.insert("Accept".into(), "xml".into());
    settings.settings.timeout_secs = Some(30);
    write_settings(&root, &settings).unwrap();
    let text = fs::read_to_string(root.join(FOLDER_FILE)).unwrap();
    let edited = reordered
        .replace("Accept = \"json\"", "Accept = \"xml\"")
        .replace("timeoutSecs = 60", "timeoutSecs = 30");
    assert_eq!(text, edited);

    settings.settings = Default::default();
    settings.headers.clear();
    write_settings(&root, &settings).unwrap();
    let text = fs::read_to_string(root.join(FOLDER_FILE)).unwrap();
    assert_eq!(
        text,
        "order = [\"b.http\", \"a.http\"]\n\n[settings]\nfutureKey = true\n"
    );
}

#[test]
fn root_variables_may_be_numbers_or_booleans() {
    let root = temp_workspace("toml-numbers");
    write(
        &root,
        FOLDER_FILE,
        "[variables]\nport = 8080\ndebug = true\nhost = \"api.test\"\n",
    );

    let mut settings = read_settings(&root).unwrap();
    assert_eq!(settings.variables["port"], "8080");
    assert_eq!(settings.variables["debug"], "true");

    settings.variables.insert("host".into(), "x.test".into());
    write_settings(&root, &settings).unwrap();
    let text = fs::read_to_string(root.join(FOLDER_FILE)).unwrap();
    assert_eq!(
        text,
        "[variables]\nport = 8080\ndebug = true\nhost = \"x.test\"\n"
    );
}
