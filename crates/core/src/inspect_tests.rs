use super::*;
use crate::workspace::tests::{temp_workspace, write};

fn draft(url: &str, body: &str) -> Request {
    Request {
        body: body.to_string(),
        ..Request::new("Draft", "GET", url)
    }
}

fn kinds(view: &RequestView) -> Vec<(String, VariableKind, String)> {
    view.variables
        .iter()
        .map(|v| (v.name.clone(), v.kind, v.value.clone()))
        .collect()
}

#[test]
fn tells_where_each_variable_comes_from() {
    let root = temp_workspace("inspect-vars");
    write(&root, "folder.toml", "[variables]\nversion = \"v1\"\n");
    write(
        &root,
        "environments/dev.toml",
        "tag = \"development\"\n[variables]\nbaseUrl = \"https://dev.test\"\n",
    );
    let url =
        "{{baseUrl}}/{{version}}/{{$uuid}}/{{$prompt id}}/{{login.response.body.$.id}}/{{nope}}";
    let view = request(&root, "a.http", &draft(url, ""), Some("dev")).unwrap();
    assert_eq!(
        kinds(&view),
        [
            (
                "baseUrl".into(),
                VariableKind::Value,
                "https://dev.test".into()
            ),
            ("version".into(), VariableKind::Workspace, "v1".into()),
            ("$uuid".into(), VariableKind::Dynamic, String::new()),
            ("$prompt id".into(), VariableKind::Prompt, String::new()),
            (
                "login.response.body.$.id".into(),
                VariableKind::Chain,
                "login".into()
            ),
            ("nope".into(), VariableKind::Missing, String::new()),
        ]
    );
}

#[test]
fn oauth_tokens_name_their_client() {
    let root = temp_workspace("inspect-oauth");
    let r = draft("https://api.test", "");
    let r = Request {
        headers: vec![crate::http_file::Header {
            name: "Authorization".into(),
            value: "Bearer {{$auth.token(\"github\")}}".into(),
        }],
        ..r
    };
    let view = request(&root, "a.http", &r, None).unwrap();
    assert_eq!(view.variables[0].kind, VariableKind::Oauth);
    assert_eq!(view.variables[0].value, "github");
}

#[test]
fn finds_a_file_body_and_lines_the_format_cannot_keep() {
    let root = temp_workspace("inspect-body");
    let view = request(
        &root,
        "a.http",
        &draft("https://x.test", "\n< ./data.json\n"),
        None,
    )
    .unwrap();
    assert_eq!(view.file_reference.as_deref(), Some("./data.json"));
    let view = request(
        &root,
        "a.http",
        &draft("https://x.test", "<> ./previous.json"),
        None,
    )
    .unwrap();
    assert_eq!(view.file_reference, None);
    let view = request(&root, "a.http", &draft("https://x.test", "a\n### b"), None).unwrap();
    assert_eq!(view.body_warnings.len(), 1);
}
