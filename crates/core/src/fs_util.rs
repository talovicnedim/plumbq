//! File writes that never leave a half-written file behind, and never show a private file to
//! other users, not even for a moment.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const PRIVATE_FILE: u32 = 0o600;
#[cfg(unix)]
const PRIVATE_DIR: u32 = 0o700;

/// Writes to a hidden temp file next to `path`, then renames it over `path` in one step.
pub fn write_atomic(path: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    write_with_mode(path, contents.as_ref(), None)
}

/// Like [`write_atomic`], readable by the current user only: for tokens, keys and anything
/// the user typed.
pub fn write_private(path: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        create_private_dir(dir)?;
    }
    write_with_mode(path, contents.as_ref(), Some(PRIVATE_FILE))
}

/// For a file another app owns (an AI app's config): keeps its current mode, or makes a new
/// file private.
pub fn write_keep_mode(path: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        create_private_dir(dir)?;
    }
    let mode = existing_mode(path).unwrap_or(PRIVATE_FILE);
    write_with_mode(path, contents.as_ref(), Some(mode))
}

/// Like [`write_atomic`], but fails with `AlreadyExists` instead of replacing a file, so two
/// writers racing for one name never overwrite each other.
pub fn write_new(path: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    write_new_with(path, contents.as_ref(), |from, to| fs::hard_link(from, to))
}

fn write_new_with(
    path: &Path,
    contents: &[u8],
    link: fn(&Path, &Path) -> io::Result<()>,
) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = write_temp(path, contents, None)?;
    let placed = place_new(&tmp, path, link);
    let _ = fs::remove_file(&tmp);
    placed
}

/// A hard link fails when `path` exists, and shows the whole file in one step. Some file
/// systems have no hard links (exFAT, some network shares; the error differs per system), so
/// any other failure claims the name with an empty file first and renames the temp file over it.
fn place_new(tmp: &Path, path: &Path, link: fn(&Path, &Path) -> io::Result<()>) -> io::Result<()> {
    match link(tmp, path) {
        Err(err) if err.kind() != io::ErrorKind::AlreadyExists => {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?;
            fs::rename(tmp, path)
        }
        linked => linked,
    }
}

/// Adds `line` to the end of a log only the current user can read; a new file is private from
/// the start. Appends are not atomic: a crash can leave a partial last line.
pub fn append_private(path: &Path, line: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        create_private_dir(dir)?;
    }
    let mut options = fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, PRIVATE_FILE);
    // One write call, so lines from two processes don't mix.
    options
        .open(path)?
        .write_all(format!("{line}\n").as_bytes())
}

/// Existing folders are left alone.
pub(crate) fn create_private_dir(dir: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, PRIVATE_DIR);
    builder.create(dir)
}

fn write_with_mode(path: &Path, contents: &[u8], mode: Option<u32>) -> io::Result<()> {
    let tmp = write_temp(path, contents, mode)?;
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

/// A hidden temp file next to `path` with `contents`. It gets `mode` when it is created, so the
/// content is never readable by others.
fn write_temp(path: &Path, contents: &[u8], mode: Option<u32>) -> io::Result<PathBuf> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    // Hidden (leading dot) so the workspace tree and the file watcher ignore it.
    let tmp = path.with_file_name(format!(".{name}.{}.tmp", uuid::Uuid::new_v4().simple()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    create_with_mode(&mut options, mode);
    let written = options.open(&tmp).and_then(|mut file| {
        file.write_all(contents)?;
        file.sync_all()
    });
    // The umask can narrow the mode; set it exactly.
    let written = written.and_then(|()| set_mode(&tmp, mode));
    if let Err(err) = written {
        let _ = fs::remove_file(&tmp);
        return Err(err);
    }
    Ok(tmp)
}

#[cfg(unix)]
fn create_with_mode(options: &mut fs::OpenOptions, mode: Option<u32>) {
    if let Some(mode) = mode {
        std::os::unix::fs::OpenOptionsExt::mode(options, mode);
    }
}

#[cfg(not(unix))]
fn create_with_mode(_: &mut fs::OpenOptions, _: Option<u32>) {}

#[cfg(unix)]
fn set_mode(path: &Path, mode: Option<u32>) -> io::Result<()> {
    match mode {
        Some(mode) => fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(mode)),
        None => Ok(()),
    }
}

#[cfg(not(unix))]
fn set_mode(_: &Path, _: Option<u32>) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn existing_mode(path: &Path) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .ok()
        .map(|m| m.permissions().mode() & 0o777)
}

#[cfg(not(unix))]
fn existing_mode(_: &Path) -> Option<u32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::tests::temp_workspace;

    #[test]
    fn replaces_the_file_and_leaves_no_temp_files() {
        let dir = temp_workspace("atomic");
        let path = dir.join("jar.json");
        write_atomic(&path, "old").unwrap();
        write_atomic(&path, "new").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    }

    #[test]
    fn new_files_never_replace_existing_ones() {
        let dir = temp_workspace("write-new");
        let path = dir.join("a.http");
        write_new(&path, "first").unwrap();
        let err = write_new(&path, "second").unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&path).unwrap(), "first");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    }

    #[test]
    fn new_files_work_where_hard_links_dont() {
        let dir = temp_workspace("write-new-no-links");
        let path = dir.join("a.http");
        let no_links: fn(&Path, &Path) -> io::Result<()> =
            |_, _| Err(io::Error::from(io::ErrorKind::Unsupported));
        write_new_with(&path, b"first", no_links).unwrap();
        let err = write_new_with(&path, b"second", no_links).unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&path).unwrap(), "first");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn private_files_are_owner_only_and_other_modes_stay() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_workspace("private");
        let private = dir.join("nested").join("keys.json");
        write_private(&private, "secret").unwrap();
        assert_eq!(existing_mode(&private), Some(PRIVATE_FILE));
        assert_eq!(existing_mode(private.parent().unwrap()), Some(PRIVATE_DIR));

        let shared = dir.join("config.json");
        write_atomic(&shared, "{}").unwrap();
        fs::set_permissions(&shared, fs::Permissions::from_mode(0o644)).unwrap();
        write_keep_mode(&shared, "{\"a\":1}").unwrap();
        assert_eq!(existing_mode(&shared), Some(0o644));
    }

    #[cfg(unix)]
    #[test]
    fn appended_logs_are_private_from_the_start() {
        let dir = temp_workspace("append");
        let log = dir.join("logs").join("activity.jsonl");
        append_private(&log, "one").unwrap();
        append_private(&log, "two").unwrap();
        assert_eq!(fs::read_to_string(&log).unwrap(), "one\ntwo\n");
        assert_eq!(existing_mode(&log), Some(PRIVATE_FILE));
        assert_eq!(existing_mode(log.parent().unwrap()), Some(PRIVATE_DIR));
    }
}
