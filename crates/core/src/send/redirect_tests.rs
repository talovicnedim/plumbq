use super::mask::cross_origin_redirects;
use super::tests::{scripted_server, secret_values};
use super::*;
use crate::http_client;
use crate::workspace::tests::{temp_workspace, write};

fn redirect_to(location: &str) -> String {
    format!(
        "HTTP/1.1 302 Found\r\nlocation: {location}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
    )
}

fn ok_with(extra_headers: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\n{extra_headers}content-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[tokio::test]
async fn cookies_from_a_redirect_target_belong_to_that_host() {
    let root = temp_workspace("send-redirect-cookie");
    let target = scripted_server(vec![ok_with(
        "set-cookie: sid=evil; Domain=127.0.0.1\r\nset-cookie: own=1\r\n",
        "ok",
    )])
    .await
    .replace("127.0.0.1", "localhost");
    let start = scripted_server(vec![redirect_to(&target)]).await;
    let context = SendContext {
        cookie_dir: Some(temp_workspace("send-redirect-jar")),
        ..Default::default()
    };
    let request = Request::new("", "GET", &start);

    send(
        &root,
        "x.http",
        &request,
        None,
        &BTreeMap::new(),
        &context,
        false,
    )
    .await
    .unwrap();

    let jar = crate::cookies::list(context.cookie_dir.as_deref().unwrap(), &root, None).unwrap();
    assert_eq!(jar.len(), 1, "{jar:?}");
    assert_eq!(jar[0].name, "own");
    assert_eq!(jar[0].domain, "localhost");
}

#[tokio::test]
async fn secret_headers_do_not_follow_redirects_to_another_origin() {
    let root = temp_workspace("send-redirect-secret");
    let other = scripted_server(vec![ok_with("", "{echo}")]).await;
    let start = scripted_server(vec![redirect_to(&other), redirect_to(&other)]).await;
    let mut request = Request::new("", "GET", &start);
    request.headers.push(Header {
        name: "X-Api-Key".into(),
        value: "{{token}}".into(),
    });
    let values = secret_values("s3cr3t-key");
    let sent = prepare(&root, "x.http", &request, &values).unwrap();
    let mut options = http_client::options(
        &root,
        &http_client::HttpSettings::default(),
        &request.settings,
    )
    .unwrap();

    options.cross_origin_redirects = cross_origin_redirects(&sent.headers, &[], &values);
    let client = http_client::build(&options).unwrap();
    let (response, final_url) = execute_with(&client, &sent, Vec::new()).await.unwrap();
    assert_eq!(response.status, 302);
    assert!(final_url.starts_with(&start));

    // Without secrets in headers the redirect is followed as usual.
    let plain = Request::new("", "GET", &start);
    let sent = prepare(&root, "x.http", &plain, &values).unwrap();
    options.cross_origin_redirects = cross_origin_redirects(&sent.headers, &[], &values);
    let client = http_client::build(&options).unwrap();
    let (response, _) = execute_with(&client, &sent, Vec::new()).await.unwrap();
    assert_eq!(response.status, 200);
}

fn moved_to(status: &str, location: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nlocation: {location}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
    )
}

#[tokio::test]
async fn secret_bodies_are_not_resent_to_another_origin() {
    let root = temp_workspace("send-redirect-body");
    let values = secret_values("s3cr3t-key");
    let post = |url: &str| {
        let mut request = Request::new("", "POST", url);
        request.body = "{\"key\":\"{{token}}\"}".into();
        request
    };
    let status_after = |status: &'static str| {
        let (root, values) = (root.clone(), values.clone());
        async move {
            let other = scripted_server(vec![ok_with("", "{echo}")]).await;
            let start = scripted_server(vec![moved_to(status, &other)]).await;
            let Wire {
                prepared,
                body,
                client,
                ..
            } = wire(&root, "x.http", &post(&start), None, None, &values, false).unwrap();
            execute_with(&client, &prepared, body)
                .await
                .unwrap()
                .0
                .status
        }
    };

    assert_eq!(status_after("307 Temporary Redirect").await, 307);
    assert_eq!(status_after("308 Permanent Redirect").await, 308);
    // These turn the POST into a GET without a body, so nothing secret goes along.
    assert_eq!(status_after("302 Found").await, 200);
}

#[tokio::test]
async fn agent_sends_never_follow_redirects_to_private_addresses() {
    let root = temp_workspace("send-redirect-agent");
    let target = scripted_server(vec![ok_with("", "reached")])
        .await
        .replace("127.0.0.1", "localhost");
    let start = scripted_server(vec![redirect_to(&target), redirect_to(&target)]).await;
    let request = Request::new("", "GET", &start);
    let agent = SendContext {
        agent: true,
        ..Default::default()
    };
    let none = BTreeMap::new();

    let user = SendContext::default();

    let stopped = send(&root, "x.http", &request, None, &none, &agent, false).await;
    assert_eq!(stopped.unwrap().status, 302);
    let followed = send(&root, "x.http", &request, None, &none, &user, false).await;
    assert_eq!(followed.unwrap().status, 200);
}

#[tokio::test]
async fn agent_clients_never_reach_a_private_address_through_a_public_name() {
    let root = temp_workspace("send-agent-dns");
    let target = scripted_server(vec![ok_with("", "reached")])
        .await
        .replace("127.0.0.1", "public.example.com");
    let start = scripted_server(vec![redirect_to(&target), redirect_to(&target)])
        .await
        .replace("127.0.0.1", "first.example.com");
    write(
        &root,
        "environments/local.toml",
        &format!("tag = \"local\"\n[variables]\nbaseUrl = \"{start}\"\n"),
    );
    let request = Request::new("", "GET", &start);
    let loopback = std::net::IpAddr::from([127, 0, 0, 1]);
    let mut options = http_client::options(
        &root,
        &http_client::HttpSettings::default(),
        &request.settings,
    )
    .unwrap();
    let rules = crate::guard::AgentRedirects::for_sends(&root, &start)
        .unwrap()
        .with_fixed("first.example.com", loopback)
        .with_fixed("public.example.com", loopback);

    options.agent_redirects = Some(rules);
    let client = http_client::build(&options).unwrap();
    let err = client.get(&start).send().await.unwrap_err();
    assert!(describe(err).contains("public.example.com"));

    let first = crate::guard::AgentRedirects::for_sends(&root, &target)
        .unwrap()
        .with_fixed("public.example.com", loopback);
    options.agent_redirects = Some(first);
    let client = http_client::build(&options).unwrap();
    let err = client.get(&target).send().await.unwrap_err();
    assert!(
        describe(err).contains("public.example.com"),
        "a first host no environment points at is checked, approved or not"
    );
}

#[tokio::test]
async fn agent_redirects_between_the_users_own_dev_names_work() {
    let root = temp_workspace("send-agent-dev-names");
    let target = scripted_server(vec![ok_with("", "reached")])
        .await
        .replace("127.0.0.1", "auth.myapp.test");
    let start = scripted_server(vec![redirect_to(&target)])
        .await
        .replace("127.0.0.1", "api.myapp.test");
    write(
        &root,
        "environments/local.toml",
        &format!("tag = \"local\"\n[variables]\nbaseUrl = \"{start}\"\nauthUrl = \"{target}\"\n"),
    );
    let request = Request::new("", "GET", &start);
    let loopback = std::net::IpAddr::from([127, 0, 0, 1]);
    let mut options = http_client::options(
        &root,
        &http_client::HttpSettings::default(),
        &request.settings,
    )
    .unwrap();
    options.agent_redirects = Some(
        crate::guard::AgentRedirects::for_sends(&root, &start)
            .unwrap()
            .with_fixed("api.myapp.test", loopback)
            .with_fixed("auth.myapp.test", loopback),
    );
    let client = http_client::build(&options).unwrap();
    let reached = client.get(&start).send().await.unwrap();
    assert_eq!(reached.status(), 200);
}

#[tokio::test]
async fn a_literal_test_host_that_resolves_to_this_machine_works_for_agent_sends() {
    let root = temp_workspace("send-agent-test-host");
    let start = scripted_server(vec![ok_with("", "reached")])
        .await
        .replace("127.0.0.1", "myapp.test");
    let request = Request::new("", "GET", &start);
    let mut options = http_client::options(
        &root,
        &http_client::HttpSettings::default(),
        &request.settings,
    )
    .unwrap();
    options.agent_redirects = Some(
        crate::guard::AgentRedirects::for_sends(&root, &start)
            .unwrap()
            .with_fixed("myapp.test", std::net::IpAddr::from([127, 0, 0, 1])),
    );
    let client = http_client::build(&options).unwrap();
    let reached = client.get(&start).send().await.unwrap();
    assert_eq!(reached.status(), 200);
}

#[tokio::test]
async fn agent_sends_without_a_checkable_host_are_refused() {
    let root = temp_workspace("send-agent-no-host");
    let agent = SendContext {
        agent: true,
        ..Default::default()
    };
    let request = Request::new("", "GET", "http://[::1/users");
    let err = send(
        &root,
        "x.http",
        &request,
        None,
        &BTreeMap::new(),
        &agent,
        false,
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("host the app can check"), "{err}");
}
