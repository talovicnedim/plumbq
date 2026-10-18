//! The workspace's display name: `name` in the root `folder.toml`, or else the folder's name.
//! Renaming never moves the folder, so keychain secrets, history and cookies stay where they are.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use toml_edit::{DocumentMut, value};

use super::FOLDER_FILE;
use crate::{Error, Result, fs_util};

/// Longer names don't fit the sidebar.
const MAX_CHARS: usize = 80;

/// A workspace as headers and lists show it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSummary {
    pub root: String,
    pub name: String,
}

pub fn summary(root: &Path) -> WorkspaceSummary {
    WorkspaceSummary {
        root: root.display().to_string(),
        name: name(root),
    }
}

/// The name the app shows for the workspace at `root`. A broken `folder.toml` shows the folder's
/// name; the tree already reports the file.
pub fn name(root: &Path) -> String {
    display_name(root).unwrap_or_else(|| folder_name(root))
}

/// Sets the display name in the root `folder.toml`, keeping everything else in the file. An
/// empty name removes it, so the folder's name shows again.
pub fn rename(root: &Path, name: &str) -> Result<WorkspaceSummary> {
    let name = name.trim();
    if name.chars().count() > MAX_CHARS || name.chars().any(char::is_control) {
        return Err(Error::Invalid(format!(
            "a workspace name is one line of up to {MAX_CHARS} characters"
        )));
    }
    let path = root.join(FOLDER_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err.into()),
    };
    if name.is_empty() && text.is_empty() {
        return Ok(summary(root));
    }
    let mut doc: DocumentMut = text
        .parse()
        .map_err(|err| Error::Invalid(format!("failed to read {}: {err}", path.display())))?;
    match name.is_empty() {
        true => {
            doc.remove("name");
        }
        false => doc["name"] = value(name),
    }
    fs_util::write_atomic(&path, doc.to_string())?;
    Ok(summary(root))
}

fn display_name(root: &Path) -> Option<String> {
    let text = fs::read_to_string(root.join(FOLDER_FILE)).ok()?;
    let doc: DocumentMut = text.parse().ok()?;
    let name = doc.get("name")?.as_str()?.trim();
    (!name.is_empty()).then(|| name.to_string())
}

fn folder_name(root: &Path) -> String {
    root.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::tests::{temp_workspace, write};

    #[test]
    fn the_folder_name_shows_until_a_name_is_set() {
        let root = temp_workspace("name-default");
        let folder = root.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(name(&root), folder);

        assert_eq!(
            rename(&root, "  Jellyreach API ").unwrap().name,
            "Jellyreach API"
        );
        assert_eq!(name(&root), "Jellyreach API");

        assert_eq!(rename(&root, "").unwrap().name, folder);
        assert_eq!(name(&root), folder);
    }

    #[test]
    fn renaming_keeps_the_rest_of_folder_toml() {
        let root = temp_workspace("name-keeps");
        let before = "# shared with the team\norder = [\"users\"]\n\n[headers]\n# the gateway needs it\nX-Team = \"core\"\n";
        write(&root, "folder.toml", before);

        rename(&root, "Core API").unwrap();
        let after = fs::read_to_string(root.join("folder.toml")).unwrap();
        assert!(after.contains("# shared with the team"), "{after}");
        assert!(after.contains("# the gateway needs it"), "{after}");
        assert!(after.contains("name = \"Core API\""), "{after}");
        assert_eq!(
            crate::workspace::read_settings(&root).unwrap().order,
            ["users"]
        );

        rename(&root, "").unwrap();
        assert_eq!(
            fs::read_to_string(root.join("folder.toml")).unwrap(),
            before
        );
    }

    #[test]
    fn names_with_line_breaks_or_too_long_are_refused() {
        let root = temp_workspace("name-invalid");
        assert!(rename(&root, "two\nlines").is_err());
        assert!(rename(&root, &"x".repeat(81)).is_err());
        assert!(!root.join("folder.toml").exists());
    }

    #[test]
    fn a_broken_folder_toml_shows_the_folder_name() {
        let root = temp_workspace("name-broken");
        write(&root, "folder.toml", "name = \"unclosed\n");
        let folder = root.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(name(&root), folder);
        assert!(rename(&root, "New").is_err());
    }
}
