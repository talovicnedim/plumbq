use super::*;
use crate::workspace::tests::{temp_workspace, write};
use crate::workspace::{FOLDER_FILE, read_settings};

#[test]
fn parallel_creates_with_one_name_never_overwrite() {
    let root = temp_workspace("create-parallel");
    let ids: Vec<String> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let root = &root;
                scope.spawn(move || {
                    let request = Request::new("Same", "GET", &format!("/{i}"));
                    create(root, "", request).unwrap()
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 8, "{ids:?}");
}

#[test]
fn create_names_file_after_request() {
    let root = temp_workspace("create");
    let id = create(&root, "", Request::new("List users", "GET", "/users")).unwrap();
    assert_eq!(id, "list-users.http");

    let again = create(&root, "", Request::new("List users", "GET", "/users")).unwrap();
    assert_eq!(again, "list-users-2.http");
    assert_eq!(get(&root, &again).unwrap().url, "/users");
}

#[test]
fn update_keeps_other_requests_in_file() {
    let root = temp_workspace("update");
    write(&root, "all.http", "### One\nGET /1\n### Two\nGET /2\n");

    update(
        &root,
        "all.http#2",
        Request::new("Two", "POST", "/two"),
        None,
    )
    .unwrap();

    assert_eq!(get(&root, "all.http").unwrap().url, "/1");
    assert_eq!(get(&root, "all.http#2").unwrap().method, "POST");
}

#[test]
fn remove_deletes_file_and_order_entry() {
    let root = temp_workspace("remove");
    write(&root, "a.http", "GET /a\n");
    write(&root, "b.http", "GET /b\n");
    write(&root, FOLDER_FILE, "order = [\"b.http\", \"a.http\"]\n");

    remove(&root, "b.http", None).unwrap();

    assert!(!root.join("b.http").exists());
    assert_eq!(read_settings(&root).unwrap().order, ["a.http"]);
}

#[test]
fn move_to_other_folder_at_position() {
    let root = temp_workspace("move");
    write(&root, "login.http", "POST /login\n");
    write(&root, "auth/logout.http", "POST /logout\n");

    let id = move_to(&root, "login.http", "auth", Some(0), None)
        .unwrap()
        .id;

    assert_eq!(id, "auth/login.http");
    assert!(!root.join("login.http").exists());
    let order = read_settings(&root.join("auth")).unwrap().order;
    assert_eq!(order, ["login.http", "logout.http"]);
}

#[test]
fn move_splits_request_out_of_multi_request_file() {
    let root = temp_workspace("split");
    write(&root, "all.http", "### One\nGET /1\n### Two\nGET /2\n");
    fs::create_dir(root.join("two")).unwrap();

    let id = move_to(&root, "all.http#2", "two", None, None).unwrap().id;

    assert_eq!(id, "two/two.http");
    assert_eq!(get(&root, "all.http").unwrap().name, "One");
    assert!(get(&root, "all.http#2").is_err());
}

#[test]
fn file_prefix_survives_updates_and_removing_the_first_request() {
    let root = temp_workspace("prefix");
    write(
        &root,
        "all.http",
        "@host = api.test\n\n### One\nGET /1\n### Two\nGET /2\n",
    );

    update(&root, "all.http", Request::new("One", "POST", "/1"), None).unwrap();
    assert_eq!(get(&root, "all.http").unwrap().prefix, "@host = api.test");

    remove(&root, "all.http", None).unwrap();
    let first = get(&root, "all.http").unwrap();
    assert_eq!(first.name, "Two");
    assert_eq!(first.prefix, "@host = api.test");
}

#[test]
fn split_out_requests_keep_the_file_variables_they_use() {
    let root = temp_workspace("split-vars");
    write(
        &root,
        "all.http",
        "@host = https://api.test\n@api = {{host}}/v2\n@unused = x\n\n\
         ### One\nGET {{api}}/one\n\n### Two\n@limit = 5\nGET {{api}}/two?limit={{limit}}\n",
    );
    fs::create_dir(root.join("two")).unwrap();

    let id = move_to(&root, "all.http#2", "two", None, None).unwrap().id;

    let moved = get(&root, &id).unwrap();
    let names: Vec<_> = moved.variables.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["host", "api", "limit"]);
    // The original keeps its own preamble.
    let left = fs::read_to_string(root.join("all.http")).unwrap();
    assert!(left.starts_with("@host = https://api.test"));
}

#[test]
fn stale_reads_are_refused_instead_of_overwriting_another_request() {
    let root = temp_workspace("conflict");
    write(&root, "all.http", "### A\nGET /a\n\n### B\nGET /b\n");
    let read = get(&root, "all.http#2").unwrap();
    // An editor inserts a request at the top: `#2` is now A.
    write(
        &root,
        "all.http",
        "### Z\nGET /z\n\n### A\nGET /a\n\n### B\nGET /b\n",
    );

    let edited = Request {
        url: "/b2".into(),
        ..read.clone()
    };
    let update_err = update(&root, "all.http#2", edited, Some(&read.source)).unwrap_err();
    let remove_err = remove(&root, "all.http#2", Some(&read.source)).unwrap_err();
    let move_err = move_to(&root, "all.http#2", "", None, Some(&read.source)).unwrap_err();

    for err in [update_err, remove_err, move_err] {
        assert_eq!(err.kind(), "conflict");
    }
    let text = fs::read_to_string(root.join("all.http")).unwrap();
    assert!(text.contains("GET /a") && text.contains("GET /b") && !text.contains("/b2"));
}

#[test]
fn update_returns_the_fresh_source_for_the_next_write() {
    let root = temp_workspace("update-source");
    write(&root, "a.http", "### A\nGET /a\n");
    let read = get(&root, "a.http").unwrap();

    let first = update(
        &root,
        "a.http",
        Request {
            url: "/1".into(),
            ..read.clone()
        },
        Some(&read.source),
    )
    .unwrap();
    let second = update(
        &root,
        "a.http",
        Request {
            url: "/2".into(),
            ..first.clone()
        },
        Some(&first.source),
    );

    assert_eq!(second.unwrap().url, "/2");
    assert_ne!(version(&read), version(&first));
}

#[test]
fn removing_shifts_the_ids_of_later_requests() {
    let root = temp_workspace("remove-ids");
    write(
        &root,
        "all.http",
        "### A\nGET /a\n### B\nGET /b\n### C\nGET /c\n",
    );

    let changes = remove(&root, "all.http", None).unwrap();

    let expect = |from: &str, to: Option<&str>| IdChange {
        from: from.into(),
        to: to.map(str::to_string),
    };
    assert_eq!(
        changes,
        [
            expect("all.http", None),
            expect("all.http#2", Some("all.http")),
            expect("all.http#3", Some("all.http#2"))
        ]
    );
    assert_eq!(get(&root, "all.http").unwrap().name, "B");
}

#[test]
fn splitting_reports_the_move_and_the_shift() {
    let root = temp_workspace("split-ids");
    write(
        &root,
        "all.http",
        "### A\nGET /a\n### B\nGET /b\n### C\nGET /c\n",
    );
    fs::create_dir(root.join("b")).unwrap();

    let moved = move_to(&root, "all.http#2", "b", None, None).unwrap();

    assert_eq!(moved.id, "b/b.http");
    assert_eq!(
        moved.changes[0],
        IdChange {
            from: "all.http#2".into(),
            to: Some("b/b.http".into())
        }
    );
    assert_eq!(
        moved.changes[1],
        IdChange {
            from: "all.http#3".into(),
            to: Some("all.http#2".into())
        }
    );
}

#[test]
fn moving_without_a_file_change_reports_nothing() {
    let root = temp_workspace("move-same");
    write(&root, "a.http", "GET /a\n");
    write(&root, "b.http", "GET /b\n");

    let moved = move_to(&root, "b.http", "", Some(0), None).unwrap();

    assert_eq!(moved.id, "b.http");
    assert!(moved.changes.is_empty());
}

#[test]
fn updating_a_later_request_without_a_separator_keeps_it_separate() {
    let root = temp_workspace("update-no-separator");
    write(&root, "all.http", "### One\nGET /1\n\n### Two\nGET /2\n");
    let replacement = http_file::parse("GET /changed\n").remove(0);

    let updated = update(&root, "all.http#2", replacement, None).unwrap();

    assert_eq!(updated.url, "/changed");
    assert_eq!(get(&root, "all.http").unwrap().url, "/1");
    assert_eq!(get(&root, "all.http#2").unwrap().url, "/changed");
}
