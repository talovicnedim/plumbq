use super::*;

#[test]
fn parses_full_request() {
    let text = "### Create user\n\
                # @assert status == 201\n\
                POST {{baseUrl}}/users HTTP/1.1\n\
                Content-Type: application/json\n\
                Authorization: Bearer {{token}}\n\
                \n\
                {\"name\": \"Ada\"}\n";
    let requests = parse(text);

    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.name, "Create user");
    assert_eq!(request.method, "POST");
    assert_eq!(request.url, "{{baseUrl}}/users");
    assert_eq!(request.headers.len(), 2);
    assert_eq!(request.headers[1].value, "Bearer {{token}}");
    assert_eq!(request.body, "{\"name\": \"Ada\"}");
    assert_eq!(request.assertions, ["status == 201"]);
}

#[test]
fn assertions_after_the_request_line_count() {
    let requests = parse("GET /users/1\n# @assert status == 200\nAccept: */*\n");
    assert_eq!(requests[0].assertions, ["status == 200"]);
    assert_eq!(requests[0].headers.len(), 1);
}

#[test]
fn bare_url_is_get() {
    let requests = parse("https://example.com/health\n");
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].url, "https://example.com/health");
    assert_eq!(requests[0].name, "");
}

#[test]
fn splits_multiple_requests() {
    let text = "### One\nGET /a\n\n### Two\nDELETE /b\n";
    let names: Vec<_> = parse(text).into_iter().map(|r| r.name).collect();
    assert_eq!(names, ["One", "Two"]);
}

#[test]
fn name_directive_used_when_separator_has_no_title() {
    let requests = parse("###\n# @name login\nPOST /login\n");
    assert_eq!(requests[0].name, "login");
}

#[test]
fn joins_multiline_query() {
    let requests = parse("GET /search\n    ?q=rust\n    &page=2\nAccept: */*\n");
    assert_eq!(requests[0].url, "/search?q=rust&page=2");
    assert_eq!(requests[0].headers[0].name, "Accept");
}

#[test]
fn keeps_comments_and_response_handlers() {
    let text = "### Login\n# Logs in as the admin\n// @no-log\nPOST /login\n\n{}\n\n\
                > {% client.global.set(\"token\", response.body.token); %}\n";
    let request = &parse(text)[0];
    assert_eq!(request.comments, ["# Logs in as the admin", "// @no-log"]);
    assert_eq!(request.body, "{}");
    assert!(request.trailer.starts_with("> {%"));
}

#[test]
fn skips_comment_only_preamble() {
    let requests = parse("# Shared notes\n\n### Real\nGET /x\n");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].name, "Real");
}

#[test]
fn round_trips() {
    let text = "### Create user\n\
                # Needs admin\n\
                # @assert status == 201\n\
                POST /users\n\
                Content-Type: application/json\n\
                \n\
                {\"name\": \"Ada\"}\n\
                \n\
                > ./handler.js\n";
    assert_eq!(serialize(&parse(text)), text);
}

#[test]
fn serializes_untitled_request_without_trailing_space() {
    let text = serialize(&[Request::new("", "GET", "/x")]);
    assert_eq!(text, "###\nGET /x\n");
}

#[test]
fn reads_file_variables_before_and_between_requests() {
    let text = "@host = api.test\n@version=v2\n\n### One\nGET https://{{host}}/{{version}}/a\n\n\
                ### Two\n@id = 7\nDELETE https://{{host}}/items/{{id}}\n";
    let requests = parse(text);

    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].prefix, "@host = api.test\n@version=v2");
    assert_eq!(
        requests[1].variables,
        [FileVariable {
            name: "id".into(),
            value: "7".into()
        }]
    );
    let names: Vec<_> = file_variables(&requests)
        .into_iter()
        .map(|v| v.name)
        .collect();
    assert_eq!(names, ["host", "version", "id"]);
}

#[test]
fn file_variables_without_separator_are_not_a_request_line() {
    let requests = parse("@base = http://x.test\nGET {{base}}/a\n");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url, "{{base}}/a");
    assert_eq!(requests[0].variables[0].value, "http://x.test");
}

#[test]
fn round_trips_prefix_variables_and_settings() {
    let text = "# Shared notes\n@host = api.test\n\n### One\n@id = 7\n# @no-redirect\n# @timeout 60\n\
                GET https://{{host}}/items/{{id}}\n\n### Two\nPURGE https://{{host}}/cache\n";
    let requests = parse(text);
    assert!(requests[0].settings.no_redirect);
    assert_eq!(requests[0].settings.timeout_secs, Some(60));
    assert_eq!(requests[1].method, "PURGE");
    assert_eq!(serialize(&requests), text);
}

#[test]
fn custom_methods_are_tokens_in_capitals() {
    assert_eq!(parse("PROPFIND /dav/\n")[0].method, "PROPFIND");
    let lower = parse("example.com/x y\n");
    assert_eq!(lower[0].method, "GET");
}

#[test]
fn validate_refuses_requests_that_would_read_back_differently() {
    let with = |edit: fn(&mut Request)| {
        let mut request = Request::new("A", "POST", "/1");
        edit(&mut request);
        request
    };
    let refused = [
        with(|r| r.method = "post".into()),
        with(|r| {
            r.method = "GO".into();
            r.url.clear();
        }),
        with(|r| r.comments.push("not a comment".into())),
        with(|r| {
            r.variables.push(FileVariable {
                name: "a b".into(),
                value: "1".into(),
            })
        }),
        with(|r| {
            r.headers.push(Header {
                name: "#X".into(),
                value: "1".into(),
            })
        }),
    ];
    for request in refused {
        assert!(validate(&request).is_err(), "{request:?}");
    }
    let lone = with(|r| r.url.clear());
    let kept = [lone, with(|r| r.comments.push("// note".into()))];
    for request in kept {
        assert!(validate(&request).is_ok(), "{request:?}");
    }
}
