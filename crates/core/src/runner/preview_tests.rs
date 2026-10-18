use super::tests::workspace;
use super::*;
use crate::workspace::tests::write;

#[test]
fn only_writes_to_production_need_the_typed_name() {
    let root = workspace("run-preview", "http://x.test");
    let writes = |target: &str, env: Option<&str>| {
        preview(&root, target, &[], env).unwrap().production_writes
    };

    assert_eq!(writes("users", Some("prod")), 1);
    assert_eq!(writes("users/list.http", Some("prod")), 0, "a GET-only run");
    assert_eq!(writes("users", Some("local")), 0);
    assert_eq!(writes("users", None), 0);
}

#[test]
fn a_method_override_counts_as_a_write() {
    let root = workspace("run-preview-override", "http://x.test");
    write(
        &root,
        "users/sneaky.http",
        "### Sneaky\nGET {{base}}/users/1\nX-HTTP-Method-Override: DELETE\n",
    );
    let sneaky = preview(&root, "users/sneaky.http", &[], Some("prod")).unwrap();
    assert_eq!(sneaky.production_writes, 1);
}

#[test]
fn says_what_the_run_needs_first() {
    let root = workspace("run-preview-all", "http://x.test");
    write(
        &root,
        "otp.http",
        "### Otp\nGET {{base}}/otp?code={{$prompt code}}\n",
    );
    let all = preview(&root, "", &[], Some("local")).unwrap();
    assert_eq!(all.requests, 5);
    assert_eq!(all.prompts, ["code"]);
    assert_eq!(all.keep_outcomes as usize, KEEP_OUTCOMES);

    let smoke = preview(&root, "", &["smoke".into()], None).unwrap();
    assert_eq!((smoke.requests, smoke.prompts.len()), (1, 0));
}
