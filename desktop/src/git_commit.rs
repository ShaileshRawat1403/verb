//! Native-local selected-file commits. Private index, reviewed bytes, ref CAS, no staging takeover.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Review {
    pub version: String,
    pub files: Vec<String>,
    pub unsupported: std::collections::BTreeMap<String, String>,
    pub blocked: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Request {
    pub version: String,
    pub paths: Vec<String>,
}
type FileState = Option<(String, u32)>;
struct Snapshot {
    states: std::collections::BTreeMap<String, FileState>,
    review: Review,
    branch: String,
    head: String,
}
fn command(project: &Path, args: &[&str], index: Option<&Path>) -> Result<Vec<u8>, String> {
    let mut cmd = crate::exec::git(project).ok_or("Git unavailable")?;
    cmd.args(args).env("GIT_LITERAL_PATHSPECS", "1");
    if let Some(index) = index {
        cmd.env("GIT_INDEX_FILE", index);
    }
    let out = if args.first() == Some(&"cat-file") {
        crate::exec::read_output(&mut cmd)
    } else {
        crate::exec::git_operation_output(&mut cmd)
    }
    .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_owned());
    }
    Ok(out.stdout)
}
fn text(project: &Path, args: &[&str], index: Option<&Path>) -> Result<String, String> {
    String::from_utf8(command(project, args, index)?)
        .map(|s| s.trim_end().to_owned())
        .map_err(|_| "Git paths must be UTF-8 for guided commits".into())
}
fn file(project: &Path, path: &str) -> Result<Option<(String, u32)>, String> {
    let mut target = project.to_path_buf();
    for component in Path::new(path).components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err("Invalid file path".into());
        }
        target.push(component);
        if fs::symlink_metadata(&target).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("Review symbolic links in the terminal before committing".into());
        }
    }
    let meta = match fs::metadata(&target) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if !meta.is_file() || meta.len() > 8 * 1024 * 1024 {
        return Err("Use the terminal for submodules or files larger than 8 MB".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(target)
        .map_err(|e| e.to_string())?
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("File grew beyond the review limit".into());
    }
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o111 != 0 {
            0o100755
        } else {
            0o100644
        }
    };
    #[cfg(not(unix))]
    let mode = 0o100644;
    Ok(Some((format!("{:x}", Sha256::digest(&bytes)), mode)))
}
fn snapshot(project: &Path) -> Result<Snapshot, String> {
    if crate::isolation::external() {
        return Err("Selected-file commits need native-local workspace support. Use the terminal on this host.".into());
    }
    let branch = text(project, &["symbolic-ref", "--quiet", "HEAD"], None)?;
    if !branch.starts_with("refs/heads/") {
        return Err("Choose a branch in the terminal first".into());
    }
    let head = text(project, &["rev-parse", "--verify", "HEAD"], None).unwrap_or_default();
    let status = command(
        project,
        &["status", "--porcelain=v1", "-z", "--no-renames", "-uall"],
        None,
    )?;
    let status = String::from_utf8(status).map_err(|_| "Use the terminal for non-UTF-8 paths")?;
    let mut blocked = None;
    if !command(project, &["diff", "--cached", "--name-only", "-z"], None)?.is_empty() {
        blocked = Some("You already have staged changes. Commit or unstage them in the terminal first; Verb will not change your staging area.".into());
    }
    for marker in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "rebase-merge",
        "rebase-apply",
    ] {
        if PathBuf::from(text(
            project,
            &["rev-parse", "--path-format=absolute", "--git-path", marker],
            None,
        )?)
        .exists()
        {
            blocked = Some("Finish the Git operation in your terminal first.".into());
        }
    }
    if text(project, &["config", "--get", "commit.gpgsign"], None).is_ok_and(|v| v == "true")
        || text(project, &["config", "--get", "core.hooksPath"], None).is_ok()
    {
        blocked = Some("This project uses signing or custom hooks. Commit in the terminal so those checks run.".into());
    }
    for hook in [
        "pre-commit",
        "prepare-commit-msg",
        "commit-msg",
        "post-commit",
        "reference-transaction",
    ] {
        let hook = PathBuf::from(text(
            project,
            &[
                "rev-parse",
                "--path-format=absolute",
                "--git-path",
                &format!("hooks/{hook}"),
            ],
            None,
        )?);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if fs::metadata(hook).is_ok_and(|m| m.permissions().mode() & 0o111 != 0) {
                blocked = Some(
                    "This project has commit hooks. Use the terminal so its checks run.".into(),
                );
            }
        }
    }
    let mut digest = Sha256::new();
    digest.update(format!("{branch}\0{head}\0{status}\0"));
    let mut files = Vec::new();
    let mut unsupported = std::collections::BTreeMap::new();
    let mut states = std::collections::BTreeMap::new();
    for row in status.split('\0').filter(|s| !s.is_empty()) {
        if row.len() < 4 {
            return Err("Unreadable Git status".into());
        }
        let path = &row[3..];
        if &row[..2] == " A" {
            blocked = Some("There are intent-to-add entries in your index. Finish staging in the terminal first.".into());
        }
        if row[..2].contains('U') || &row[..2] == "AA" || &row[..2] == "DD" {
            blocked = Some("Resolve conflicts in the terminal first.".into());
        }
        match file(project, path) {
            Ok(state) => {
                digest.update(format!("{path}\0{state:?}\0"));
                states.insert(path.to_owned(), state);
            }
            Err(reason) => {
                unsupported.insert(path.to_owned(), reason);
            }
        }
        files.push(path.to_owned());
    }
    if files.len() > 200 {
        blocked = Some("Use the terminal for more than 200 changed files.".into());
    }
    Ok(Snapshot {
        states,
        review: Review {
            version: format!("{:x}", digest.finalize()),
            files,
            unsupported,
            blocked,
        },
        branch,
        head,
    })
}
pub(crate) fn review(project: &Path) -> Result<Review, String> {
    Ok(snapshot(project)?.review)
}
struct Cleanup(Vec<PathBuf>);
impl Drop for Cleanup {
    fn drop(&mut self) {
        for p in &self.0 {
            let _ = fs::remove_file(p);
        }
    }
}

pub(crate) fn commit(
    project: &Path,
    message: &str,
    request: &Request,
    audit: Option<(PathBuf, String, String)>,
) -> Result<String, String> {
    if crate::isolation::external() {
        return Err("Selected-file commits need native-local workspace support. Use the terminal on this host.".into());
    }
    let message = message.trim();
    if message.is_empty() || message.len() > 4096 {
        return Err("Describe the commit in up to 4 KB of text".into());
    }
    let index = PathBuf::from(text(
        project,
        &["rev-parse", "--path-format=absolute", "--git-path", "index"],
        None,
    )?);
    let lock = index.with_file_name("index.lock");
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut locked = options
        .open(&lock)
        .map_err(|_| "Git is busy. Wait for the terminal operation, then reopen this review.")?;
    let temp = index.with_file_name(format!(
        "verb-commit-{}-{}",
        std::process::id(),
        crate::now_millis()
    ));
    let mut cleanup = Cleanup(vec![lock.clone()]);
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut private_index = options.open(&temp).map_err(|e| e.to_string())?;
    cleanup
        .0
        .extend([temp.clone(), temp.with_extension("lock")]);
    if index.exists() {
        let meta = fs::symlink_metadata(&index).map_err(|e| e.to_string())?;
        if !meta.is_file() || meta.len() > 64 * 1024 * 1024 {
            return Err("Use the terminal for this Git index".into());
        }
        private_index
            .write_all(&fs::read(&index).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    }
    drop(private_index);
    let snap = snapshot(project)?;
    if let Some(reason) = snap.review.blocked {
        return Err(reason);
    }
    if request.version != snap.review.version {
        return Err("Files or Git history changed. Close and reopen the commit review; nothing was committed.".into());
    }
    if request.paths.is_empty() || request.paths.len() > 200 {
        return Err("Select between 1 and 200 changed files".into());
    }
    if request
        .paths
        .iter()
        .any(|p| snap.review.unsupported.contains_key(p))
    {
        return Err(
            "The selection contains an unsupported file. Use the terminal for that file.".into(),
        );
    }
    let mut paths = request.paths.clone();
    paths.sort();
    paths.dedup();
    if paths.len() != request.paths.len() || paths.iter().any(|p| !snap.review.files.contains(p)) {
        return Err("The selection does not match this review".into());
    }
    let mut audit_written = false;
    let outcome = (|| {
        let mut expected = snap.states.clone();
        if let Some((path, before, after)) = &audit {
            if fs::read_to_string(path).map_err(|e| e.to_string())? != *before {
                return Err("The Spec changed; reopen the commit review".into());
            }
            let relative = path
                .strip_prefix(project)
                .map_err(|_| "Spec outside project")?
                .to_str()
                .ok_or("Spec path is not UTF-8")?
                .to_owned();
            if !paths.contains(&relative) {
                paths.push(relative.clone());
            }
            let mode = file(project, &relative)?.ok_or("Spec disappeared")?.1;
            expected.insert(
                relative,
                Some((format!("{:x}", Sha256::digest(after.as_bytes())), mode)),
            );
            crate::specs::write_atomic(path, after)?;
            audit_written = true;
        }
        let expected = paths
            .iter()
            .map(|p| (p.clone(), expected.get(p).cloned().unwrap_or(None)))
            .collect::<Vec<_>>();
        if !index.exists() {
            fs::remove_file(&temp).map_err(|e| e.to_string())?;
            command(project, &["read-tree", "--empty"], Some(&temp))?;
        }
        let mut args = vec!["add", "--all", "--"];
        args.extend(paths.iter().map(String::as_str));
        command(project, &args, Some(&temp))?;
        for (path, state) in expected {
            let row = text(project, &["ls-files", "--stage", "--", &path], Some(&temp))?;
            match state {
                None if row.is_empty() => {}
                Some((hash, mode)) => {
                    let metadata = row
                        .split('\t')
                        .next()
                        .unwrap_or("")
                        .split_whitespace()
                        .collect::<Vec<_>>();
                    if metadata.len() != 3 || u32::from_str_radix(metadata[0], 8).ok() != Some(mode)
                    {
                        return Err("File mode changed during review; reopen it".into());
                    }
                    let blob = command(project, &["cat-file", "blob", metadata[1]], None)?;
                    if format!("{:x}", Sha256::digest(blob)) != hash {
                        return Err("File content or a Git filter changed during review; reopen it. Nothing committed.".into());
                    }
                }
                _ => return Err("File changed while staging; reopen the review".into()),
            }
        }
        let tree = text(project, &["write-tree"], Some(&temp))?;
        let mut args = vec!["commit-tree", &tree];
        if !snap.head.is_empty() {
            args.extend(["-p", &snap.head]);
        }
        args.extend(["-m", message]);
        let oid = text(project, &args, Some(&temp))?;
        let bytes = fs::read(&temp).map_err(|e| e.to_string())?;
        locked
            .write_all(&bytes)
            .and_then(|_| locked.sync_all())
            .map_err(|e| e.to_string())?;
        let old = if snap.head.is_empty() {
            "0".repeat(oid.len())
        } else {
            snap.head.clone()
        };
        command(
            project,
            &[
                "update-ref",
                "-m",
                "Verb selected-file commit",
                &snap.branch,
                &oid,
                &old,
            ],
            None,
        )?;
        // The ref now names a commit. Never roll back its audit or offer an automatic retry.
        audit_written = false;
        fs::rename(&lock, &index).map_err(|_| format!("Commit {} exists, but the Git index could not be updated. Inspect git status in the terminal before doing anything else.", &oid[..8]))?;
        Ok(oid[..8].to_owned())
    })();
    if audit_written {
        if let Some((path, before, after)) = &audit {
            if fs::read_to_string(path).ok().as_ref() == Some(after) {
                let _ = crate::specs::write_atomic(path, before);
            }
        }
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "verb-select-{}-{}-{}",
            std::process::id(),
            crate::now_millis(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ));
        fs::create_dir_all(&p).unwrap();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.name", "Test"],
            vec!["config", "user.email", "test@example.invalid"],
            vec!["config", "commit.gpgsign", "false"],
            vec!["commit", "--allow-empty", "-qm", "init"],
        ] {
            text(&p, &args, None).unwrap();
        }
        p
    }
    fn request(p: &Path, paths: &[&str]) -> Request {
        Request {
            version: review(p).unwrap().version,
            paths: paths.iter().map(|s| s.to_string()).collect(),
        }
    }
    #[test]
    fn selects_literal_paths_and_retains_unselected_files() {
        let p = fixture();
        for name in ["a.txt", "experiment.txt", ":(glob)*.txt", "space ü.txt"] {
            fs::write(p.join(name), name).unwrap();
        }
        let r = request(&p, &["a.txt", ":(glob)*.txt", "space ü.txt"]);
        commit(&p, "selected", &r, None).unwrap();
        let files = text(&p, &["ls-tree", "--name-only", "HEAD"], None).unwrap();
        assert!(files.contains("a.txt") && files.contains(":(glob)*.txt"));
        assert!(!files.contains("experiment.txt"));
        assert_eq!(
            fs::read_to_string(p.join("experiment.txt")).unwrap(),
            "experiment.txt"
        );
        assert!(text(&p, &["diff", "--cached", "--name-only"], None)
            .unwrap()
            .is_empty());
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn stale_content_and_unknown_selections_are_refused_without_staging() {
        let p = fixture();
        fs::write(p.join("a"), "first").unwrap();
        let r = request(&p, &["a"]);
        fs::write(p.join("a"), "newer").unwrap();
        assert!(commit(&p, "x", &r, None).unwrap_err().contains("changed"));
        let r = request(&p, &["../outside"]);
        assert!(commit(&p, "x", &r, None).is_err());
        assert!(text(&p, &["diff", "--cached", "--name-only"], None)
            .unwrap()
            .is_empty());
        assert!(!p.join(".git/index.lock").exists());
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn preexisting_staging_is_preserved_byte_for_byte() {
        let p = fixture();
        fs::write(p.join("a"), "staged").unwrap();
        command(&p, &["add", "a"], None).unwrap();
        let before = fs::read(p.join(".git/index")).unwrap();
        assert!(review(&p).unwrap().blocked.unwrap().contains("staged"));
        let r = request(&p, &["a"]);
        assert!(commit(&p, "x", &r, None).is_err());
        assert_eq!(before, fs::read(p.join(".git/index")).unwrap());
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn deletes_only_the_selected_file() {
        let p = fixture();
        fs::write(p.join("a"), "a").unwrap();
        fs::write(p.join("b"), "b").unwrap();
        commit(&p, "both", &request(&p, &["a", "b"]), None).unwrap();
        fs::remove_file(p.join("a")).unwrap();
        fs::write(p.join("b"), "later").unwrap();
        commit(&p, "delete a", &request(&p, &["a"]), None).unwrap();
        assert_eq!(text(&p, &["show", "HEAD:b"], None).unwrap(), "b");
        assert_eq!(fs::read_to_string(p.join("b")).unwrap(), "later");
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn audit_is_included_and_failure_restores_only_its_own_edit() {
        let p = fixture();
        fs::write(p.join("a"), "a").unwrap();
        fs::write(p.join("spec.md"), "before").unwrap();
        let r = request(&p, &["a"]);
        commit(
            &p,
            "with audit",
            &r,
            Some((p.join("spec.md"), "before".into(), "after".into())),
        )
        .unwrap();
        assert_eq!(text(&p, &["show", "HEAD:spec.md"], None).unwrap(), "after");
        fs::write(p.join("a"), "new").unwrap();
        let r = request(&p, &["a"]);
        text(&p, &["config", "user.name", ""], None).unwrap();
        let outcome = commit(
            &p,
            "failure",
            &r,
            Some((p.join("spec.md"), "after".into(), "attempt".into())),
        );
        assert!(outcome.is_err());
        assert_eq!(fs::read_to_string(p.join("spec.md")).unwrap(), "after");
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn supports_first_commit_without_existing_index() {
        let p = fixture();
        text(&p, &["update-ref", "-d", "refs/heads/main"], None).unwrap();
        let _ = fs::remove_file(p.join(".git/index"));
        fs::write(p.join("first"), "new").unwrap();
        commit(&p, "initial", &request(&p, &["first"]), None).unwrap();
        assert_eq!(text(&p, &["show", "HEAD:first"], None).unwrap(), "new");
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn protects_intent_to_add_and_existing_lock() {
        let p = fixture();
        fs::write(p.join("intent"), "later").unwrap();
        command(&p, &["add", "--intent-to-add", "intent"], None).unwrap();
        assert!(review(&p).unwrap().blocked.is_some());
        let before = fs::read(p.join(".git/index")).unwrap();
        fs::write(p.join(".git/index.lock"), "another Git process").unwrap();
        assert!(commit(&p, "x", &request(&p, &["intent"]), None).is_err());
        assert_eq!(
            fs::read_to_string(p.join(".git/index.lock")).unwrap(),
            "another Git process"
        );
        assert_eq!(fs::read(p.join(".git/index")).unwrap(), before);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn preserves_unselected_index_flags() {
        let p = fixture();
        for name in ["keep", "edit"] {
            fs::write(p.join(name), "base").unwrap();
        }
        commit(&p, "base", &request(&p, &["keep", "edit"]), None).unwrap();
        command(&p, &["update-index", "--assume-unchanged", "keep"], None).unwrap();
        fs::write(p.join("edit"), "new").unwrap();
        commit(&p, "edit", &request(&p, &["edit"]), None).unwrap();
        assert!(text(&p, &["ls-files", "-v", "--", "keep"], None)
            .unwrap()
            .starts_with('h'));
        fs::remove_dir_all(p).unwrap();
    }
}
