use super::super::ImportFormat;
use super::environment::infer_tag;
use super::*;
use crate::environments::{self, EnvironmentTag};
use crate::workspace::read_settings;
use crate::workspace::tests::temp_workspace;

fn request(name: &str, method: &str, url: &str) -> ImportedItem {
    ImportedItem::Request(Request::new(name, method, url))
}

fn collection(items: Vec<ImportedItem>) -> ImportedCollection {
    ImportedCollection {
        format: ImportFormat::OpenApi,
        notes: Vec::new(),
        name: "Shop API".into(),
        headers: BTreeMap::from([("Authorization".into(), "Bearer {{token}}".into())]),
        items,
        environments: vec![ImportedEnvironment {
            name: "Shop API".into(),
            variables: BTreeMap::from([("baseUrl".into(), "https://shop.test".into())]),
            secrets: vec!["token".into()],
        }],
        credentials: Vec::new(),
    }
}

#[test]
fn write_creates_folders_requests_order_and_environments() {
    let root = temp_workspace("import-write");
    let items = vec![
        request("Zeta", "GET", "{{baseUrl}}/z"),
        ImportedItem::Folder {
            name: "Orders".into(),
            headers: BTreeMap::new(),
            items: vec![request("List orders", "GET", "{{baseUrl}}/orders")],
        },
        request("Alpha", "GET", "{{baseUrl}}/a"),
    ];

    let summary = write(&root, "", collection(items)).unwrap();
    assert_eq!(summary.folder, "shop-api");
    assert!(!summary.merged);
    assert_eq!(
        (summary.requests, summary.added, summary.folders),
        (3, 3, 1)
    );
    assert_eq!(summary.environments, ["shop-api"]);

    let settings = read_settings(&root.join("shop-api")).unwrap();
    assert_eq!(settings.order, ["zeta.http", "orders", "alpha.http"]);
    assert_eq!(settings.headers["Authorization"], "Bearer {{token}}");
    let zeta = requests::get(&root, "shop-api/zeta.http").unwrap();
    assert_eq!(zeta.comments, ["# @source GET {{baseUrl}}/z | Zeta"]);

    let env = environments::get(&root, "shop-api").unwrap();
    assert_eq!(env.variables["baseUrl"], "https://shop.test");
    assert_eq!(env.secrets[0].name, "token");
}

#[test]
fn reimport_merges_keeps_edits_and_reports_stale() {
    let root = temp_workspace("import-merge");
    let first = vec![
        request("List users", "GET", "{{baseUrl}}/users?limit=10"),
        request("Delete user", "DELETE", "{{baseUrl}}/users/{{id}}"),
        ImportedItem::Folder {
            name: "Orders".into(),
            headers: BTreeMap::new(),
            items: vec![request("Get order", "GET", "{{baseUrl}}/orders/{{id}}")],
        },
    ];
    write(&root, "", collection(first)).unwrap();

    // The user edits a body and an environment value.
    let id = "shop-api/list-users.http";
    let mut edited = requests::get(&root, id).unwrap();
    edited.body = "kept".into();
    requests::update(&root, id, edited, None).unwrap();
    let mut env = environments::get(&root, "shop-api").unwrap();
    env.variables
        .insert("baseUrl".into(), "http://localhost".into());
    environments::update(&root, env).unwrap();

    let second = vec![
        request("All users", "GET", "{{baseUrl}}/users?limit=50"),
        request("Create user", "POST", "{{baseUrl}}/users"),
        ImportedItem::Folder {
            name: "Orders".into(),
            headers: BTreeMap::new(),
            items: vec![request("Get order", "GET", "{{baseUrl}}/orders/{{id}}")],
        },
    ];
    let summary = write(&root, "", collection(second)).unwrap();

    assert!(summary.merged);
    assert_eq!(summary.folder, "shop-api");
    assert_eq!(
        (summary.added, summary.updated, summary.unchanged),
        (1, 1, 1)
    );
    assert_eq!(summary.folders, 0);
    assert_eq!(summary.stale, ["shop-api/delete-user.http"]);
    assert!(root.join("shop-api/delete-user.http").exists());

    let renamed = requests::get(&root, id).unwrap();
    assert_eq!(renamed.name, "All users");
    assert_eq!(renamed.body, "kept");
    assert_eq!(renamed.url, "{{baseUrl}}/users?limit=10");
    let env = environments::get(&root, "shop-api").unwrap();
    assert_eq!(env.variables["baseUrl"], "http://localhost");
    assert_eq!(summary.environments, ["shop-api"]);
    assert!(!root.join("shop-api-2").exists());
}

#[test]
fn same_named_folder_without_an_import_is_left_alone() {
    let root = temp_workspace("import-collide");
    folders::create(&root, "", "Shop API").unwrap();
    requests::create(&root, "shop-api", Request::new("Mine", "GET", "/mine")).unwrap();

    let summary = write(&root, "", collection(vec![request("A", "GET", "/a")])).unwrap();
    assert!(!summary.merged);
    assert_eq!(summary.folder, "shop-api-2");
}

#[test]
fn literal_credentials_become_secrets() {
    let root = temp_workspace("import-credentials");
    let mut source = collection(Vec::new());
    source.headers = BTreeMap::from([("X-API-Key".into(), "k-123".into())]);
    let mut login = Request::new("Login", "GET", "{{baseUrl}}/me");
    login.headers.push(crate::http_file::Header {
        name: "Authorization".into(),
        value: "Bearer abc.def".into(),
    });
    source.items = vec![ImportedItem::Request(login)];
    source.environments[0].secrets.clear();

    let summary = write(&root, "", source).unwrap();

    let settings = read_settings(&root.join("shop-api")).unwrap();
    assert_eq!(settings.headers["X-API-Key"], "{{apiKey}}");
    let text = std::fs::read_to_string(root.join("shop-api/login.http")).unwrap();
    assert!(text.contains("Authorization: Bearer {{token}}"), "{text}");
    assert!(!text.contains("abc.def"));
    let secrets = crate::environments::secret_names(&root, "shop-api").unwrap();
    assert_eq!(secrets, ["apiKey", "token"]);
    assert!(summary.notes.iter().any(|n| n.contains("apiKey, token")));
}

#[test]
fn environments_get_tags_and_secret_variables_lose_their_values() {
    let root = temp_workspace("import-env-tags");
    let mut source = collection(Vec::new());
    source.headers.clear();
    source.environments = vec![ImportedEnvironment {
        name: "Production".into(),
        variables: BTreeMap::from([
            ("baseUrl".into(), "https://shop.test".into()),
            ("apiKey".into(), "live-key".into()),
        ]),
        secrets: Vec::new(),
    }];

    let summary = write(&root, "", source).unwrap();

    assert_eq!(
        crate::environments::tag(&root, "production").unwrap(),
        EnvironmentTag::Production
    );
    let variables = crate::environments::own_variables(&root, "production").unwrap();
    assert!(!variables.contains_key("apiKey"));
    assert_eq!(
        crate::environments::secret_names(&root, "production").unwrap(),
        ["apiKey"]
    );
    let file = std::fs::read_to_string(root.join("environments/production.toml")).unwrap();
    assert!(!file.contains("live-key"));
    assert!(summary.notes.iter().any(|n| n.contains("apiKey")));
}

#[test]
fn infers_environment_tags_from_names() {
    for (name, tag) in [
        ("prod", EnvironmentTag::Production),
        ("Shop Production", EnvironmentTag::Production),
        ("live-eu", EnvironmentTag::Production),
        ("Staging", EnvironmentTag::Staging),
        ("dev", EnvironmentTag::Development),
        ("Local", EnvironmentTag::Local),
        ("product-catalog", EnvironmentTag::Local),
    ] {
        assert_eq!(infer_tag(name), tag, "{name}");
    }
}

#[test]
fn requests_sharing_a_key_are_matched_by_name_and_never_duplicated() {
    let root = temp_workspace("import-same-key");
    let pages = || {
        vec![
            request("Page 1", "GET", "{{baseUrl}}/x?page=1"),
            request("Page 2", "GET", "{{baseUrl}}/x?page=2"),
        ]
    };
    write(&root, "", collection(pages())).unwrap();

    for _ in 0..2 {
        let summary = write(&root, "", collection(pages())).unwrap();
        assert_eq!(
            (summary.added, summary.updated, summary.unchanged),
            (0, 0, 2)
        );
    }
    assert_eq!(
        requests::get(&root, "shop-api/page-1.http").unwrap().name,
        "Page 1"
    );
    assert_eq!(
        requests::get(&root, "shop-api/page-2.http").unwrap().name,
        "Page 2"
    );
    assert!(!root.join("shop-api/page-2-2.http").exists());
}

#[test]
fn re_imports_keep_the_users_own_renames() {
    let root = temp_workspace("import-user-rename");
    write(&root, "", collection(vec![request("List", "GET", "/list")])).unwrap();
    let id = "shop-api/list.http";
    let mut mine = requests::get(&root, id).unwrap();
    mine.name = "My favourite".into();
    requests::update(&root, id, mine, None).unwrap();

    // Unchanged source: the user's name stays.
    write(&root, "", collection(vec![request("List", "GET", "/list")])).unwrap();
    assert_eq!(requests::get(&root, id).unwrap().name, "My favourite");

    // The source renames it: the user's name still wins, the marker learns the new name.
    let summary = write(
        &root,
        "",
        collection(vec![request("List all", "GET", "/list")]),
    )
    .unwrap();
    assert_eq!((summary.updated, summary.unchanged), (0, 1));
    let request = requests::get(&root, id).unwrap();
    assert_eq!(request.name, "My favourite");
    assert!(
        request
            .comments
            .contains(&"# @source GET /list | List all".to_string())
    );
}

#[test]
fn old_markers_without_names_still_match() {
    let root = temp_workspace("import-old-marker");
    folders::create(&root, "", "Shop API").unwrap();
    let mut old = Request::new("Mine now", "GET", "/list");
    old.comments.push("# @source GET /list".into());
    requests::create(&root, "shop-api", old).unwrap();

    let summary = write(&root, "", collection(vec![request("List", "GET", "/list")])).unwrap();

    assert!(summary.merged);
    assert_eq!((summary.added, summary.unchanged), (0, 1));
    assert_eq!(
        requests::get(&root, "shop-api/mine-now.http").unwrap().name,
        "Mine now"
    );
}

#[test]
fn long_note_and_stale_lists_are_cut_with_a_count() {
    let mut summary = empty_summary(ImportFormat::Postman, String::new(), true, Vec::new());
    summary.notes = (0..25).map(|n| format!("note {n}")).collect();
    summary.stale = (0..30).map(|n| format!("r{n}.http")).collect();
    cap_lists(&mut summary);
    assert_eq!(summary.stale.len(), MAX_LISTED);
    assert_eq!(summary.notes.len(), MAX_LISTED + 2);
    assert!(summary.notes.contains(&"and 5 more notes".to_string()));
    assert!(
        summary
            .notes
            .contains(&"10 more requests are no longer in the source".to_string())
    );
}
