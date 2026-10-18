//! The demo workspace: the `examples/jsonplaceholder` requests against the public JSONPlaceholder
//! API, embedded so the app can create a ready-to-send workspace on first launch.

use std::path::{Path, PathBuf};

use crate::{Result, fs_util};

/// Folder name of the demo inside the parent directory.
pub const NAME: &str = "Plumbq demo";

/// Workspace-relative path and content of every demo file.
const FILES: [(&str, &str); 10] = [
    (
        "environments/public.toml",
        include_str!("../../../examples/jsonplaceholder/environments/public.toml"),
    ),
    (
        "folder.toml",
        include_str!("../../../examples/jsonplaceholder/folder.toml"),
    ),
    (
        "posts/create-post.http",
        include_str!("../../../examples/jsonplaceholder/posts/create-post.http"),
    ),
    (
        "posts/delete-post.http",
        include_str!("../../../examples/jsonplaceholder/posts/delete-post.http"),
    ),
    (
        "posts/folder.toml",
        include_str!("../../../examples/jsonplaceholder/posts/folder.toml"),
    ),
    (
        "posts/get-post-author.http",
        include_str!("../../../examples/jsonplaceholder/posts/get-post-author.http"),
    ),
    (
        "posts/get-post.http",
        include_str!("../../../examples/jsonplaceholder/posts/get-post.http"),
    ),
    (
        "posts/list-posts.http",
        include_str!("../../../examples/jsonplaceholder/posts/list-posts.http"),
    ),
    (
        "users/list-users.http",
        include_str!("../../../examples/jsonplaceholder/users/list-users.http"),
    ),
    (
        "users/user-todos.http",
        include_str!("../../../examples/jsonplaceholder/users/user-todos.http"),
    ),
];

/// Creates the demo workspace in `parent` and returns its root. An existing demo is reused as
/// it is, so the user's edits, moves and deletions stay.
pub fn create(parent: &Path) -> Result<PathBuf> {
    let root = parent.join(NAME);
    if root.exists() {
        return Ok(root);
    }
    for (rel, content) in FILES {
        fs_util::write_atomic(&root.join(rel), content)?;
    }
    Ok(root)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::workspace::tests::temp_workspace;
    use crate::{environments, workspace};

    #[test]
    fn creates_a_working_workspace_and_keeps_edits_on_reuse() {
        let parent = temp_workspace("demo");
        let root = create(&parent).unwrap();

        let tree = workspace::tree(&root).unwrap();
        assert!(format!("{tree:?}").contains("posts/list-posts.http"));
        assert_eq!(environments::list(&root).unwrap()[0].name, "public");

        fs::write(
            root.join("posts/list-posts.http"),
            "GET https://changed.test\n",
        )
        .unwrap();
        create(&parent).unwrap();
        let text = fs::read_to_string(root.join("posts/list-posts.http")).unwrap();
        assert_eq!(text, "GET https://changed.test\n");
    }

    #[test]
    fn requests_the_user_removed_stay_removed() {
        let parent = temp_workspace("demo-removed");
        let root = create(&parent).unwrap();
        fs::remove_file(root.join("posts/delete-post.http")).unwrap();
        fs::remove_dir_all(root.join("users")).unwrap();

        create(&parent).unwrap();
        assert!(!root.join("posts/delete-post.http").exists());
        assert!(!root.join("users").exists());
    }
}
