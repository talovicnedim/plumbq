use super::*;
use crate::workspace::tests::temp_workspace;

const NOW: i64 = 1_800_000_000;

fn url(text: &str) -> Url {
    Url::parse(text).unwrap()
}

fn cookie(set_cookie: &str, from: &str) -> Cookie {
    parse(set_cookie, &url(from), NOW).unwrap()
}

#[test]
fn parses_attributes() {
    let c = cookie(
        "sid=abc; Path=/api; Domain=.Example.com; Secure; HttpOnly; SameSite=Lax",
        "https://app.example.com/login",
    );
    assert_eq!(c.name, "sid");
    assert_eq!(c.value, "abc");
    assert_eq!(c.domain, "example.com");
    assert!(!c.host_only);
    assert_eq!(c.path, "/api");
    assert!(c.secure && c.http_only);
    assert_eq!(c.expires, None);
}

#[test]
fn defaults_to_host_only_and_the_request_directory() {
    let c = cookie("a=1", "http://api.test/v1/users/7");
    assert!(c.host_only);
    assert_eq!(c.domain, "api.test");
    assert_eq!(c.path, "/v1/users");
    assert_eq!(cookie("a=1", "http://api.test/login").path, "/");
}

#[test]
fn max_age_wins_over_expires() {
    let c = cookie(
        "a=1; Expires=Wed, 21 Oct 2015 07:28:00 GMT; Max-Age=60",
        "http://x.test/",
    );
    assert_eq!(c.expires.as_deref(), Some("2027-01-15T08:01:00.000Z"));
    let dated = cookie(
        "a=1; Expires=Wed, 21-Oct-2015 07:28:00 GMT",
        "http://x.test/",
    );
    assert_eq!(dated.expires.as_deref(), Some("2015-10-21T07:28:00.000Z"));
}

#[test]
fn ignores_invalid_lines_and_foreign_domains() {
    assert!(parse("no-equals-sign", &url("http://x.test/"), NOW).is_none());
    assert!(parse("=value", &url("http://x.test/"), NOW).is_none());
    assert!(parse("a=1; Domain=other.test", &url("http://x.test/"), NOW).is_none());
    assert!(parse("a=1; Domain=t.test", &url("http://not.test/"), NOW).is_none());
}

#[test]
fn matches_domains_paths_and_secure() {
    let jar = vec![
        cookie("host=1", "http://example.com/"),
        cookie("domain=2; Domain=example.com", "http://example.com/"),
        cookie("deep=3; Path=/api/v1", "http://example.com/"),
        cookie("tls=4; Secure", "https://example.com/"),
    ];
    assert_eq!(
        header_for(&jar, &url("http://example.com/api/v1/users"), NOW).unwrap(),
        "deep=3; host=1; domain=2"
    );
    assert_eq!(
        header_for(&jar, &url("http://www.example.com/"), NOW).unwrap(),
        "domain=2"
    );
    assert!(
        header_for(&jar, &url("https://example.com/"), NOW)
            .unwrap()
            .contains("tls=4")
    );
    assert_eq!(
        header_for(&jar, &url("http://example.com/api/v10"), NOW).unwrap(),
        "host=1; domain=2"
    );
    assert!(header_for(&jar, &url("http://other.test/"), NOW).is_none());
}

#[test]
fn ip_hosts_match_only_exactly() {
    let jar = vec![cookie("a=1", "http://127.0.0.1:8080/")];
    assert_eq!(
        header_for(&jar, &url("http://127.0.0.1:9/"), NOW).unwrap(),
        "a=1"
    );
    assert!(!domain_matches("10.0.0.1", "0.0.1"));
}

#[test]
fn store_replaces_and_expired_cookies_delete() {
    let mut jar = Vec::new();
    store(&mut jar, cookie("a=1", "http://x.test/"), NOW);
    store(&mut jar, cookie("a=2", "http://x.test/"), NOW);
    assert_eq!(jar.len(), 1);
    assert_eq!(jar[0].value, "2");

    store(&mut jar, cookie("a=gone; Max-Age=0", "http://x.test/"), NOW);
    assert!(jar.is_empty());

    let old = cookie("old=1; Max-Age=10", "http://x.test/");
    assert!(header_for(&[old], &url("http://x.test/"), NOW + 11).is_none());
}

#[test]
fn jars_are_separate_per_environment_and_persist() {
    let dir = temp_workspace("cookies");
    let root = Path::new("/projects/api");
    let jar = vec![cookie("sid=staging", "http://x.test/")];

    save(&dir, root, Some("staging"), &jar).unwrap();

    assert_eq!(list(&dir, root, Some("staging")).unwrap(), jar);
    assert!(list(&dir, root, Some("production")).unwrap().is_empty());
    assert!(list(&dir, root, None).unwrap().is_empty());

    remove(&dir, root, Some("staging"), "x.test", "sid").unwrap();
    assert!(list(&dir, root, Some("staging")).unwrap().is_empty());

    save(&dir, root, None, &jar).unwrap();
    clear(&dir, root, None).unwrap();
    assert!(list(&dir, root, None).unwrap().is_empty());
}

#[test]
fn rejects_public_suffix_domains() {
    let from = url("https://api.example.co.uk/login");
    assert!(parse("a=1; Domain=co.uk", &from, 0).is_none());
    assert!(parse("a=1; Domain=uk", &from, 0).is_none());
    assert!(parse("a=1; Domain=.com", &url("https://api.example.com/"), 0).is_none());
    assert!(parse("a=1; Domain=example.co.uk", &from, 0).is_some());
    // A host that is itself on the list (e.g. github.io pages) may set a host cookie.
    let page = url("https://github.io/");
    assert!(parse("a=1; Domain=github.io", &page, 0).is_some());
}

#[test]
fn huge_max_age_and_expires_do_not_overflow() {
    let from = url("https://x.test/");
    let forever = parse("a=1; Max-Age=9223372036854775807", &from, 100).unwrap();
    assert_eq!(forever.expires.as_deref(), Some("2106-02-07T06:28:15.000Z"));
    let far = parse(
        "a=1; Expires=Wed, 21 Oct 99999999999999 07:28:00 GMT",
        &from,
        0,
    )
    .unwrap();
    assert_eq!(far.expires, None);
    let bad_clock = parse("a=1; Expires=Wed, 21 Oct 2030 99:99:99 GMT", &from, 0).unwrap();
    assert_eq!(bad_clock.expires, None);
}

#[test]
fn an_unreadable_jar_counts_as_empty() {
    let dir = temp_workspace("cookie-corrupt");
    let root = Path::new("/w");
    save(&dir, root, Some("dev"), &[cookie("a=1", "https://x.test/")]).unwrap();
    let jar_file = jar_path(&dir, root, Some("dev"));
    std::fs::write(&jar_file, "[{\"name\": \"cut").unwrap();

    assert!(list(&dir, root, Some("dev")).unwrap().is_empty());
}

#[test]
fn reads_and_replaces_jars_at_the_old_location() {
    let dir = temp_workspace("cookie-legacy");
    let root = Path::new("/w");
    let legacy = legacy_jar_path(&dir, root, Some("dev"));
    std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    let old = vec![cookie("a=1", "https://x.test/")];
    std::fs::write(&legacy, serde_json::to_string(&old).unwrap()).unwrap();

    assert_eq!(list(&dir, root, Some("dev")).unwrap().len(), 1);
    save(&dir, root, Some("dev"), &old).unwrap();
    assert!(!legacy.exists());
    assert_eq!(list(&dir, root, Some("dev")).unwrap().len(), 1);
}

#[test]
fn a_full_jar_drops_its_oldest_cookie() {
    let mut jar = Vec::new();
    for n in 0..=KEEP {
        store(&mut jar, cookie(&format!("c{n}=1"), "https://api.test/"), 0);
    }
    assert_eq!(jar.len(), KEEP);
    assert_eq!(jar[0].name, "c1");
}

#[test]
fn expiry_is_rfc3339_and_jars_with_unix_seconds_still_load() {
    let dir = temp_workspace("cookie-unix-expiry");
    let root = Path::new("/w");
    let jar_file = jar_path(&dir, root, None);
    std::fs::create_dir_all(jar_file.parent().unwrap()).unwrap();
    let old = r#"[{"name":"sid","value":"1","domain":"x.test","hostOnly":true,"path":"/",
        "expires":4102444800,"secure":false,"httpOnly":false},
        {"name":"s","value":"2","domain":"x.test","hostOnly":true,"path":"/",
        "expires":null,"secure":false,"httpOnly":false}]"#;
    std::fs::write(&jar_file, old).unwrap();

    let jar = list(&dir, root, None).unwrap();

    assert_eq!(jar.len(), 2);
    assert_eq!(jar[0].expires.as_deref(), Some("2100-01-01T00:00:00.000Z"));
    assert_eq!(jar[1].expires, None);
    save(&dir, root, None, &jar).unwrap();
    let saved = std::fs::read_to_string(&jar_file).unwrap();
    assert!(
        saved.contains(r#""expires": "2100-01-01T00:00:00.000Z""#),
        "{saved}"
    );
    assert_eq!(list(&dir, root, None).unwrap(), jar);
}

#[test]
fn unreadable_expiries_count_as_past() {
    let mut cookie = cookie("a=1", "https://x.test/");
    cookie.expires = Some("soon".into());
    assert!(is_expired(&cookie, NOW));
}
