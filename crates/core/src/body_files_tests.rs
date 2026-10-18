use super::*;
use crate::workspace::tests::{temp_workspace, write};

#[test]
fn plain_bodies_pass_through() {
    let body = load(
        Path::new("/ws"),
        "a.http",
        "{\"a\":1}",
        Some("application/json"),
        &Values::default(),
    )
    .unwrap();
    assert_eq!(body.bytes, b"{\"a\":1}");
    assert_eq!(body.description, "{\"a\":1}");
}

#[test]
fn reads_file_bodies_relative_to_the_request() {
    let root = temp_workspace("body-file");
    write(&root, "users/data/new.json", "{\"name\":\"Ada\"}");
    let body = load(
        &root,
        "users/create.http",
        "< ./data/new.json",
        Some("application/json"),
        &Values::default(),
    )
    .unwrap();
    assert_eq!(body.bytes, b"{\"name\":\"Ada\"}");
    assert_eq!(body.description, "<file ./data/new.json, 14 B>");

    let up = load(
        &root,
        "users/admin/create.http",
        "< ../data/new.json",
        None,
        &Values::default(),
    )
    .unwrap();
    assert_eq!(up.bytes, b"{\"name\":\"Ada\"}");
}

#[test]
fn multipart_bodies_get_crlf_and_file_bytes() {
    let root = temp_workspace("body-multipart");
    std::fs::write(root.join("pixel.bin"), [0u8, 159, 146, 150]).unwrap();
    let body = "--B\nContent-Disposition: form-data; name=\"f\"; filename=\"pixel.bin\"\n\n< ./pixel.bin\n--B--";

    let loaded = load(
        &root,
        "upload.http",
        body,
        Some("multipart/form-data; boundary=B"),
        &Values::default(),
    )
    .unwrap();

    let mut expected =
        b"--B\r\nContent-Disposition: form-data; name=\"f\"; filename=\"pixel.bin\"\r\n\r\n"
            .to_vec();
    expected.extend([0u8, 159, 146, 150]);
    expected.extend(b"\r\n--B--\r\n");
    assert_eq!(loaded.bytes, expected);
    assert!(loaded.description.contains("<file ./pixel.bin, 4 B>"));
}

#[test]
fn text_only_multipart_bodies_get_crlf_too() {
    let body = "--B\nContent-Disposition: form-data; name=\"title\"\n\nHoliday\n--B--";
    let loaded = load(
        Path::new("/ws"),
        "upload.http",
        body,
        Some("multipart/form-data; boundary=B"),
        &Values::default(),
    )
    .unwrap();

    assert_eq!(
        loaded.bytes,
        b"--B\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\nHoliday\r\n--B--\r\n"
    );
    assert_eq!(loaded.description, body);
}

#[test]
fn template_files_resolve_variables_and_plain_files_do_not() {
    let root = temp_workspace("body-template");
    write(&root, "data/user.json", "{\"name\":\"{{name}}\"}");
    let values = Values {
        variables: std::collections::BTreeMap::from([("name".into(), "Ada".into())]),
        ..Default::default()
    };

    let template = load(&root, "a.http", "<@ ./data/user.json", None, &values).unwrap();
    assert_eq!(template.bytes, b"{\"name\":\"Ada\"}");
    assert!(template.description.contains("with variables"));

    let plain = load(&root, "a.http", "< ./data/user.json", None, &values).unwrap();
    assert_eq!(plain.bytes, b"{\"name\":\"{{name}}\"}");

    let missing = load(
        &root,
        "a.http",
        "<@ ./data/user.json",
        None,
        &Values::default(),
    );
    assert!(
        missing
            .unwrap_err()
            .to_string()
            .contains("unknown variables: name")
    );
    assert_eq!(
        templates(&root, "a.http", "<@ ./data/user.json\n< ./x"),
        ["{\"name\":\"{{name}}\"}"]
    );
}

#[test]
fn templates_must_be_text() {
    let root = temp_workspace("body-template-binary");
    std::fs::write(root.join("pixel.bin"), [0u8, 159, 146, 150]).unwrap();
    let err = load(&root, "a.http", "<@ ./pixel.bin", None, &Values::default()).unwrap_err();
    assert!(err.to_string().contains("isn't text"));
}

#[cfg(unix)]
#[test]
fn symlinked_body_files_cannot_leave_the_workspace() {
    let root = temp_workspace("body-symlink");
    let outside = temp_workspace("body-symlink-outside");
    std::fs::write(outside.join("credentials"), "aws-secret").unwrap();
    std::os::unix::fs::symlink(outside.join("credentials"), root.join("data.json")).unwrap();

    let err = load(&root, "a.http", "< ./data.json", None, &Values::default()).unwrap_err();
    assert!(err.to_string().contains("outside the workspace"), "{err}");
}

#[test]
fn files_outside_the_workspace_are_refused() {
    let root = temp_workspace("body-escape");
    for path in ["../../etc/passwd", "/etc/passwd"] {
        let err = load(
            &root,
            "a.http",
            &format!("< {path}"),
            None,
            &Values::default(),
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("must be inside the workspace"),
            "{path}"
        );
    }
    let missing = load(&root, "a.http", "< ./nope.json", None, &Values::default()).unwrap_err();
    assert!(
        missing
            .to_string()
            .starts_with("failed to read body file ./nope.json")
    );
}

#[test]
fn only_file_references_written_in_the_body_load_files() {
    let root = temp_workspace("body-injected");
    write(&root, "secret.txt", "s3cr3t");
    write(&root, "data.json", "{}");
    let values = Values {
        variables: std::collections::BTreeMap::from([
            ("payload".into(), "x\n< ./secret.txt".into()),
            ("file".into(), "./data.json".into()),
        ]),
        ..Default::default()
    };

    let injected = load(&root, "a.http", "{{payload}}", None, &values).unwrap();
    assert_eq!(injected.bytes, b"x\n< ./secret.txt");
    let mixed = load(&root, "a.http", "< ./data.json\n{{payload}}", None, &values).unwrap();
    assert_eq!(mixed.bytes, b"{}\nx\n< ./secret.txt");

    let named = load(&root, "a.http", "< {{file}}", None, &values).unwrap();
    assert_eq!(named.bytes, b"{}");
}

#[test]
fn picked_files_are_stored_relative_to_the_request() {
    let root = temp_workspace("body-pick");
    write(&root, "users/data/a.json", "{}");
    write(&root, "files/photo.png", "");
    let path = |id: &str, file: &str| reference(&root, id, &root.join(file)).unwrap().path;

    assert_eq!(
        path("users/create.http", "users/data/a.json"),
        "./data/a.json"
    );
    assert_eq!(
        path("users/admin/x.http#2", "files/photo.png"),
        "../../files/photo.png"
    );
    assert_eq!(path("top.http", "files/photo.png"), "./files/photo.png");

    let picked = reference(&root, "users/create.http", &root.join("files/photo.png")).unwrap();
    assert_eq!(picked.body, "< ../files/photo.png");
    assert_eq!(picked.content_type, "image/png");
}

#[test]
fn picked_files_outside_the_workspace_are_refused() {
    let root = temp_workspace("body-pick-outside");
    let elsewhere = temp_workspace("body-pick-elsewhere");
    write(&elsewhere, "secret.txt", "");
    let err = reference(&root, "a.http", &elsewhere.join("secret.txt")).unwrap_err();
    assert!(err.to_string().contains("inside the workspace"), "{err}");
}
