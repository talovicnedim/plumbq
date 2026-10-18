//! History follows requests whose ids change (moves, renames, shifts in a multi-request file).

use std::fs;
use std::path::Path;

use super::{HistoryEntry, dir_name, entry_files, prune, request_dir, request_dirs};
use crate::requests::IdChange;
use crate::{Result, fs_util};

/// Moves history along with requests whose ids changed (moves, folder renames, ids shifting in
/// a multi-request file), so chaining and comparisons keep working after reorganizing. A
/// removed request's history is deleted, so a later request at the same id doesn't inherit it.
pub fn follow(history_dir: &Path, root: &Path, changes: &[IdChange]) -> Result<()> {
    // Two phases through staging directories, because shifts overlap (`#3` becomes `#2` while
    // `#2` is removed): everything moves out first, then into its new place.
    let staging_root = history_dir
        .join(dir_name(&root.display().to_string()))
        .join(format!(".moving-{}", std::process::id()));
    let mut staged = Vec::new();
    for (i, change) in changes.iter().enumerate() {
        let staging = staging_root.join(i.to_string());
        for dir in request_dirs(history_dir, root, &change.from) {
            if !dir.is_dir() {
                continue;
            }
            match change.to {
                Some(_) => move_entries(&dir, &staging, None)?,
                None => fs::remove_dir_all(&dir)?,
            }
        }
        if let Some(to) = &change.to {
            staged.push((staging, to));
        }
    }
    for (staging, to) in staged {
        if staging.is_dir() {
            let dest = request_dir(history_dir, root, to);
            move_entries(&staging, &dest, Some(to))?;
            prune(&dest)?;
        }
    }
    if staging_root.is_dir() {
        fs::remove_dir_all(&staging_root)?;
    }
    Ok(())
}

// With `request_id`, entries are rewritten to name it.
fn move_entries(from: &Path, to: &Path, request_id: Option<&str>) -> Result<()> {
    fs_util::create_private_dir(to)?;
    for path in entry_files(from)? {
        let Some(name) = path.file_name() else {
            continue;
        };
        let target = to.join(name);
        match request_id {
            None => fs::rename(&path, &target)?,
            Some(id) => {
                let text = fs::read_to_string(&path)?;
                // An unreadable entry is moved as is rather than lost.
                match serde_json::from_str::<HistoryEntry>(&text) {
                    Ok(mut entry) => {
                        entry.request_id = id.to_string();
                        fs_util::write_private(&target, serde_json::to_string(&entry)?)?;
                        fs::remove_file(&path)?;
                    }
                    Err(_) => fs::rename(&path, &target)?,
                }
            }
        }
    }
    fs::remove_dir_all(from)?;
    Ok(())
}
