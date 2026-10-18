use super::*;
use crate::requests::IdChange;
use crate::send::SentRequest;
use crate::workspace::tests::temp_workspace;

fn response(status: u16) -> Response {
    Response {
        status,
        status_text: String::new(),
        headers: Vec::new(),
        body: "{}".into(),
        size: 2,
        binary: false,
        body_base64: String::new(),
        truncated: false,
        duration_ms: 12,
        sent: SentRequest {
            method: "GET".into(),
            url: "http://x.test/users".into(),
            headers: Vec::new(),
            body: String::new(),
        },
        assertions: Vec::new(),
        masked_secrets: Vec::new(),
    }
}

#[test]
fn add_list_get_newest_first() {
    let history = temp_workspace("history");
    let root = Path::new("/projects/api");

    let first = add(&history, root, "users/list.http", None, response(200)).unwrap();
    let second = add(
        &history,
        root,
        "users/list.http",
        Some("local"),
        response(500),
    )
    .unwrap();

    let rows = list(&history, root, "users/list.http").unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, second.id);
    assert_eq!(rows[0].status, 500);
    let entry = get(&history, root, "users/list.http", &first.id).unwrap();
    assert_eq!(entry.response.status, 200);
}

#[test]
fn keeps_a_bounded_number_of_entries() {
    let history = temp_workspace("history-prune");
    let root = Path::new("/p");
    for _ in 0..KEEP_PER_REQUEST + 3 {
        add(&history, root, "a.http", None, response(200)).unwrap();
    }
    assert_eq!(
        list(&history, root, "a.http").unwrap().len(),
        KEEP_PER_REQUEST
    );
}

#[test]
fn large_binary_bodies_are_returned_but_not_stored() {
    let history = temp_workspace("history-binary");
    let root = Path::new("/p");
    let mut big = response(200);
    big.binary = true;
    big.body_base64 = "A".repeat(MAX_BINARY_BASE64 + 1);

    let entry = add(&history, root, "a.http", None, big).unwrap();

    assert_eq!(entry.response.body_base64.len(), MAX_BINARY_BASE64 + 1);
    let stored = get(&history, root, "a.http", &entry.id).unwrap();
    assert!(stored.response.body_base64.is_empty());
    assert!(stored.response.binary);
}

#[test]
fn similar_ids_and_workspaces_get_separate_history() {
    let history = temp_workspace("history-collide");
    add(
        &history,
        Path::new("/p"),
        "users/list.http",
        None,
        response(200),
    )
    .unwrap();
    add(
        &history,
        Path::new("/p"),
        "users_list.http",
        None,
        response(500),
    )
    .unwrap();
    add(
        &history,
        Path::new("/my api"),
        "a.http",
        None,
        response(201),
    )
    .unwrap();

    assert_eq!(
        list(&history, Path::new("/p"), "users/list.http")
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        list(&history, Path::new("/p"), "users_list.http").unwrap()[0].status,
        500
    );
    assert!(
        list(&history, Path::new("/my_api"), "a.http")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn reads_history_from_the_old_directory_names() {
    let history = temp_workspace("history-legacy");
    let root = Path::new("/p");
    let legacy = history.join(flatten("/p")).join(flatten("a.http"));
    fs::create_dir_all(&legacy).unwrap();
    let old = add(&history, root, "a.http", None, response(418)).unwrap();
    // Move the entry where an older version would have written it.
    let current = request_dir(&history, root, "a.http").join(format!("{}.json", old.id));
    fs::rename(&current, legacy.join(format!("{}.json", old.id))).unwrap();

    assert_eq!(list(&history, root, "a.http").unwrap()[0].status, 418);
    assert_eq!(
        get(&history, root, "a.http", &old.id)
            .unwrap()
            .response
            .status,
        418
    );
}

#[test]
fn unreadable_entries_are_skipped() {
    let history = temp_workspace("history-corrupt");
    let root = Path::new("/p");
    add(&history, root, "a.http", None, response(200)).unwrap();
    let dir = request_dir(&history, root, "a.http");
    fs::write(dir.join("999999999999999999999.json"), "{\"cut off").unwrap();

    let rows = list(&history, root, "a.http").unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, 200);
}

#[test]
fn get_rejects_path_like_ids() {
    let history = temp_workspace("history-escape");
    assert!(get(&history, Path::new("/p"), "a.http", "../x").is_err());
}

#[test]
fn history_follows_moves_shifts_and_removals() {
    let history = temp_workspace("history-follow");
    let root = Path::new("/projects/follow");
    add(&history, root, "all.http#2", None, response(202)).unwrap();
    add(&history, root, "all.http#3", None, response(203)).unwrap();
    add(&history, root, "login.http", None, response(200)).unwrap();
    let change = |from: &str, to: Option<&str>| IdChange {
        from: from.into(),
        to: to.map(str::to_string),
    };

    // `#2` is removed and `#3` moves up; `login.http` moves into a folder.
    follow(
        &history,
        root,
        &[
            change("all.http#2", None),
            change("all.http#3", Some("all.http#2")),
            change("login.http", Some("auth/login.http")),
        ],
    )
    .unwrap();

    let shifted = list(&history, root, "all.http#2").unwrap();
    assert_eq!(shifted.len(), 1);
    assert_eq!(shifted[0].status, 203);
    assert!(list(&history, root, "all.http#3").unwrap().is_empty());
    let moved = list(&history, root, "auth/login.http").unwrap();
    let entry = get(&history, root, "auth/login.http", &moved[0].id).unwrap();
    assert_eq!(entry.request_id, "auth/login.http");
    assert!(list(&history, root, "login.http").unwrap().is_empty());
}

#[test]
fn latest_is_the_newest_entry_of_the_environment() {
    let history = temp_workspace("history-latest");
    let root = Path::new("/p");
    add(&history, root, "a.http", Some("local"), response(201)).unwrap();
    add(&history, root, "a.http", Some("local"), response(202)).unwrap();
    add(&history, root, "a.http", Some("prod"), response(500)).unwrap();

    let local = latest(&history, root, "a.http", Some("local")).unwrap();
    assert_eq!(local.map(|e| e.response.status), Some(202));
    assert!(latest(&history, root, "a.http", None).unwrap().is_none());
}

#[test]
fn long_text_bodies_are_cut_in_history() {
    let history = temp_workspace("history-cut");
    let root = Path::new("/p");
    let mut big = response(200);
    big.body = format!("é{}", "x".repeat(MAX_TEXT_BODY));

    let returned = add(&history, root, "a.http", None, big).unwrap();

    assert_eq!(returned.response.body.len(), MAX_TEXT_BODY + 2);
    let stored = get(&history, root, "a.http", &returned.id)
        .unwrap()
        .response;
    assert!(stored.truncated);
    assert!(stored.body.len() <= MAX_TEXT_BODY);
    assert!(stored.body.starts_with('é'));
}

#[cfg(unix)]
#[test]
fn moved_history_stays_private() {
    use std::os::unix::fs::PermissionsExt;
    let history = temp_workspace("history-follow-private");
    let root = Path::new("/projects/private");
    add(&history, root, "login.http", None, response(200)).unwrap();

    let change = IdChange {
        from: "login.http".into(),
        to: Some("auth/login.http".into()),
    };
    follow(&history, root, &[change]).unwrap();

    let dir = request_dir(&history, root, "auth/login.http");
    let mode = std::fs::metadata(dir).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700);
}
