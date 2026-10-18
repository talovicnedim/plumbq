use super::*;
use crate::workspace::tests::{temp_workspace, write};

#[test]
fn reads_environment_file() {
    let root = temp_workspace("env");
    write(
        &root,
        "environments/staging.toml",
        "tag = \"staging\"\n[variables]\nbaseUrl = \"https://staging.test\"\n",
    );

    let env = get(&root, "staging").unwrap();
    assert_eq!(env.tag, EnvironmentTag::Staging);
    assert_eq!(env.variables["baseUrl"], "https://staging.test");

    let names: Vec<_> = list(&root).unwrap().into_iter().map(|e| e.name).collect();
    assert_eq!(names, ["staging"]);
}

#[test]
fn create_then_update_variables() {
    let root = temp_workspace("env-create");
    let name = create(&root, "Production", EnvironmentTag::Production).unwrap();
    assert_eq!(name, "production");

    let mut env = get(&root, &name).unwrap();
    env.variables
        .insert("baseUrl".into(), "https://api.test".into());
    update(&root, env).unwrap();

    assert_eq!(
        values(&root, Some("production")).unwrap().variables["baseUrl"],
        "https://api.test"
    );
}

#[test]
fn reads_oauth_clients() {
    let root = temp_workspace("env-auth");
    write(
        &root,
        "environments/dev.toml",
        "[variables]\nid = \"app\"\n\n[auth.github]\ngrant = \"authorization_code\"\n\
         auth_url = \"https://gh.test/authorize\"\ntoken_url = \"https://gh.test/token\"\n\
         client_id = \"{{id}}\"\nscope = \"read:user\"\n",
    );

    let client = auth_client(&root, "dev", "github").unwrap();
    assert_eq!(client.grant, crate::oauth::OAuthGrant::AuthorizationCode);
    assert_eq!(client.client_id, "{{id}}");
    assert!(client.client_secret.is_empty());
    assert!(matches!(
        auth_client(&root, "dev", "nope"),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn workspace_variables_are_defaults_environments_override() {
    let root = temp_workspace("env-defaults");
    write(
        &root,
        "folder.toml",
        "[variables]\nbaseUrl = \"http://localhost\"\nversion = \"v1\"\n",
    );
    write(
        &root,
        "environments/prod.toml",
        "tag = \"production\"\n[variables]\nbaseUrl = \"https://api.test\"\n",
    );

    let prod = values(&root, Some("prod")).unwrap();
    assert_eq!(prod.variables["baseUrl"], "https://api.test");
    assert_eq!(prod.variables["version"], "v1");
    let none = values(&root, None).unwrap();
    assert_eq!(none.variables["baseUrl"], "http://localhost");
}

#[test]
fn rejects_names_that_could_escape() {
    let root = temp_workspace("env-escape");
    assert!(matches!(get(&root, "../x"), Err(Error::Invalid(_))));
}

#[test]
fn no_environment_has_no_values() {
    let values = values(Path::new("/nowhere"), None).unwrap();
    assert!(values.variables.is_empty());
    assert_eq!(values.tag, EnvironmentTag::Local);
}

#[test]
fn secrets_come_from_process_env_before_the_keychain() {
    let root = temp_workspace("env-ci-secret");
    write(
        &root,
        "environments/ci.toml",
        "secrets = [\"api-token\"]\n[variables]\nbaseUrl = \"https://api.test\"\n",
    );
    let env = |name: &str| (name == "PLUMBQ_SECRET_API_TOKEN").then(|| "s3cr3t".to_string());

    let values = values_with(&root, Some("ci"), &env).unwrap();

    assert_eq!(values.secrets["api-token"], "s3cr3t");
    assert_eq!(values.variables["baseUrl"], "https://api.test");
}

#[test]
fn secret_env_var_names() {
    assert_eq!(secret_env_var("token"), "PLUMBQ_SECRET_TOKEN");
    assert_eq!(secret_env_var("api-key.v2"), "PLUMBQ_SECRET_API_KEY_V2");
}

#[test]
fn reads_names_tags_and_variables_without_the_keychain() {
    let root = temp_workspace("env-meta");
    write(
        &root,
        "environments/prod.toml",
        "tag = \"production\"\nsecrets = [\"token\"]\n[variables]\nbaseUrl = \"https://api.test\"\n",
    );
    write(&root, "folder.toml", "[variables]\nversion = \"v1\"\n");

    assert_eq!(names(&root).unwrap(), ["prod"]);
    assert!(exists(&root, "prod"));
    assert!(!exists(&root, "nope"));
    assert!(!exists(&root, "../x"));
    assert_eq!(tag(&root, "prod").unwrap(), EnvironmentTag::Production);
    assert_eq!(secret_names(&root, "prod").unwrap(), ["token"]);
    let plain = plain_variables(&root, Some("prod")).unwrap();
    assert_eq!(plain["version"], "v1");
    assert_eq!(plain["baseUrl"], "https://api.test");
    assert!(!plain.contains_key("token"));
    assert!(
        !own_variables(&root, "prod")
            .unwrap()
            .contains_key("version")
    );
}

#[test]
fn saving_keeps_comments_unknown_keys_and_value_types() {
    let root = temp_workspace("env-keep");
    write(
        &root,
        "environments/dev.toml",
        "# Shared dev setup\ntag = \"development\"\ndescription = \"keep me\"\n\n\
         [variables]\n# the API\nbaseUrl = \"https://dev.test\"\nport = 8080\n",
    );

    let mut env = get(&root, "dev").unwrap();
    assert_eq!(env.variables["port"], "8080");
    env.variables
        .insert("baseUrl".into(), "https://dev2.test".into());
    update(&root, env).unwrap();

    let text = std::fs::read_to_string(root.join("environments/dev.toml")).unwrap();
    assert!(text.contains("# Shared dev setup"));
    assert!(text.contains("description = \"keep me\""));
    assert!(text.contains("# the API"));
    assert!(text.contains("port = 8080"), "{text}");
    assert!(text.contains("baseUrl = \"https://dev2.test\""));
}

#[test]
fn one_broken_file_does_not_hide_the_others() {
    let root = temp_workspace("env-broken");
    write(&root, "environments/good.toml", "tag = \"staging\"\n");
    write(
        &root,
        "environments/staging.eu.toml",
        "[variables]\nregion = \"eu\"\n",
    );
    write(&root, "environments/broken.toml", "tag = \n");

    let list = list(&root).unwrap();

    let names: Vec<_> = list.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["broken", "good", "staging.eu"]);
    assert!(
        list[0]
            .warning
            .as_deref()
            .is_some_and(|w| w.contains("broken.toml"))
    );
    assert!(list[1].warning.is_none());
    assert_eq!(list[2].variables["region"], "eu");
    // A broken file is never overwritten by a save.
    assert!(update(&root, list[0].clone()).is_err());
}

#[test]
fn a_stale_save_never_removes_secrets() {
    let root = temp_workspace("env-stale-secrets");
    write(&root, "environments/dev.toml", "secrets = [\"token\"]\n");
    let mut stale = get(&root, "dev").unwrap();
    stale.secrets.clear();
    stale.variables.insert("a".into(), "1".into());

    update(&root, stale).unwrap();

    assert_eq!(secret_names(&root, "dev").unwrap(), ["token"]);
    assert!(matches!(
        remove_secret(&root, "dev", "nope"),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn names_never_point_outside_environments() {
    let root = temp_workspace("env-names");
    for name in ["../x", ".hidden", "a/b", "", "C:evil", "a:b", "a\\b"] {
        assert!(get(&root, name).is_err(), "{name}");
    }
}

#[test]
fn plain_variables_are_set_and_removed_but_secrets_are_refused() {
    let root = temp_workspace("env-variables");
    write(
        &root,
        "environments/dev.toml",
        "# team settings\ntag = \"development\"\nsecrets = [\"token\"]\n\n[variables]\nbaseUrl = \"http://dev.test\"\n",
    );
    set_variable(&root, "dev", "port", "8080").unwrap();
    assert_eq!(own_variables(&root, "dev").unwrap()["port"], "8080");
    let text = std::fs::read_to_string(root.join("environments/dev.toml")).unwrap();
    assert!(text.contains("# team settings"), "{text}");

    assert_eq!(
        set_variable(&root, "dev", "token", "x").unwrap_err().kind(),
        "invalid"
    );
    remove_variable(&root, "dev", "port").unwrap();
    assert!(!own_variables(&root, "dev").unwrap().contains_key("port"));
    assert_eq!(
        remove_variable(&root, "dev", "port").unwrap_err().kind(),
        "notFound"
    );
}

#[test]
fn names_only_match_the_file_name_exactly() {
    let root = temp_workspace("env-case");
    write(
        &root,
        "environments/prod.toml",
        "tag = \"production\"\n[variables]\nbaseUrl = \"https://api.test\"\n",
    );
    assert!(exists(&root, "prod"));
    for name in ["PROD", "Prod"] {
        assert!(!exists(&root, name), "{name}");
        assert_eq!(get(&root, name).unwrap_err().kind(), "notFound", "{name}");
        assert!(plain_variables(&root, Some(name)).is_err(), "{name}");
        assert!(values_with(&root, Some(name), &|_| None).is_err(), "{name}");
        assert!(set_variable(&root, name, "x", "1").is_err(), "{name}");
    }
    assert!(!own_variables(&root, "prod").unwrap().contains_key("x"));
}

#[test]
fn names_the_keychain_uses_for_its_own_entries_are_refused() {
    let root = temp_workspace("env-keychain-names");
    write(
        &root,
        "environments/$workspace.toml",
        "secrets = [\"clientPfxPassword\"]\n",
    );
    // Windows doesn't allow a | in file names.
    #[cfg(unix)]
    write(&root, "environments/a|b.toml", "");
    write(
        &root,
        "environments/dev.toml",
        "secrets = [\"$auth:github\"]\n",
    );

    assert_eq!(names(&root).unwrap(), ["dev"]);
    assert_eq!(get(&root, "$workspace").unwrap_err().kind(), "invalid");
    let resolved = values_with(&root, Some("dev"), &|_| Some("v".into())).unwrap();
    assert!(resolved.secrets.is_empty(), "{:?}", resolved.secrets);

    for name in ["a|b", "$auth:x"] {
        let env = Environment {
            name: "dev".into(),
            tag: EnvironmentTag::Local,
            variables: BTreeMap::new(),
            secrets: vec![SecretVariable {
                name: name.into(),
                is_set: false,
            }],
            auth: Vec::new(),
            warning: None,
        };
        assert_eq!(update(&root, env).unwrap_err().kind(), "invalid", "{name}");
    }
    assert_eq!(secret_names(&root, "dev").unwrap(), ["$auth:github"]);
}

#[test]
fn files_whose_names_cant_be_used_are_reported_not_hidden() {
    let root = temp_workspace("env-bad-names");
    write(&root, "environments/staging.toml", "tag = \"staging\"\n");
    write(&root, "environments/$x.toml", "tag = \"local\"\n");
    // Windows doesn't allow a | in file names, so there such a file can't exist.
    #[cfg(unix)]
    write(&root, "environments/a|b.toml", "tag = \"local\"\n");
    write(&root, "environments/.draft.toml", "tag = \"local\"\n");

    let names: Vec<_> = list(&root).unwrap().into_iter().map(|e| e.name).collect();
    assert_eq!(names, ["staging"]);
    let skipped: Vec<_> = warnings(&root)
        .unwrap()
        .into_iter()
        .map(|w| w.path)
        .collect();
    let mut expected = vec!["environments/$x.toml"];
    if cfg!(unix) {
        expected.push("environments/a|b.toml");
    }
    assert_eq!(skipped, expected);
}

#[test]
fn names_with_spaces_and_dots_keep_working() {
    let root = temp_workspace("env-loose-names");
    for name in ["my env", "staging.eu", "Prod-US", "dev (old)"] {
        write(
            &root,
            &format!("environments/{name}.toml"),
            "tag = \"local\"\n",
        );
        assert!(exists(&root, name), "{name}");
        assert_eq!(get(&root, name).unwrap().name, name);
    }
    assert!(!exists(&root, "my  env"));
    assert!(warnings(&root).unwrap().is_empty());
}

#[test]
fn names_that_could_leave_the_folder_or_hit_the_keychain_are_refused() {
    let root = temp_workspace("env-bad-name-rules");
    let too_long = "a".repeat(251);
    for name in [
        "",
        ".hidden",
        "$auth",
        "a|b",
        "a/b",
        "..\\x",
        "a\nb",
        too_long.as_str(),
    ] {
        assert_eq!(get(&root, name).unwrap_err().kind(), "invalid", "{name:?}");
    }
}

#[test]
fn unknown_environments_get_a_hint() {
    let known = vec!["staging".to_string()];
    assert_eq!(unknown_hint("Staging", &known), " (did you mean staging?)");
    assert_eq!(unknown_hint("prod", &known), "");
    assert!(unknown_hint("$x", &known).contains("starts with $"));
    assert!(unknown_hint("a|b", &known).contains("|"));
}
