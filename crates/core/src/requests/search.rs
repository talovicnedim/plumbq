//! Finding requests in the tree by words, tag and method.

use crate::workspace::TreeNode;

/// Requests matching every word of `query` (in name, URL, id or tags), a tag and a method,
/// in tree order.
pub fn search<'a>(
    nodes: &'a [TreeNode],
    query: &str,
    tag: Option<&str>,
    method: Option<&str>,
) -> Vec<&'a TreeNode> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut found = Vec::new();
    for node in nodes {
        match node {
            TreeNode::Folder { children, .. } => {
                found.extend(search(children, query, tag, method));
            }
            TreeNode::Request {
                id,
                name,
                method: m,
                url,
                tags,
            } => {
                let text = format!("{name} {url} {id} {}", tags.join(" ")).to_lowercase();
                let matches = words.iter().all(|w| text.contains(w))
                    && tag.is_none_or(|t| tags.iter().any(|own| own.eq_ignore_ascii_case(t)))
                    && method.is_none_or(|want| m.eq_ignore_ascii_case(want));
                if matches {
                    found.push(node);
                }
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_matches_words_tags_and_methods_anywhere_in_the_tree() {
        let request = |id: &str, name: &str, method: &str, tags: &[&str]| TreeNode::Request {
            id: id.into(),
            name: name.into(),
            method: method.into(),
            url: format!("{{{{baseUrl}}}}/{name}"),
            tags: tags.iter().map(|t| t.to_string()).collect(),
        };
        let nodes = vec![
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
        ];
        let ids = |found: Vec<&TreeNode>| -> Vec<String> {
            found
                .into_iter()
                .filter_map(|n| match n {
                    TreeNode::Request { id, .. } => Some(id.clone()),
                    TreeNode::Folder { .. } => None,
                })
                .collect()
        };
        assert_eq!(
            ids(search(&nodes, "", None, Some("post"))),
            ["users/admin/ban.http", "login.http"]
        );
        assert_eq!(
            ids(search(&nodes, "USERS list", None, None)),
            ["users/list.http"]
        );
        assert_eq!(ids(search(&nodes, "", Some("AUTH"), None)), ["login.http"]);
    }
}
