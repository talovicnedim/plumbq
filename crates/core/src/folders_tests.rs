use super::*;
use crate::examples;
use crate::workspace::read_settings;
use crate::workspace::tests::{temp_workspace, write};

#[test]
fn create_and_rename_keep_order_in_sync() {
    let root = temp_workspace("folders");
    write(&root, "a.http", "GET /a\n");
    reorder(&root, "", vec!["a.http".into()]).unwrap();

    let path = create(&root, "", "User Admin").unwrap();
    assert_eq!(path, "user-admin");
    assert_eq!(
        read_settings(&root).unwrap().order,
        ["a.http", "user-admin"]
    );

    let renamed = rename(&root, &path, "Admins").unwrap();
    assert_eq!(renamed.path, "admins");
    assert_eq!(read_settings(&root).unwrap().order, ["a.http", "admins"]);
}

#[test]
fn rename_reports_the_new_ids_of_requests_inside() {
    let root = temp_workspace("folders-rename-ids");
    write(&root, "users/list.http", "GET /users\n");
    write(
        &root,
        "users/admin/all.http",
        "### A\nGET /a\n### B\nGET /b\n",
    );
    write(&root, "users-other/x.http", "GET /x\n");

    let renamed = rename(&root, "users", "accounts").unwrap();

    let mut changes: Vec<_> = renamed
        .changes
        .iter()
        .map(|c| (c.from.as_str(), c.to.as_deref().unwrap_or_default()))
        .collect();
    changes.sort();
    assert_eq!(
        changes,
        [
            ("users/admin/all.http", "accounts/admin/all.http"),
            ("users/admin/all.http#2", "accounts/admin/all.http#2"),
            ("users/list.http", "accounts/list.http"),
        ]
    );
}

#[test]
fn remove_without_recursive_only_deletes_folders_with_nothing_of_value() {
    let root = temp_workspace("remove-folder");
    write(&root, "users/list.http", "GET /users\n");
    write(&root, "src/main.rs", "fn main() {}\n");
    write(&root, "empty/folder.toml", "");
    write(&root, "empty/old.examples.json", "{}");
    write(&root, "empty/nested/folder.toml", "");

    assert!(remove(&root, "users", false).is_err());
    let err = remove(&root, "src", false).unwrap_err();
    assert!(err.to_string().contains("src/main.rs"));
    assert!(root.join("src/main.rs").exists());

    remove(&root, "empty", false).unwrap();
    assert!(!root.join("empty").exists());

    let removed = remove(&root, "users", true).unwrap();
    assert_eq!(
        removed,
        [IdChange {
            from: "users/list.http".into(),
            to: None
        }]
    );
    assert!(!root.join("users").exists());
}

#[test]
fn remove_refuses_the_root_hidden_folders_and_environments() {
    let root = temp_workspace("remove-guarded");
    write(&root, ".git/config", "");
    write(&root, "environments/local.toml", "");

    for path in ["", ".git", "environments"] {
        assert!(remove(&root, path, true).is_err(), "{path} was removed");
    }
    assert!(root.join(".git/config").exists());
    assert!(root.join("environments/local.toml").exists());
    assert!(rename(&root, "", "x").is_err());
}

#[test]
fn reorder_rejects_unknown_names() {
    let root = temp_workspace("reorder");
    write(&root, "a.http", "GET /a\n");
    assert!(reorder(&root, "", vec!["nope.http".into()]).is_err());
}

#[test]
fn reorder_keeps_shared_headers() {
    let root = temp_workspace("reorder-headers");
    write(&root, "a.http", "GET /a\n");
    write(
        &root,
        "folder.toml",
        "[headers]\nAccept = \"application/json\"\n",
    );

    reorder(&root, "", vec!["a.http".into()]).unwrap();

    let settings = read_settings(&root).unwrap();
    assert_eq!(settings.headers["Accept"], "application/json");
}

#[test]
fn duplicate_copies_everything_and_sits_after_the_original() {
    let root = temp_workspace("duplicate");
    write(&root, "users/list.http", "GET /users\n");
    write(&root, "users/folder.toml", "[headers]\nX-Team = \"core\"\n");
    write(&root, "orders/get.http", "GET /orders\n");
    reorder(&root, "", vec!["users".into(), "orders".into()]).unwrap();
    let response = crate::send::Response {
        status: 200,
        status_text: String::new(),
        headers: Vec::new(),
        body: "[]".into(),
        size: 2,
        binary: false,
        truncated: false,
        duration_ms: 1,
        sent: crate::send::SentRequest {
            method: "GET".into(),
            url: "/users".into(),
            headers: Vec::new(),
            body: String::new(),
        },
        assertions: Vec::new(),
        masked_secrets: Vec::new(),
        body_base64: String::new(),
    };
    examples::create(&root, "users/list.http", "Empty", &response).unwrap();

    let copy = duplicate(&root, "users").unwrap();

    assert_eq!(copy, "users-copy");
    assert!(root.join("users-copy/list.http").exists());
    assert_eq!(
        examples::list(&root, "users-copy/list.http").unwrap().len(),
        1
    );
    assert_eq!(
        read_settings(&root.join("users-copy")).unwrap().headers["X-Team"],
        "core"
    );
    assert_eq!(
        read_settings(&root).unwrap().order,
        ["users", "users-copy", "orders"]
    );
    assert!(duplicate(&root, "").is_err());
}

#[test]
fn content_lists_everything_but_folder_files() {
    let root = temp_workspace("folder-content");
    write(&root, "users/folder.toml", "");
    write(&root, "users/empty/folder.toml", "");
    assert!(content(&root, "users").unwrap().is_empty());

    write(&root, "users/data/payload.json", "{}");
    write(&root, "users/README.md", "docs");
    assert_eq!(
        content(&root, "users").unwrap(),
        ["users/README.md", "users/data/payload.json"]
    );
    assert!(matches!(content(&root, "missing"), Err(Error::NotFound(_))));
}

#[test]
fn shared_headers_merge_unless_replaced() {
    let map = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    };
    let current = map(&[
        ("Authorization", "Bearer {{token}}"),
        ("Accept", "text/plain"),
    ]);
    let change = |set: &[(&str, &str)], remove: &[&str], replace: bool| FolderHeaderChange {
        set: map(set),
        remove: remove.iter().map(|r| r.to_string()).collect(),
        replace,
    };
    assert_eq!(
        merged_headers(
            current.clone(),
            &change(&[("accept", "application/json")], &[], false)
        ),
        map(&[
            ("Authorization", "Bearer {{token}}"),
            ("accept", "application/json")
        ])
    );
    assert_eq!(
        merged_headers(current.clone(), &change(&[], &["AUTHORIZATION"], false)),
        map(&[("Accept", "text/plain")])
    );
    assert_eq!(
        merged_headers(current, &change(&[("X-Team", "core")], &[], true)),
        map(&[("X-Team", "core")])
    );

    let root = temp_workspace("folder-headers");
    write(&root, "users/folder.toml", "");
    let saved = set_headers(
        &root,
        "users",
        &change(&[("Accept", "application/json")], &[], false),
    )
    .unwrap();
    assert_eq!(saved, map(&[("Accept", "application/json")]));
    assert_eq!(get_settings(&root, "users").unwrap().headers, saved);
}

#[test]
fn a_broken_parent_folder_toml_stops_folder_changes_before_they_start() {
    let root = temp_workspace("folders-broken-order");
    write(&root, "api/users/list.http", "GET /users\n");
    write(&root, "api/folder.toml", "order = [\n");

    assert!(rename(&root, "api/users", "people").is_err());
    assert!(root.join("api/users/list.http").exists());
    assert!(remove(&root, "api/users", true).is_err());
    assert!(root.join("api/users/list.http").exists());
    assert!(create(&root, "api", "New").is_err());
    assert!(!root.join("api/new").exists());
    assert!(duplicate(&root, "api/users").is_err());
    assert!(!root.join("api/users-copy").exists());
}

#[test]
fn folders_the_tree_hides_at_the_top_are_refused() {
    let root = temp_workspace("folders-hidden-names");
    write(&root, "api/list.http", "GET /\n");

    for name in ["Build", "Vendor", "Environments"] {
        assert!(create(&root, "", name).is_err(), "{name}");
    }
    assert!(rename(&root, "api", "target").is_err());
    assert!(root.join("api/list.http").exists());
    assert_eq!(create(&root, "api", "Build").unwrap(), "api/build");
}

#[cfg(unix)]
#[test]
fn duplicate_never_copies_what_links_lead_outside_to() {
    let root = temp_workspace("duplicate-links");
    let outside = temp_workspace("duplicate-links-outside");
    write(&outside, "credentials", "aws-secret");
    write(&root, "users/list.http", "GET /users\n");
    write(&root, "shared.json", "{}");
    use std::os::unix::fs::symlink;
    symlink(outside.join("credentials"), root.join("users/creds.json")).unwrap();
    symlink(&outside, root.join("users/outside")).unwrap();
    symlink(root.join("shared.json"), root.join("users/shared.json")).unwrap();

    duplicate(&root, "users").unwrap();

    let copy = root.join("users-copy");
    assert!(copy.join("list.http").exists());
    assert_eq!(
        std::fs::read_to_string(copy.join("shared.json")).unwrap(),
        "{}"
    );
    assert!(!copy.join("creds.json").exists());
    assert!(!copy.join("outside").exists());
}
