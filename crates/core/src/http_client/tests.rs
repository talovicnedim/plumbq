use std::time::Duration;

use super::*;
use crate::request_settings::RequestSettings;
use crate::workspace::tests::{temp_workspace, write};

#[test]
fn request_directives_win_over_workspace_settings() {
    let workspace = HttpSettings {
        timeout_secs: Some(60),
        connect_timeout_secs: Some(5),
        follow_redirects: Some(true),
        verify_tls: Some(true),
        ..Default::default()
    };
    let request = RequestSettings {
        no_redirect: true,
        insecure: true,
        timeout_secs: Some(2),
        ..Default::default()
    };
    let options = options(Path::new("/ws"), &workspace, &request).unwrap();

    assert_eq!(options.timeout, Duration::from_secs(2));
    assert_eq!(options.connect_timeout, Some(Duration::from_secs(5)));
    assert!(!options.follow_redirects);
    assert!(!options.verify_tls);
}

#[test]
fn builds_with_a_pfx_identity_and_rejects_two_certificates() {
    let root = crate::workspace::tests::temp_workspace("pfx");
    std::fs::write(
        root.join("client.pfx"),
        crate::client_pfx::tests::test_pfx("pw"),
    )
    .unwrap();
    let base = options(&root, &HttpSettings::default(), &RequestSettings::default()).unwrap();
    let with_pfx = ClientOptions {
        client_pfx: Some(root.join("client.pfx")),
        client_pfx_password: "pw".into(),
        ..base
    };
    assert!(build(&with_pfx).is_ok());

    let wrong = ClientOptions {
        client_pfx_password: "nope".into(),
        ..with_pfx.clone()
    };
    assert!(
        build(&wrong)
            .unwrap_err()
            .to_string()
            .contains("wrong password")
    );

    let both = ClientOptions {
        client_cert: Some(root.join("client.pem")),
        ..with_pfx
    };
    assert!(build(&both).unwrap_err().to_string().contains("not both"));
}

#[test]
fn defaults_are_thirty_seconds_redirects_and_verified_tls() {
    let options = options(
        Path::new("/ws"),
        &HttpSettings::default(),
        &RequestSettings::default(),
    )
    .unwrap();
    assert_eq!(options.timeout, Duration::from_secs(30));
    assert!(options.follow_redirects && options.verify_tls);
    assert!(build(&options).is_ok());
}

#[test]
fn reads_settings_from_the_root_folder_file() {
    let root = temp_workspace("http-settings");
    write(
        &root,
        "folder.toml",
        "[settings]\ntimeoutSecs = 90\nproxy = \"socks5://127.0.0.1:1080\"\ncaCert = \"certs/ca.pem\"\n",
    );
    let settings = workspace_settings(&root).unwrap();
    assert_eq!(settings.timeout_secs, Some(90));

    let options = options(&root, &settings, &RequestSettings::default()).unwrap();
    assert_eq!(options.ca_cert, Some(root.join("certs/ca.pem")));
    assert!(build(&options).is_err(), "the CA file doesn't exist");
    let proxy_only = ClientOptions {
        ca_cert: None,
        ..options
    };
    assert!(build(&proxy_only).is_ok(), "socks5 proxies are supported");
}

#[test]
fn rejects_bad_proxies_and_escaping_paths() {
    let bad_proxy = HttpSettings {
        proxy: "::not a url".into(),
        ..Default::default()
    };
    let options = options(Path::new("/ws"), &bad_proxy, &RequestSettings::default()).unwrap();
    assert!(build(&options).is_err());

    let escaping = HttpSettings {
        ca_cert: "../ca.pem".into(),
        ..Default::default()
    };
    assert!(super::options(Path::new("/ws"), &escaping, &RequestSettings::default()).is_err());
}

#[test]
fn a_scheme_upgrade_on_the_same_host_counts_as_the_same_origin() {
    let url = |text: &str| reqwest::Url::parse(text).unwrap();
    let same = |from: &str, to: &str| same_origin(&url(from), &url(to));
    assert!(same("http://api.test/a", "https://api.test/b"));
    assert!(same("http://api.test:80/", "https://api.test:443/"));
    assert!(same("https://api.test/", "https://api.test/x"));
    assert!(
        !same("https://api.test/", "http://api.test/"),
        "a downgrade"
    );
    assert!(!same("http://api.test:8080/", "https://api.test:8443/"));
    assert!(!same("http://api.test/", "https://other.test/"));
}
