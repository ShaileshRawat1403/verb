//! Last-known-good: a state the user says worked, and how far the tree has moved since.
//!
//! Backlog C3. C2 measures what the *last command* did to the tree. That answers "what just
//! happened", but not the question people actually ask when something breaks: "what changed since it
//! last worked?" The C2 notes are explicit that the baseline must be "a state the *user* considered
//! working, rather than the previous command" -- measuring from "the last command that exited 0"
//! failed on the device, because the command that changes the tree usually exits 0 too.
//!
//! So Verb never decides what "good" is. `verb good mark` records it when the user says so, and
//! everything else is a comparison against that mark, read from Git at the moment of asking.
//!
//! What the mark stores, per checkout, follows the structural-memory rule:
//!
//! * when it was marked, the HEAD commit id, and how many files were uncommitted then;
//! * a SHA-256 fingerprint of the working tree (HEAD, porcelain status, the diff against HEAD, and
//!   the blob ids of untracked files), so "exactly as it was" can be answered without keeping any of
//!   those bytes.
//!
//! No file names, no branch name, no diff. File names are listed only by `verb good files`, read
//! live from Git and printed, never stored.

use crate::fsutil::atomic_write;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Beyond this much diff the fingerprint is skipped rather than holding it all in memory. The
/// comparison still works from commits and file counts; only "exactly as marked" becomes unknown.
const MAX_FINGERPRINT_DIFF_BYTES: usize = 64 * 1024 * 1024;
const MAX_FINGERPRINT_UNTRACKED: usize = 5_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Mark {
    schema_version: u8,
    /// Milliseconds since the Unix epoch.
    pub marked_at: u128,
    /// `None` in a repository with no commits yet.
    pub head: Option<String>,
    pub uncommitted: usize,
    /// `None` when the tree was too large to fingerprint.
    pub fingerprint: Option<String>,
}

/// How the tree now relates to the mark. Every field is a reading taken now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Distance {
    /// `Some(true)` only when HEAD and the fingerprint both match; `None` when either side has no
    /// fingerprint to compare.
    pub identical: Option<bool>,
    pub head_moved: bool,
    /// Commits reachable from HEAD but not from the marked commit.
    pub commits_since: Option<u64>,
    /// Commits in the marked state that HEAD no longer contains -- a reset or rebase dropped them.
    pub commits_dropped: Option<u64>,
    /// Paths that differ from the marked commit, counting uncommitted and untracked work.
    pub files_differ: Option<usize>,
    /// The marked commit no longer exists in this repository (garbage-collected, or a different
    /// clone). The distance is then unknown, not zero.
    pub mark_missing: bool,
}

pub(crate) fn mark(project: &Path) -> Result<Mark, String> {
    mark_in(&store_dir(project)?, project)
}

pub(crate) fn load(project: &Path) -> Result<Option<Mark>, String> {
    load_in(&store_dir(project)?, project)
}

pub(crate) fn forget(project: &Path) -> Result<bool, String> {
    forget_in(&store_dir(project)?, project)
}

fn mark_in(dir: &Path, project: &Path) -> Result<Mark, String> {
    if git(project, &["rev-parse", "--is-inside-work-tree"]).as_deref() != Some("true") {
        return Err("last-known-good needs a Git working tree".to_owned());
    }
    let mark = Mark {
        schema_version: 1,
        marked_at: crate::now_millis(),
        head: head(project),
        uncommitted: porcelain(project).map(|p| p.lines().count()).unwrap_or(0),
        fingerprint: fingerprint(project),
    };
    let bytes = serde_json::to_vec_pretty(&mark)
        .map_err(|error| format!("could not encode last-known-good: {error}"))?;
    atomic_write(&mark_path(dir, project), &bytes)?;
    Ok(mark)
}

fn load_in(dir: &Path, project: &Path) -> Result<Option<Mark>, String> {
    let path = mark_path(dir, project);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("could not read last-known-good: {error}")),
    };
    let mark: Mark = serde_json::from_str(&text)
        .map_err(|error| format!("invalid last-known-good record: {error}"))?;
    if mark.schema_version != 1
        || mark.head.as_deref().is_some_and(|head| !is_object_id(head))
        || mark
            .fingerprint
            .as_deref()
            .is_some_and(|value| value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err("invalid last-known-good record".to_owned());
    }
    Ok(Some(mark))
}

fn forget_in(dir: &Path, project: &Path) -> Result<bool, String> {
    match fs::remove_file(mark_path(dir, project)) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("could not forget last-known-good: {error}")),
    }
}

/// The distance from `mark` to the tree now. `with_fingerprint` is off for hot paths (the TUI band),
/// where hashing a large diff on every failed command would be felt.
pub(crate) fn distance(project: &Path, mark: &Mark, with_fingerprint: bool) -> Distance {
    let now_head = head(project);
    let head_moved = now_head != mark.head;
    let Some(marked) = mark.head.as_deref() else {
        // Marked before the first commit: every tracked or untracked path differs from nothing.
        return Distance {
            identical: None,
            head_moved,
            commits_since: None,
            commits_dropped: None,
            files_differ: porcelain(project).map(|p| p.lines().count()),
            mark_missing: false,
        };
    };
    let exists = git(
        project,
        &["cat-file", "-e", &format!("{marked}^{{commit}}")],
    )
    .is_some();
    if !exists {
        return Distance {
            identical: Some(false),
            head_moved,
            commits_since: None,
            commits_dropped: None,
            files_differ: None,
            mark_missing: true,
        };
    }
    let count = |range: String| {
        git(project, &["rev-list", "--count", &range]).and_then(|value| value.parse().ok())
    };
    let identical = if with_fingerprint {
        match (&mark.fingerprint, fingerprint(project)) {
            (Some(then), Some(now)) => Some(!head_moved && then == &now),
            _ => None,
        }
    } else {
        None
    };
    Distance {
        identical,
        head_moved,
        commits_since: count(format!("{marked}..HEAD")),
        commits_dropped: count(format!("HEAD..{marked}")),
        files_differ: files_differing(project, marked).map(|files| files.len()),
        mark_missing: false,
    }
}

/// The paths that differ from the marked commit, read now. Printed by `verb good files`; never
/// stored and never placed in JSON evidence that could reach a model.
pub(crate) fn files_differing(project: &Path, marked: &str) -> Option<Vec<String>> {
    let mut files: Vec<String> = git(project, &["diff", "--name-only", marked])?
        .lines()
        .map(str::to_owned)
        .collect();
    if let Some(untracked) = git(project, &["ls-files", "--others", "--exclude-standard"]) {
        files.extend(untracked.lines().map(str::to_owned));
    }
    files.sort();
    files.dedup();
    Some(files)
}

impl Distance {
    /// One line a person can read, e.g. `3 commits and 5 files since`.
    pub(crate) fn summary(&self) -> String {
        if self.mark_missing {
            return "the marked commit is no longer in this repository".to_owned();
        }
        if self.identical == Some(true) {
            return "exactly as marked".to_owned();
        }
        let mut parts = Vec::new();
        if let Some(commits) = self.commits_since.filter(|count| *count > 0) {
            parts.push(plural(commits as usize, "commit"));
        }
        if let Some(dropped) = self.commits_dropped.filter(|count| *count > 0) {
            parts.push(format!("{} dropped", plural(dropped as usize, "commit")));
        }
        match self.files_differ {
            Some(0) if parts.is_empty() => {
                return match self.identical {
                    Some(false) => "same files and commits, different content".to_owned(),
                    _ => "no file differs from the marked commit".to_owned(),
                }
            }
            Some(0) => {}
            Some(1) => parts.push("1 file differs".to_owned()),
            Some(files) => parts.push(format!("{files} files differ")),
            None => parts.push("file changes unknown".to_owned()),
        }
        parts.join(", ")
    }

    pub(crate) fn to_json(&self) -> String {
        let option_u64 = |value: Option<u64>| {
            value
                .map(|v| v.to_string())
                .unwrap_or_else(|| "null".to_owned())
        };
        format!(
            "{{\"identical\":{},\"headMoved\":{},\"commitsSince\":{},\"commitsDropped\":{},\"filesDiffer\":{},\"markMissing\":{}}}",
            self.identical
                .map(|v| v.to_string())
                .unwrap_or_else(|| "null".to_owned()),
            self.head_moved,
            option_u64(self.commits_since),
            option_u64(self.commits_dropped),
            self.files_differ
                .map(|v| v.to_string())
                .unwrap_or_else(|| "null".to_owned()),
            self.mark_missing
        )
    }
}

impl Mark {
    pub(crate) fn to_json(&self) -> String {
        format!(
            "{{\"markedAt\":\"{}\",\"head\":{},\"uncommitted\":{},\"fingerprinted\":{}}}",
            crate::iso8601(self.marked_at),
            self.head
                .as_deref()
                .map(|head| format!("\"{head}\""))
                .unwrap_or_else(|| "null".to_owned()),
            self.uncommitted,
            self.fingerprint.is_some()
        )
    }

    pub(crate) fn short_head(&self) -> Option<&str> {
        self.head.as_deref().map(|head| &head[..head.len().min(12)])
    }
}

fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// `verb good [status|mark|forget|files] [--json]`
pub(crate) fn command(project: &Path, args: &[String], json: bool) -> Result<(), String> {
    match args.first().map(String::as_str).unwrap_or("status") {
        "mark" => {
            let mark = mark(project)?;
            if json {
                println!("{}", mark.to_json());
            } else {
                println!(
                    "Marked last-known-good at {}{}.",
                    mark.short_head().unwrap_or("(no commits yet)"),
                    match mark.uncommitted {
                        0 => String::new(),
                        n => format!(" with {} uncommitted", plural(n, "file")),
                    }
                );
                if mark.uncommitted > 0 {
                    println!("Uncommitted work is fingerprinted, not saved: commit or stash it if you may need it back.");
                }
            }
            Ok(())
        }
        "forget" => {
            let forgotten = forget(project)?;
            if json {
                println!("{{\"forgotten\":{forgotten}}}");
            } else if forgotten {
                println!("Forgot last-known-good for this checkout.");
            } else {
                println!("No last-known-good was marked for this checkout.");
            }
            Ok(())
        }
        "files" => {
            let mark = load(project)?.ok_or("no last-known-good is marked; run verb good mark")?;
            let head = mark.head.as_deref().ok_or(
                "the mark was made before the first commit; there is nothing to diff against",
            )?;
            let files = files_differing(project, head)
                .ok_or("Git could not compare against the marked commit")?;
            if json {
                let items: Vec<String> = files
                    .iter()
                    .map(|file| format!("\"{}\"", crate::json_escape(file)))
                    .collect();
                println!("{{\"files\":[{}]}}", items.join(","));
            } else if files.is_empty() {
                println!("No file differs from the marked commit.");
            } else {
                for file in files {
                    println!("{file}");
                }
            }
            Ok(())
        }
        "status" => {
            let Some(mark) = load(project)? else {
                if json {
                    println!("{{\"mark\":null,\"distance\":null}}");
                } else {
                    println!("No last-known-good is marked for this checkout.");
                    println!("When the project is in a state that works, run: verb good mark");
                }
                return Ok(());
            };
            let distance = distance(project, &mark, true);
            if json {
                println!(
                    "{{\"mark\":{},\"distance\":{}}}",
                    mark.to_json(),
                    distance.to_json()
                );
                return Ok(());
            }
            println!(
                "Last known good: {} ({})",
                mark.short_head().unwrap_or("before the first commit"),
                crate::iso8601(mark.marked_at)
            );
            println!("Since then (observed now): {}", distance.summary());
            if mark.uncommitted > 0 {
                println!(
                    "The marked state also had {} uncommitted; files are compared against its commit.",
                    plural(mark.uncommitted, "file")
                );
            }
            if let (Some(short), false) = (mark.short_head(), distance.identical == Some(true)) {
                if !distance.mark_missing {
                    println!();
                    println!("Ways back, none run by Verb:");
                    let ways = [
                        ("verb good files".to_owned(), "list what differs"),
                        (format!("git diff {short}"), "see every change since"),
                        ("git stash push -u".to_owned(), "set current work aside"),
                        (
                            format!("git switch --detach {short}"),
                            "try the marked commit",
                        ),
                    ];
                    let width = ways.iter().map(|(way, _)| way.len()).max().unwrap_or(0);
                    for (way, meaning) in ways {
                        println!("  {way:<width$}  {meaning}");
                    }
                }
            }
            Ok(())
        }
        other => Err(format!(
            "unknown good command: {other}; use status, mark, forget or files"
        )),
    }
}

// ----- plumbing -------------------------------------------------------------------------------

/// One mark per checkout: worktrees of one project are marked independently, because "it worked"
/// is a statement about a working tree.
fn store_dir(project: &Path) -> Result<PathBuf, String> {
    Ok(crate::project::identity(project)?
        .store
        .join("last-known-good"))
}

fn mark_path(dir: &Path, project: &Path) -> PathBuf {
    let checkout = git(project, &["rev-parse", "--show-toplevel"])
        .map(PathBuf::from)
        .unwrap_or_else(|| project.to_path_buf());
    let key = format!(
        "{:x}",
        Sha256::digest(checkout.to_string_lossy().as_bytes())
    );
    dir.join(format!("{}.json", &key[..32]))
}

fn head(project: &Path) -> Option<String> {
    git(project, &["rev-parse", "--verify", "-q", "HEAD"]).filter(|id| is_object_id(id))
}

fn porcelain(project: &Path) -> Option<String> {
    git(
        project,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )
}

fn is_object_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn fingerprint(project: &Path) -> Option<String> {
    let mut hasher = Sha256::new();
    hasher.update(b"verb-lkg-v1\0");
    hasher.update(head(project).unwrap_or_default().as_bytes());
    hasher.update(b"\0");
    hasher.update(git_bytes(
        project,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?);
    hasher.update(b"\0");
    if head(project).is_some() {
        let diff = git_bytes(
            project,
            &["diff", "HEAD", "--binary", "--no-ext-diff", "--no-color"],
        )?;
        if diff.len() > MAX_FINGERPRINT_DIFF_BYTES {
            return None;
        }
        hasher.update(&diff);
    }
    hasher.update(b"\0");
    let untracked = git_bytes(
        project,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?;
    let paths: Vec<&[u8]> = untracked
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .collect();
    if paths.len() > MAX_FINGERPRINT_UNTRACKED {
        return None;
    }
    if !paths.is_empty() {
        // Blob ids of untracked content, computed by Git, so a changed untracked file changes the
        // fingerprint without Verb reading or keeping it.
        let mut input = Vec::new();
        for path in &paths {
            input.extend_from_slice(path);
            input.push(b'\n');
        }
        hasher.update(git_bytes_with_input(
            project,
            &["hash-object", "--stdin-paths"],
            &input,
        )?);
    }
    Some(format!("{:x}", hasher.finalize()))
}

fn git(project: &Path, args: &[&str]) -> Option<String> {
    git_bytes(project, args).map(|bytes| String::from_utf8_lossy(&bytes).trim().to_owned())
}

fn git_bytes(project: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(project)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

fn git_bytes_with_input(project: &Path, args: &[&str], input: &[u8]) -> Option<Vec<u8>> {
    let mut child = Command::new("git")
        .args(args)
        .current_dir(project)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdin = child.stdin.take()?;
    let input = input.to_vec();
    // Written from a thread so a large path list cannot deadlock against a full stdout pipe.
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&input);
    });
    let output = child.wait_with_output().ok()?;
    let _ = writer.join();
    output.status.success().then_some(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch {
        root: PathBuf,
        repo: PathBuf,
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn run(dir: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}: {output:?}");
    }

    fn scratch() -> Option<Scratch> {
        Command::new("git").arg("--version").output().ok()?;
        let root = std::env::temp_dir().join(format!("verb-good-{}", crate::new_id()));
        let repo = root.join("repo");
        fs::create_dir_all(&repo).unwrap();
        run(&repo, &["init", "-q", "-b", "main"]);
        fs::write(repo.join("a.txt"), "one\n").unwrap();
        run(&repo, &["add", "."]);
        run(&repo, &["commit", "-q", "-m", "one"]);
        Some(Scratch { root, repo })
    }

    impl Scratch {
        fn dir(&self) -> PathBuf {
            self.root.join("state")
        }
        fn mark(&self) -> Mark {
            mark_in(&self.dir(), &self.repo).unwrap()
        }
    }

    #[test]
    fn a_mark_compares_as_identical_until_the_tree_moves() {
        let Some(s) = scratch() else { return };
        let mark = s.mark();
        assert!(mark.head.is_some());
        assert_eq!(mark.uncommitted, 0);
        assert_eq!(load_in(&s.dir(), &s.repo).unwrap().as_ref(), Some(&mark));

        let same = distance(&s.repo, &mark, true);
        assert_eq!(same.identical, Some(true));
        assert_eq!(same.summary(), "exactly as marked");

        fs::write(s.repo.join("a.txt"), "two\n").unwrap();
        fs::write(s.repo.join("new.txt"), "n\n").unwrap();
        let moved = distance(&s.repo, &mark, true);
        assert_eq!(moved.identical, Some(false));
        assert_eq!(moved.files_differ, Some(2));
        assert_eq!(moved.commits_since, Some(0));

        run(&s.repo, &["add", "."]);
        run(&s.repo, &["commit", "-q", "-m", "two"]);
        let committed = distance(&s.repo, &mark, false);
        assert!(committed.head_moved);
        assert_eq!(committed.commits_since, Some(1));
        assert_eq!(committed.summary(), "1 commit, 2 files differ");
        assert_eq!(
            files_differing(&s.repo, mark.head.as_deref().unwrap()).unwrap(),
            ["a.txt", "new.txt"]
        );

        run(
            &s.repo,
            &["reset", "-q", "--hard", mark.head.as_deref().unwrap()],
        );
        run(&s.repo, &["clean", "-qfd"]);
        assert_eq!(distance(&s.repo, &mark, true).identical, Some(true));

        assert!(forget_in(&s.dir(), &s.repo).unwrap());
        assert!(!forget_in(&s.dir(), &s.repo).unwrap());
        assert_eq!(load_in(&s.dir(), &s.repo).unwrap(), None);
    }

    #[test]
    fn untracked_content_changes_the_fingerprint_without_being_stored() {
        let Some(s) = scratch() else { return };
        fs::write(s.repo.join("notes.txt"), "client acme secret\n").unwrap();
        let mark = s.mark();
        assert_eq!(mark.uncommitted, 1);
        let stored = fs::read_to_string(mark_path(&s.dir(), &s.repo)).unwrap();
        assert!(!stored.contains("notes.txt"));
        assert!(!stored.contains("acme"));
        assert!(!stored.contains("main"), "no branch name: {stored}");

        fs::write(s.repo.join("notes.txt"), "changed\n").unwrap();
        assert_eq!(distance(&s.repo, &mark, true).identical, Some(false));
    }

    #[test]
    fn a_dropped_commit_is_reported_rather_than_hidden() {
        let Some(s) = scratch() else { return };
        fs::write(s.repo.join("b.txt"), "b\n").unwrap();
        run(&s.repo, &["add", "."]);
        run(&s.repo, &["commit", "-q", "-m", "two"]);
        let mark = s.mark();
        run(&s.repo, &["reset", "-q", "--hard", "HEAD~1"]);
        let distance = distance(&s.repo, &mark, false);
        assert_eq!(distance.commits_dropped, Some(1));
        assert_eq!(distance.summary(), "1 commit dropped, 1 file differs");
    }

    #[test]
    fn a_tampered_record_is_refused() {
        let Some(s) = scratch() else { return };
        s.mark();
        let path = mark_path(&s.dir(), &s.repo);
        let text = fs::read_to_string(&path)
            .unwrap()
            .replace("\"head\": \"", "\"head\": \"; rm -rf ~ #");
        fs::write(&path, text).unwrap();
        assert!(load_in(&s.dir(), &s.repo).is_err());
    }

    #[test]
    fn a_missing_marked_commit_is_unknown_not_zero() {
        let mark = Mark {
            schema_version: 1,
            marked_at: 0,
            head: Some("0".repeat(40)),
            uncommitted: 0,
            fingerprint: None,
        };
        let Some(s) = scratch() else { return };
        let distance = distance(&s.repo, &mark, true);
        assert!(distance.mark_missing);
        assert_eq!(distance.files_differ, None);
        assert_eq!(
            distance.summary(),
            "the marked commit is no longer in this repository"
        );
    }
}
