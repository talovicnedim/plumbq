use std::fs;

use super::*;

/// A fresh empty directory under the system temp dir.
pub fn temp_workspace(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let dir = std::env::temp_dir().join(format!("ac-test-{name}-{nanos:x}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn names(nodes: &[TreeNode]) -> Vec<String> {
    nodes
        .iter()
        .map(|node| match node {
            TreeNode::Folder { name, .. } | TreeNode::Request { name, .. } => name.clone(),
        })
        .collect()
}

#[test]
fn tree_lists_folders_first_then_requests_alphabetically() {
    let root = temp_workspace("tree");
    write(&root, "b.http", "### Beta\nGET /b\n");
    write(&root, "a.http", "GET /a\n");
    write(&root, "users/list.http", "GET /users\n");

    assert_eq!(names(&tree(&root).unwrap()), ["users", "a", "Beta"]);
}

#[test]
fn tree_follows_folder_order() {
    let root = temp_workspace("order");
    write(&root, "a.http", "GET /a\n");
    write(&root, "b.http", "GET /b\n");
    write(&root, "c.http", "GET /c\n");
    write(&root, FOLDER_FILE, "order = [\"c.http\", \"a.http\"]\n");

    assert_eq!(names(&tree(&root).unwrap()), ["c", "a", "b"]);
}

#[test]
fn tree_hides_environments_hidden_and_empty_dirs() {
    let root = temp_workspace("hidden");
    write(&root, "environments/local.toml", "");
    write(&root, ".git/x.http", "GET /x\n");
    write(&root, "docs/readme.md", "");
    write(&root, "empty/folder.toml", "");
    write(&root, "a.http", "GET /a\n");

    assert_eq!(names(&tree(&root).unwrap()), ["empty", "a"]);
}

#[test]
fn tree_gives_each_request_of_a_file_its_own_id() {
    let root = temp_workspace("multi");
    write(&root, "all.http", "### One\nGET /1\n### Two\nGET /2\n");

    let ids: Vec<_> = tree(&root)
        .unwrap()
        .into_iter()
        .filter_map(|n| match n {
            TreeNode::Request { id, .. } => Some(id),
            TreeNode::Folder { .. } => None,
        })
        .collect();
    assert_eq!(ids, ["all.http", "all.http#2"]);
}

#[test]
fn create_makes_a_ready_to_use_workspace() {
    let parent = temp_workspace("create-ws");
    let root = create(&parent, "My API").unwrap();

    assert_eq!(root, parent.join("My API"));
    assert_eq!(names(&tree(&root).unwrap()), ["Example"]);
    assert!(root.join("environments/local.toml").is_file());
    assert!(create(&parent, "My API").is_err());
    assert!(create(&parent, "../escape").is_err());
}

#[test]
fn canonical_root_drops_windows_verbatim_prefix() {
    assert_eq!(
        strip_verbatim(PathBuf::from(r"\\?\C:\Users\ada\api")),
        PathBuf::from(r"C:\Users\ada\api")
    );
    // UNC verbatim paths have no plain equivalent with a drive letter; keep them.
    let unc = PathBuf::from(r"\\?\UNC\server\share");
    assert_eq!(strip_verbatim(unc.clone()), unc);
    let root = temp_workspace("canonical");
    assert_eq!(
        canonical_root(&root).unwrap(),
        strip_verbatim(root.canonicalize().unwrap())
    );
    assert!(canonical_root(&root.join("missing")).is_err());
}

#[test]
fn sorts_numbers_naturally() {
    let root = temp_workspace("natural");
    write(&root, "request 10.http", "GET /10\n");
    write(&root, "request 2.http", "GET /2\n");
    write(&root, "Request 1.http", "GET /1\n");

    assert_eq!(
        names(&tree(&root).unwrap()),
        ["Request 1", "request 2", "request 10"]
    );
}

#[test]
fn parses_request_ids() {
    assert_eq!(parse_request_id("a/b.http").unwrap(), ("a/b.http", 0));
    assert_eq!(parse_request_id("a/b.http#3").unwrap(), ("a/b.http", 2));
    assert!(parse_request_id("a/b.http#1").is_err());
}

#[cfg(unix)]
#[test]
fn resolve_existing_rejects_symlinks_leaving_the_workspace() {
    let root = temp_workspace("symlink");
    let outside = temp_workspace("symlink-outside");
    fs::write(outside.join("credentials"), "secret").unwrap();
    std::os::unix::fs::symlink(outside.join("credentials"), root.join("data.json")).unwrap();
    write(&root, "inside.json", "{}");
    std::os::unix::fs::symlink(root.join("inside.json"), root.join("alias.json")).unwrap();

    assert!(matches!(
        resolve_existing(&root, "data.json"),
        Err(Error::Invalid(_))
    ));
    assert!(resolve_existing(&root, "alias.json").is_ok());
    assert!(matches!(
        resolve_existing(&root, "missing.json"),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn resolve_rejects_escaping_paths() {
    let root = Path::new("/ws");
    assert!(resolve(root, "../etc/passwd").is_err());
    assert!(resolve(root, "/etc/passwd").is_err());
    assert_eq!(resolve(root, "a/b.http").unwrap(), root.join("a/b.http"));
}

#[test]
fn slugs_names() {
    assert_eq!(slug("Create user"), "create-user");
    assert_eq!(slug("  GET /users/{id}  "), "get-users-id");
    assert_eq!(slug("!!!"), "untitled");
    assert_eq!(slug("C# client"), "c-client");
}

#[test]
fn a_broken_folder_toml_is_a_warning_not_a_broken_tree() {
    let root = temp_workspace("toml-broken");
    write(&root, "a.http", "GET /a\n");
    write(&root, "users/list.http", "GET /users\n");
    write(&root, "users/folder.toml", "order = [\n");

    let (nodes, warnings) = tree_with_warnings(&root).unwrap();
    assert_eq!(names(&nodes), ["users", "a"]);
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].path, "users/folder.toml");
    assert!(write_settings(&root.join("users"), &FolderSettings::default()).is_err());
    assert_eq!(
        fs::read_to_string(root.join("users/folder.toml")).unwrap(),
        "order = [\n"
    );
}

#[test]
fn unreadable_files_are_skipped_with_a_warning() {
    let root = temp_workspace("latin1");
    write(&root, "a.http", "GET /a\n");
    fs::write(root.join("latin1.http"), b"### Caf\xe9\nGET /x\n").unwrap();

    let (nodes, warnings) = tree_with_warnings(&root).unwrap();
    assert_eq!(names(&nodes), ["a"]);
    assert_eq!(warnings[0].path, "latin1.http");
}

#[cfg(unix)]
#[test]
fn symlinked_folders_are_not_followed() {
    let root = temp_workspace("symlink-loop");
    write(&root, "a.http", "GET /a\n");
    std::os::unix::fs::symlink(&root, root.join("loop")).unwrap();

    let (nodes, warnings) = tree_with_warnings(&root).unwrap();
    assert_eq!(names(&nodes), ["a"]);
    assert_eq!(warnings[0].path, "loop");
}

#[test]
fn build_folders_are_only_skipped_at_the_root() {
    let root = temp_workspace("skipped");
    write(&root, "build/x.http", "GET /x\n");
    write(&root, "shop/build/deploy.http", "POST /deploy\n");

    let nodes = tree(&root).unwrap();
    assert_eq!(names(&nodes), ["shop"]);
    let TreeNode::Folder { children, .. } = &nodes[0] else {
        panic!("shop should be a folder");
    };
    assert_eq!(names(children), ["build"]);
}

#[test]
fn names_with_a_hash_are_left_out_of_the_tree() {
    let root = temp_workspace("hash-names");
    write(&root, "c#.http", "GET /c\n");
    write(&root, "d#/x.http", "GET /x\n");
    write(&root, "a.http", "GET /a\n");

    let (nodes, warnings) = tree_with_warnings(&root).unwrap();
    assert_eq!(names(&nodes), ["a"]);
    let mut warned: Vec<&str> = warnings.iter().map(|w| w.path.as_str()).collect();
    warned.sort();
    assert_eq!(warned, ["c#.http", "d#"]);
}
