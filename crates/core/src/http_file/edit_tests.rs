use super::*;

/// Parses `text`, applies `edit` to request `index`, and writes the file back.
fn edited(text: &str, index: usize, edit: impl FnOnce(&mut Request)) -> String {
    let mut requests = parse(text);
    edit(&mut requests[index]);
    serialize(&requests)
}

#[test]
fn keeps_a_bom_out_of_the_request_and_writes_it_back() {
    let text =
        "\u{feff}### Login\nPOST https://x.test/login\nContent-Type: application/json\n\n{}\n";
    let requests = parse(text);
    assert_eq!(requests[0].name, "Login");
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].url, "https://x.test/login");
    assert_eq!(serialize(&requests), text);

    let renamed = edited(text, 0, |r| r.name = "Sign in".into());
    assert_eq!(
        renamed,
        "\u{feff}### Sign in\nPOST https://x.test/login\nContent-Type: application/json\n\n{}\n"
    );
}

#[test]
fn line_breaks_in_single_line_fields_never_inject_requests() {
    let mut request = Request::new("Health", "GET", "https://api.test/health");
    request.headers.push(Header {
        name: "X-Note".into(),
        value: "a\n\n### Wipe\nDELETE https://api.test/users".into(),
    });
    assert!(validate(&request).is_err());

    let written = serialize(&[request]);
    let back = parse(&written);
    assert_eq!(back.len(), 1, "{written}");
    assert_eq!(back[0].method, "GET");

    assert!(validate(&Request::new("ok", "GET", "/x")).is_ok());
    assert!(validate(&Request::new("a\nb", "GET", "/x")).is_err());
    assert!(validate(&Request::new("ok", "GET X", "/x")).is_err());
}

#[test]
fn name_directive_line_is_kept() {
    let text = "###\n# @name login\nPOST /login\n";
    let written = edited(text, 0, |r| r.url = "/sign-in".into());
    assert_eq!(written, "###\n# @name login\nPOST /sign-in\n");

    let titled = "### Log in\n# @name login\nPOST /login\n";
    let written = edited(titled, 0, |r| r.name = "Sign in".into());
    assert_eq!(written, "### Sign in\n# @name login\nPOST /login\n");

    let namespace = parse("# @namespace foo\nGET /x\n");
    assert_eq!(namespace[0].name, "");
    assert_eq!(namespace[0].comments, ["# @namespace foo"]);
}

#[test]
fn a_request_without_url_keeps_its_method() {
    let written = serialize(&[Request::new("New", "POST", "")]);
    assert_eq!(written, "### New\nPOST\n");
    let back = parse(&written);
    assert_eq!(
        (back[0].method.as_str(), back[0].url.as_str()),
        ("POST", "")
    );
    assert_eq!(serialize(&back), written);

    assert_eq!(parse("localhost\n")[0].url, "localhost");
}

#[test]
fn comments_among_headers_survive_edits() {
    let text = "GET https://x.test\n# Authorization: Bearer old\n// X-Debug: 1\nAccept: */*\n";
    let written = edited(text, 0, |r| r.headers[0].value = "application/json".into());
    assert_eq!(
        written,
        "GET https://x.test\n# Authorization: Bearer old\n// X-Debug: 1\nAccept: application/json\n"
    );
}

#[test]
fn a_response_handler_right_after_the_headers_is_kept() {
    let text = "GET https://x.test\nAccept: */*\n> {%\n  client.global.set('a', 1);\n%}\n";
    let request = &parse(text)[0];
    assert_eq!(request.headers.len(), 1);
    assert!(request.trailer.starts_with("> {%"));
    assert_eq!(serialize(&parse(text)), text);
}

#[test]
fn edits_keep_crlf_version_directive_spelling_and_line_order() {
    let text = "### Get\r\n# @assert status == 200\r\n# Docs\r\n@v = 1\r\n// @timeout 500 ms\r\n\
                GET https://x.test/a HTTP/1.1\r\nAccept: */*\r\n";
    assert_eq!(serialize(&parse(text)), text);

    let written = edited(text, 0, |r| r.url = "https://x.test/b".into());
    assert_eq!(
        written,
        "### Get\r\n# @assert status == 200\r\n# Docs\r\n@v = 1\r\n// @timeout 500 ms\r\n\
         GET https://x.test/b HTTP/1.1\r\nAccept: */*\r\n"
    );
}

#[test]
fn an_edit_only_changes_its_own_lines() {
    let text = "@host = api.test\n\n### One\nGET https://{{host}}/a\n\n\n### Two\n#  spaced comment\n\
                POST https://{{host}}/b\nContent-Type:application/json\nX-Trace: 1\n\n{\"a\": 1}\n";
    let written = edited(text, 1, |r| r.headers[1].value = "2".into());
    assert_eq!(written, text.replace("X-Trace: 1", "X-Trace: 2"));
}

#[test]
fn new_and_removed_lines_go_next_to_their_neighbours() {
    let text = "### A\n# Docs\n# @assert status == 200\nGET /a\nAccept: */*\nX-Old: 1\n";
    let written = edited(text, 0, |r| {
        r.headers.retain(|h| h.name != "X-Old");
        r.headers.insert(
            0,
            Header {
                name: "X-New".into(),
                value: "1".into(),
            },
        );
        r.assertions.push("duration < 500".into());
        r.settings.no_redirect = true;
    });
    assert_eq!(
        written,
        "### A\n# Docs\n# @assert status == 200\n# @assert duration < 500\n# @no-redirect\n\
         GET /a\nX-New: 1\nAccept: */*\n"
    );
}

#[test]
fn note_blocks_around_requests_are_kept_on_edit() {
    let text = "### Notes\n# only notes here\n\n### Real\nGET /x\n\n### More notes\n# end\n";
    let written = edited(text, 0, |r| r.url = "/y".into());
    assert_eq!(written, text.replace("GET /x", "GET /y"));
}

#[test]
fn quoted_body_lines_stay_in_the_body() {
    let text = "POST /notes\n\nhello\n> quoted\nmore\n";
    let request = &parse(text)[0];
    assert_eq!(request.body, "hello\n> quoted\nmore");
    assert!(request.trailer.is_empty());
}

#[test]
fn warns_about_body_lines_the_format_cannot_keep() {
    let mut request = Request::new("Doc", "POST", "/docs");
    request.body = "# Title\n### Section\n> {% x %}\n> fine".into();
    let warnings = body_warnings(&request);
    assert_eq!(warnings.len(), 2);
    assert!(warnings[0].contains("line 2"));
    assert!(warnings[1].contains("line 3"));
}

#[test]
fn file_start_text_never_lands_in_the_middle_of_a_file() {
    let first = parse("### A\nGET /a\n").remove(0);
    let moved = parse("@host = x.test\n\n### B\nGET /b\n").remove(0);
    let written = serialize(&[first, moved]);
    let back = parse(&written);
    assert_eq!(back.len(), 2, "{written}");
    assert_eq!(back[0].body, "", "{written}");
    assert_eq!(back[1].url, "/b");
}

#[test]
fn a_replaced_later_request_always_gets_its_own_block() {
    let files = [
        "### One\nGET /1\n\n### Two\nGET /2\n",
        "GET /1\n\n###\nGET /2\n\n### Three\nGET /3\n",
        "@host = x.test\n\n### One\nGET /1\n### Two\n# @name two\nGET /2\n",
    ];
    let replacements = [
        "GET /changed\n",
        "# @name changed\nPOST /changed\n",
        "@id = 1\nPUT /changed/{{id}}\n",
        "### Changed\nDELETE /changed\n",
    ];
    for file in files {
        let count = parse(file).len();
        for index in 0..count {
            for replacement in replacements {
                let mut requests = parse(file);
                requests[index] = parse(replacement).remove(0);
                let written = serialize(&requests);
                let back = parse(&written);
                assert_eq!(back.len(), count, "{written}");
                assert_eq!(back[index].url, requests[index].url, "{written}");
                assert_eq!(back[index].method, requests[index].method, "{written}");
            }
        }
    }
}
