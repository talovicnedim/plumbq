//! Recently opened workspaces, most recent first, in a JSON file chosen by the caller.

use std::fs;
use std::path::Path;

use crate::workspace::{self, WorkspaceSummary};
use crate::{Error, Result};

/// Older workspaces drop off the list when a new one is opened.
const KEEP: usize = 10;

/// The recent workspaces with the names the app shows for them.
pub fn list(path: &Path) -> Result<Vec<WorkspaceSummary>> {
    Ok(roots(path)?
        .iter()
        .map(|root| workspace::summary(Path::new(root)))
        .collect())
}

/// Moves `root` to the front.
pub fn add(path: &Path, root: &str) -> Result<()> {
    let mut all = roots(path)?;
    all.retain(|r| r != root);
    all.insert(0, root.to_string());
    all.truncate(KEEP);
    crate::fs_util::write_private(path, serde_json::to_string_pretty(&all)?)?;
    Ok(())
}

fn roots(path: &Path) -> Result<Vec<String>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let all: Vec<String> = serde_json::from_str(&fs::read_to_string(path)?).map_err(|err| {
        Error::Invalid(format!(
            "failed to read the list of recent workspaces {}: {err} (fix or delete the file)",
            path.display()
        ))
    })?;
    // Drop folders that were moved or deleted since.
    Ok(all
        .into_iter()
        .filter(|root| Path::new(root).is_dir())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::tests::temp_workspace;

    #[test]
    fn add_moves_to_front_and_skips_missing_folders() {
        let dir = temp_workspace("recent");
        let file = dir.join("recent.json");
        let a = temp_workspace("recent-a").display().to_string();
        let b = temp_workspace("recent-b").display().to_string();

        add(&file, &a).unwrap();
        add(&file, &b).unwrap();
        add(&file, &a).unwrap();
        add(&file, "/does/not/exist").unwrap();

        let roots: Vec<String> = list(&file).unwrap().into_iter().map(|w| w.root).collect();
        assert_eq!(roots, [a, b]);
    }

    #[test]
    fn recent_workspaces_carry_the_name_the_app_shows() {
        let dir = temp_workspace("recent-names");
        let file = dir.join("recent.json");
        let named = temp_workspace("recent-named");
        workspace::rename(&named, "Billing API").unwrap();
        add(&file, &named.display().to_string()).unwrap();

        assert_eq!(list(&file).unwrap()[0].name, "Billing API");
    }

    #[test]
    fn unreadable_list_names_the_file_to_fix() {
        let dir = temp_workspace("recent-broken");
        let file = dir.join("recent.json");
        std::fs::write(&file, "not json").unwrap();

        let err = list(&file).unwrap_err().to_string();

        assert!(err.contains(&file.display().to_string()), "{err}");
        assert!(err.contains("delete"), "{err}");
    }
}
