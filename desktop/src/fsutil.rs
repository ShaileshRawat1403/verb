//! File writes that either land whole or not at all.
//!
//! Verb's durable records (project identities, the work ledger, continuity files) are small, and a
//! half-written one is worse than a missing one: a reader cannot tell a torn record from a record
//! that says something strange. Every writer goes through [`atomic_write`], which has three
//! properties the per-module copies it replaces did not all share:
//!
//! * the temporary file is created owner-only (`0600`) *before* any byte is written, so there is no
//!   window in which another account can read it;
//! * its name is unique, so two writers of the same record cannot interleave into one temporary;
//! * the directory is synced after the rename, so the new name survives a crash, not only the data.

use crate::new_id;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;

/// Replaces `path` with `bytes`, creating its parent directory if needed.
pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let name = path
        .file_name()
        .ok_or_else(|| format!("invalid record path: {}", path.display()))?
        .to_string_lossy();
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    let temporary = parent.join(format!(".{name}.{}.tmp", new_id()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = options
        .open(&temporary)
        .map_err(|error| format!("could not create {}: {error}", temporary.display()))
        .and_then(|mut file| {
            file.write_all(bytes)
                .and_then(|()| file.sync_all())
                .map_err(|error| format!("could not save {}: {error}", path.display()))
        })
        .and_then(|()| {
            fs::rename(&temporary, path)
                .map_err(|error| format!("could not publish {}: {error}", path.display()))
        });
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
        return result;
    }
    #[cfg(unix)]
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("could not sync {}: {error}", parent.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("verb-fsutil-{label}-{}", new_id()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn a_record_is_replaced_whole_and_leaves_no_temporary_behind() {
        let root = scratch("replace");
        let path = root.join("nested").join("record.json");
        atomic_write(&path, b"first").unwrap();
        atomic_write(&path, b"second").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
        let leftovers: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_record_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let root = scratch("mode");
        let path = root.join("record.json");
        atomic_write(&path, b"private").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_failed_write_removes_its_temporary() {
        let root = scratch("fail");
        // A directory where the record should go makes the rename fail after the temporary exists.
        let path = root.join("occupied");
        fs::create_dir_all(path.join("child")).unwrap();
        assert!(atomic_write(&path, b"x").is_err());
        let leftovers: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
