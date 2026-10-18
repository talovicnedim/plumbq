use super::*;
use crate::workspace::tests::{temp_workspace, write};

#[tokio::test]
async fn production_writes_need_confirmation() {
    let root = temp_workspace("send-prod");
    write(&root, "environments/prod.toml", "tag = \"production\"\n");
    let request = Request::new("Delete", "DELETE", "http://127.0.0.1:9/x");

    let result = send(
        &root,
        "x.http",
        &request,
        Some("prod"),
        &BTreeMap::new(),
        &SendContext::default(),
        false,
    )
    .await;
    assert!(matches!(result, Err(Error::NeedsConfirmation(_))));
}

#[tokio::test]
async fn method_override_headers_count_for_the_production_check() {
    let root = temp_workspace("send-prod-override");
    write(&root, "environments/prod.toml", "tag = \"production\"\n");
    write(
        &root,
        "admin/folder.toml",
        "[headers]\nX-Method-Override = \"PUT\"\n",
    );
    let send_get = |id: &'static str, header: Option<(&str, &str)>| {
        let mut request = Request::new("", "GET", "http://127.0.0.1:9/x");
        if let Some((name, value)) = header {
            request.headers.push(Header {
                name: name.into(),
                value: value.into(),
            });
        }
        let root = root.clone();
        async move {
            send(
                &root,
                id,
                &request,
                Some("prod"),
                &BTreeMap::new(),
                &SendContext::default(),
                false,
            )
            .await
        }
    };

    let overridden = send_get("x.http", Some(("x-http-method-override", "delete"))).await;
    assert!(
        matches!(&overridden, Err(Error::NeedsConfirmation(m)) if m.starts_with("DELETE")),
        "{overridden:?}"
    );
    let from_folder = send_get("admin/x.http", None).await;
    assert!(matches!(from_folder, Err(Error::NeedsConfirmation(_))));
    // A safe override of a safe method is still a read: it gets to the (closed) port.
    let read = send_get("x.http", Some(("X-HTTP-Method", "HEAD"))).await;
    assert!(matches!(read, Err(Error::Http(_))), "{read:?}");
}

#[test]
fn agent_sends_are_judged_by_the_effective_method() {
    let root = temp_workspace("send-agent-override");
    let mut request = Request::new("", "GET", "http://x.test/users/1");
    request.headers.push(Header {
        name: "X-HTTP-Method-Override".into(),
        value: "DELETE".into(),
    });

    let assessment = crate::guard::assess_agent_send(&root, "x.http", &request, None).unwrap();

    assert_eq!(assessment.method, "DELETE");
    assert!(!assessment.safe);
}

#[test]
fn an_empty_method_override_is_no_override() {
    let root = temp_workspace("send-empty-override");
    let mut request = Request::new("", "GET", "http://x.test/users/1");
    request.headers.push(Header {
        name: "X-HTTP-Method-Override".into(),
        value: " ".into(),
    });

    assert_eq!(effective_method(&root, "x.http", &request).unwrap(), "GET");
}

#[tokio::test]
async fn production_check_comes_before_oauth_and_prompts() {
    let root = temp_workspace("send-prod-first");
    write(&root, "environments/prod.toml", "tag = \"production\"\n");
    // Resolving either variable would fail differently (unknown OAuth client, missing answer).
    let request = Request::new(
        "",
        "POST",
        "http://127.0.0.1:9/{{$auth.token(\"api\")}}/{{$prompt otp}}",
    );
    let result = send(
        &root,
        "x.http",
        &request,
        Some("prod"),
        &BTreeMap::new(),
        &SendContext::default(),
        false,
    )
    .await;
    assert!(matches!(result, Err(Error::NeedsConfirmation(_))));
}
