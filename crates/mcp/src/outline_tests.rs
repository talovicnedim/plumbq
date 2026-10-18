use plumbq_core::http_file::Header;

use super::*;

fn request(id: &str, name: &str, method: &str, tags: &[&str]) -> TreeNode {
    TreeNode::Request {
        id: id.into(),
        name: name.into(),
        method: method.into(),
        url: format!("{{{{baseUrl}}}}/{name}"),
        tags: tags.iter().map(|t| t.to_string()).collect(),
    }
}

fn workspace() -> Vec<TreeNode> {
    vec![
        TreeNode::Folder {
            name: "users".into(),
            path: "users".into(),
            children: vec![
                request("users/list.http", "list", "GET", &["smoke"]),
                TreeNode::Folder {
                    name: "admin".into(),
                    path: "users/admin".into(),
                    children: vec![request("users/admin/ban.http", "ban", "POST", &[])],
                },
            ],
        },
        request("login.http", "login", "POST", &["auth"]),
    ]
}

#[test]
fn tree_shows_paths_ids_and_tags() {
    let text = tree(&workspace(), None);
    assert!(text.starts_with("📁 users/  (folder path: users)\n  GET list  [id: users/list.http]"));
    assert!(text.contains("#smoke\n"));
    assert!(text.contains("    POST ban  [id: users/admin/ban.http]"));
}

#[test]
fn depth_summarizes_deeper_folders() {
    let text = tree(&workspace(), Some(1));
    assert!(text.contains("  📁 admin/  (folder path: users/admin, 1 requests)"));
    assert!(!text.contains("ban.http"));
    let top = tree(&workspace(), Some(0));
    assert!(top.starts_with("📁 users/  (folder path: users, 2 requests)"));
}

#[test]
fn big_trees_are_cut_with_a_hint() {
    let nodes: Vec<TreeNode> = (0..MAX_TREE_LINES + 5)
        .map(|i| request(&format!("r{i}.http"), &format!("r{i}"), "GET", &[]))
        .collect();
    let text = tree(&nodes, None);
    assert_eq!(text.lines().count(), MAX_TREE_LINES + 1);
    assert!(
        text.ends_with("… 5 more lines. Pass `folder` and/or `depth`, or use requests_search.\n")
    );
}

#[test]
fn finds_folders_and_requests() {
    let nodes = workspace();
    assert_eq!(subtree(&nodes, "users/admin").map(<[_]>::len), Some(1));
    assert!(subtree(&nodes, "nope").is_none());

    assert_eq!(search_results(&[], 10), "No matching requests.\n");
}

#[test]
fn folder_settings_show_headers_order_and_defaults() {
    let mut settings = FolderSettings::default();
    settings
        .headers
        .insert("Accept".into(), "application/json".into());
    settings.order = vec!["b.http".into(), "a.http".into()];
    settings
        .variables
        .insert("baseUrl".into(), "https://api.test".into());

    let root = folder("", &settings);
    assert!(root.contains("  Accept: application/json\n"));
    assert!(root.contains("Order: b.http, a.http\n"));
    assert!(root.contains("  baseUrl: https://api.test\n"));
    let sub = folder("users", &FolderSettings::default());
    assert!(sub.contains("Shared headers:\n  (none)\n"));
    assert!(!sub.contains("Variable defaults"));
}

#[test]
fn preview_shows_request_prompts_and_verdict() {
    let sent = SentRequest {
        method: "POST".into(),
        url: "https://api.test/login".into(),
        headers: vec![Header {
            name: "Authorization".into(),
            value: "Bearer {{token}}".into(),
        }],
        body: "{}".into(),
    };
    let text = preview(&Ok(sent), &["otp".into()], &Ok(()));
    assert!(text.starts_with("POST https://api.test/login\nAuthorization: Bearer {{token}}\n\n{}"));
    assert!(text.contains("Asks for: otp"));
    assert!(text.contains("This server may send it."));

    let refused = preview(&Err("unknown variables: x".into()), &[], &Err("no".into()));
    assert!(refused.contains("Can't be sent as is: unknown variables: x"));
    assert!(refused.contains("This server won't send it: no"));
}
