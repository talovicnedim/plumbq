use super::*;
use crate::workspace::tests::{temp_workspace, write};

fn env() -> Values {
    Values {
        variables: BTreeMap::from([
            ("baseUrl".into(), "https://env.test".into()),
            ("host".into(), "env-host".into()),
        ]),
        ..Default::default()
    }
}

#[test]
fn file_variables_win_over_the_environment_and_can_use_it() {
    let root = temp_workspace("file-vars");
    write(
        &root,
        "api.http",
        "@host = file-host\n@api = {{baseUrl}}/v2\n\n### One\nGET {{api}}/{{host}}\n### Two\n@id = 7\nGET /x\n",
    );
    let request = crate::requests::get(&root, "api.http").unwrap();
    let mut values = env();

    resolve_into(
        &root,
        "api.http",
        &request,
        "{{api}}/{{host}}",
        &BTreeMap::new(),
        &mut values,
    )
    .unwrap();

    assert_eq!(values.variables["host"], "file-host");
    assert_eq!(values.variables["api"], "https://env.test/v2");
    assert_eq!(
        values.variables["id"], "7",
        "variables of other requests in the file count"
    );
}

#[test]
fn file_variables_built_from_secrets_stay_masked() {
    let root = temp_workspace("file-vars-secret");
    write(
        &root,
        "a.http",
        "@token = {{token}}\n@auth = Bearer {{token}}\n\nGET /x\nAuthorization: {{auth}}\n",
    );
    let request = crate::requests::get(&root, "a.http").unwrap();
    let mut values = env();
    values.secrets.insert("token".into(), "s3cr3t-value".into());

    resolve_into(
        &root,
        "a.http",
        &request,
        "{{auth}}",
        &BTreeMap::new(),
        &mut values,
    )
    .unwrap();

    assert_eq!(values.secrets["token"], "s3cr3t-value");
    assert_eq!(values.secrets["auth"], "Bearer s3cr3t-value");
    assert!(!values.variables.contains_key("auth"));
    let masked = variables::mask("Authorization: Bearer s3cr3t-value", &values.secrets);
    assert!(!masked.contains("s3cr3t-value"));
}

#[test]
fn unsaved_edits_and_kept_names_win() {
    let root = temp_workspace("file-vars-edit");
    write(&root, "a.http", "@id = 1\nGET /items/{{id}}\n");
    let mut draft = crate::requests::get(&root, "a.http").unwrap();
    draft.variables[0].value = "2".into();
    let mut values = env();

    resolve_into(
        &root,
        "a.http",
        &draft,
        "{{id}}",
        &BTreeMap::new(),
        &mut values,
    )
    .unwrap();
    assert_eq!(values.variables["id"], "2");

    let keep = BTreeMap::from([("id".to_string(), "row".to_string())]);
    let mut values = env();
    values.variables.insert("id".into(), "row".into());
    resolve_into(&root, "a.http", &draft, "{{id}}", &keep, &mut values).unwrap();
    assert_eq!(values.variables["id"], "row");
}

#[test]
fn unresolvable_variables_fail_only_when_used() {
    let root = temp_workspace("file-vars-missing");
    write(&root, "a.http", "@token = {{nope}}\nGET /a\n");
    let request = crate::requests::get(&root, "a.http").unwrap();

    assert!(
        resolve_into(
            &root,
            "a.http",
            &request,
            "/a",
            &BTreeMap::new(),
            &mut env()
        )
        .is_ok()
    );
    let err = resolve_into(
        &root,
        "a.http",
        &request,
        "{{token}}",
        &BTreeMap::new(),
        &mut env(),
    );
    assert_eq!(err.unwrap_err().to_string(), "unknown variables: nope");
}

#[test]
fn unsaved_requests_bring_their_own_variables() {
    let root = temp_workspace("file-vars-new");
    let mut request = Request::new("", "GET", "/{{x}}");
    request.variables.push(FileVariable {
        name: "x".into(),
        value: "1".into(),
    });
    assert_eq!(raw_values(&root, "new.http", &request).unwrap(), ["1"]);
}

#[test]
fn collects_variables_from_the_whole_file() {
    let root = temp_workspace("file-vars-all");
    write(
        &root,
        "all.http",
        "@host = a.test\n\n### One\n@one = 1\nGET /1\n\n### Two\n@two = 2\nGET /2\n",
    );
    let first = http_file::parse(&std::fs::read_to_string(root.join("all.http")).unwrap());

    let names: Vec<String> = collect(&root, "all.http", &first[0])
        .unwrap()
        .into_iter()
        .map(|v| v.name)
        .collect();

    assert_eq!(names, ["host", "one", "two"]);
}
