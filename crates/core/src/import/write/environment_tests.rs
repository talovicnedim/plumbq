use super::super::{ImportFormat, ImportedCollection, environment_changes, write};
use super::*;
use crate::http_file::Request;
use crate::import::ImportedItem;
use crate::workspace::tests::{temp_workspace, write as write_file};

fn collection(name: &str, variables: &[(&str, &str)]) -> ImportedCollection {
    ImportedCollection {
        format: ImportFormat::Postman,
        notes: Vec::new(),
        name: name.into(),
        headers: BTreeMap::new(),
        items: vec![ImportedItem::Request(Request::new(
            "Ping",
            "GET",
            "https://x.test/ping",
        ))],
        environments: vec![ImportedEnvironment {
            name: name.into(),
            variables: variables
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            secrets: vec!["apiKey".into()],
        }],
        credentials: Vec::new(),
    }
}

#[test]
fn the_plan_shows_what_an_import_adds_to_an_existing_environment() {
    let root = temp_workspace("import-env-plan");
    write_file(
        &root,
        "environments/production.toml",
        "tag = \"production\"\n[variables]\nbaseUrl = \"https://api.test\"\n",
    );
    let first = collection("Production", &[("baseUrl", "https://x.test")]);
    let planned = environment_changes(&root, "", &first).unwrap();
    assert_eq!(planned[0].name, "production-2");
    assert!(!planned[0].existing);
    assert_eq!(
        write(&root, "", first).unwrap().environments,
        ["production-2"]
    );

    let again = collection(
        "Production",
        &[
            ("baseUrl", "https://x.test"),
            ("exfil", "https://evil.test"),
        ],
    );
    let planned = environment_changes(&root, "", &again).unwrap();
    assert_eq!(
        planned,
        [ImportEnvironmentChange {
            name: "production".into(),
            tag: EnvironmentTag::Production,
            existing: true,
            variables: BTreeMap::from([("exfil".into(), "https://evil.test".into())]),
            secrets: vec!["apiKey".into()],
        }]
    );
    assert_eq!(
        write(&root, "", again).unwrap().environments,
        ["production"]
    );
}

#[test]
fn new_names_skip_existing_ones_in_any_letter_case() {
    let root = temp_workspace("import-env-case");
    write_file(&root, "environments/Staging.toml", "tag = \"staging\"\n");
    let planned = environment_changes(&root, "", &collection("staging", &[])).unwrap();
    assert_eq!(planned[0].name, "staging-2");
}
