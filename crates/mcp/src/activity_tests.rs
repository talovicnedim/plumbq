use super::*;

fn temp_workspace(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "ac-test-activity-{name}-{}",
        &crate::settings::new_token()[..12]
    ))
}

fn entry(summary: &str) -> ActivityEntry {
    ActivityEntry {
        at: plumbq_core::time::now_rfc3339(),
        actor: "claude-code".into(),
        from_chat: false,
        workspace: "/ws".into(),
        tool: "requests_send".into(),
        environment: None,
        permission: Permission::SendSafe,
        outcome: ActivityOutcome::Ran,
        summary: summary.into(),
    }
}

#[test]
fn logs_from_before_the_outcome_rename_still_load() {
    let dir = temp_workspace("old");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        path(&dir),
        r#"{"at":"2026-10-01T10:00:00.000Z","actor":"claude-code","workspace":"/ws","tool":"workspace_tree","permission":"readWorkspace","outcome":"allowed","summary":"tree"}
"#,
    )
    .unwrap();
    let entries = list(&dir, 10).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].outcome, ActivityOutcome::Ran);
    assert_eq!(entries[0].environment, None);
}

#[test]
fn keeps_the_newest_thousand() {
    let dir = temp_workspace("cap");
    let calls = KEEP * 3;
    for i in 0..calls {
        record(&dir, entry(&format!("call {i} {}", "é".repeat(280)))).unwrap();
    }
    let entries = list(&dir, u32::MAX).unwrap();
    assert_eq!(entries.len(), KEEP);
    let newest = (calls - 1).to_string();
    assert_eq!(entries[0].summary.split(' ').nth(1), Some(newest.as_str()));
    let lines = fs::read_to_string(path(&dir)).unwrap().lines().count();
    assert!(lines < calls, "the file was trimmed: {lines} lines");
}

#[cfg(unix)]
#[test]
fn a_log_under_the_entry_limit_is_appended_to_not_rewritten() {
    use std::os::unix::fs::MetadataExt;
    let dir = temp_workspace("append");
    let long = "é".repeat(MAX_CHARS);
    for _ in 0..KEEP - 200 {
        record(&dir, entry(&long)).unwrap();
    }
    let before = fs::metadata(path(&dir)).unwrap();
    assert!(
        before.len() > 500_000,
        "long entries: {} bytes",
        before.len()
    );
    record(&dir, entry("one more")).unwrap();
    let after = fs::metadata(path(&dir)).unwrap();
    assert_eq!(after.ino(), before.ino(), "the file was rewritten");
    assert_eq!(list(&dir, u32::MAX).unwrap().len(), KEEP - 199);
}

#[test]
fn a_huge_entry_never_erases_the_log() {
    let dir = temp_workspace("huge-field");
    for i in 0..10 {
        record(&dir, entry(&format!("call {i}"))).unwrap();
    }
    let huge = ActivityEntry {
        actor: "x".repeat(TRIM_ABOVE_BYTES as usize + 1),
        workspace: "w".repeat(10_000),
        tool: "t".repeat(10_000),
        environment: Some("e".repeat(10_000)),
        ..entry("from a client with a very long name")
    };
    record(&dir, huge).unwrap();
    let entries = list(&dir, u32::MAX).unwrap();
    assert_eq!(entries.len(), 11, "older entries survive");
    assert!(entries[0].actor.chars().count() <= MAX_CHARS + 1);
    assert!(entries[0].workspace.chars().count() <= MAX_CHARS + 1);
    assert!(fs::metadata(path(&dir)).unwrap().len() < 20_000);
}

#[test]
fn a_trim_skips_oversized_lines_and_keeps_the_newest_entries() {
    let dir = temp_workspace("huge-line");
    for i in 0..10 {
        record(&dir, entry(&format!("call {i}"))).unwrap();
    }
    let junk = format!("{}\n", "j".repeat(TRIM_ABOVE_BYTES as usize));
    fs_util::append_private(&path(&dir), &junk).unwrap();
    record(&dir, entry("after the junk")).unwrap();
    let entries = list(&dir, u32::MAX).unwrap();
    assert_eq!(entries.len(), 11);
    assert_eq!(entries[0].summary, "after the junk");
    assert_eq!(entries[10].summary, "call 0");
    assert!(
        fs::metadata(path(&dir)).unwrap().len() < 20_000,
        "the junk is gone"
    );
}

#[test]
fn a_trim_keeps_the_newest_entries_even_when_they_are_long() {
    let dir = temp_workspace("min-keep");
    let long = serde_json::to_string(&entry(&"l".repeat(15_000))).unwrap();
    for _ in 0..MIN_KEEP + 50 {
        fs_util::append_private(&path(&dir), &long).unwrap();
    }
    record(&dir, entry("newest")).unwrap();
    let entries = list(&dir, u32::MAX).unwrap();
    assert_eq!(entries.len(), MIN_KEEP);
    assert_eq!(entries[0].summary, "newest");
}

#[test]
fn records_newest_first_and_redacts_credentials() {
    let dir = temp_workspace("activity");
    record(&dir, entry("first")).unwrap();
    record(
        &dir,
        entry("send GET https://x.test/?api_key=abc123 with Bearer eyJhbGciOiJIUzI1NiJ9.e30"),
    )
    .unwrap();
    let entries = list(&dir, 10).unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries[0].summary.starts_with("send GET"));
    let raw = fs::read_to_string(path(&dir)).unwrap();
    assert!(!raw.contains("eyJhbGci"), "{raw}");
    assert!(raw.contains("Bearer •••"), "{raw}");
    clear(&dir).unwrap();
    assert!(list(&dir, 10).unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn the_log_is_private_from_the_first_line() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_workspace("private-log").join("data");
    record(&dir, entry("first")).unwrap();
    let mode = fs::metadata(path(&dir)).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn redaction_keeps_placeholders_and_plain_words() {
    assert_eq!(
        redact("token={{token}} name=ada"),
        "token={{token}} name=ada"
    );
    assert_eq!(redact("password=hunter2"), "password=•••");
    assert_eq!(redact("x 0123456789abcdef0123456789abcdef y"), "x ••• y");
}

#[test]
fn summaries_are_one_short_line() {
    let long = format!("{}\nsecond line", "a".repeat(500));
    assert_eq!(cut(&long).chars().count(), MAX_CHARS + 1);
    assert!(!cut(&long).contains("second"));
}
