//! Read-only diffs of the files Git reports as changed, for reviewing work in the browser.
//!
//! Only paths that `git status` lists can be diffed, so an ignored file (`.env`, build output) is
//! never read this way, and the path always reaches Git after `--`, never as an option. Untracked
//! files are shown as entirely added, through the same checks as the file preview (inside the
//! project, no binaries, bounded size). External diff drivers and textconv filters are switched off:
//! the diff is Git's own text, nothing a repository's configuration can run.

use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// Larger diffs are refused rather than truncated mid-hunk.
const MAX_DIFF_BYTES: usize = 400_000;
/// More lines than this are cut, and the reply says so.
const MAX_LINES: usize = 3_000;

#[derive(Serialize, Debug, PartialEq)]
pub(crate) struct DiffLine {
    /// `add`, `del` or `ctx`.
    pub kind: &'static str,
    pub text: String,
    pub old: Option<u32>,
    pub new: Option<u32>,
}

#[derive(Serialize, Debug, PartialEq)]
pub(crate) struct Hunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Serialize, Debug)]
pub(crate) struct FileDiff {
    pub path: String,
    pub code: String,
    pub hunks: Vec<Hunk>,
    pub added: u32,
    pub removed: u32,
    /// Why there are no (or not all) lines: binary, too large, cut short.
    pub reason: Option<String>,
}

/// Parses `git diff` output for one file. Returns hunks, added and removed counts, and whether Git
/// reported the file as binary.
pub(crate) fn parse_unified(text: &str) -> (Vec<Hunk>, u32, u32, bool) {
    let mut hunks: Vec<Hunk> = Vec::new();
    let (mut added, mut removed) = (0, 0);
    let (mut old, mut new) = (0u32, 0u32);
    let mut binary = false;
    for line in text.lines() {
        if line.starts_with("Binary files ") || line == "GIT binary patch" {
            binary = true;
        } else if let Some(rest) = line.strip_prefix("@@ ") {
            let (o, n) = hunk_starts(rest);
            old = o;
            new = n;
            hunks.push(Hunk {
                header: line.to_owned(),
                lines: Vec::new(),
            });
        } else if let Some(hunk) = hunks.last_mut() {
            let (kind, body) = match line.chars().next() {
                Some('+') => ("add", &line[1..]),
                Some('-') => ("del", &line[1..]),
                Some(' ') => ("ctx", &line[1..]),
                Some('\\') => continue, // "\ No newline at end of file"
                _ => ("ctx", line),
            };
            let (o, n) = match kind {
                "add" => {
                    added += 1;
                    new += 1;
                    (None, Some(new - 1))
                }
                "del" => {
                    removed += 1;
                    old += 1;
                    (Some(old - 1), None)
                }
                _ => {
                    old += 1;
                    new += 1;
                    (Some(old - 1), Some(new - 1))
                }
            };
            hunk.lines.push(DiffLine {
                kind,
                text: body.to_owned(),
                old: o,
                new: n,
            });
        }
    }
    (hunks, added, removed, binary)
}

/// `-12,7 +12,9 @@ fn x` → (12, 12).
fn hunk_starts(rest: &str) -> (u32, u32) {
    let mut parts = rest.split_whitespace();
    let start = |part: Option<&str>, sign: char| {
        part.and_then(|p| p.strip_prefix(sign))
            .and_then(|p| p.split(',').next())
            .and_then(|p| p.parse().ok())
            .unwrap_or(0)
    };
    (start(parts.next(), '-'), start(parts.next(), '+'))
}

fn git(project: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(project)
        .args(["-c", "core.quotepath=off"])
        .args(args)
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

fn has_head(project: &Path) -> bool {
    git(project, &["rev-parse", "--verify", "-q", "HEAD"]).is_ok()
}

/// `+added −removed` for every changed file, keyed by its status path. Binary files have no counts.
pub(crate) fn numstat(
    project: &Path,
    changes: &[crate::specs::Change],
) -> HashMap<String, (u32, u32)> {
    let mut counts = HashMap::new();
    if has_head(project) {
        if let Ok(out) = git(
            project,
            &[
                "diff",
                "--numstat",
                "--no-ext-diff",
                "--no-textconv",
                "HEAD",
            ],
        ) {
            for line in out.lines() {
                let mut f = line.splitn(3, '\t');
                let (Some(a), Some(r), Some(path)) = (f.next(), f.next(), f.next()) else {
                    continue;
                };
                if let (Ok(a), Ok(r)) = (a.parse(), r.parse()) {
                    counts.insert(path.to_owned(), (a, r));
                }
            }
        }
    }
    for change in changes {
        if (change.code == "??" || !has_head(project)) && !counts.contains_key(&change.path) {
            if let Ok(preview) = crate::hub::preview(project, &change.path) {
                if let Some(text) = preview.text {
                    counts.insert(change.path.clone(), (text.lines().count() as u32, 0));
                }
            }
        }
    }
    counts
}

/// The diff of one changed file against the last commit (staged and unstaged together).
pub(crate) fn file_diff(project: &Path, path: &str) -> Result<FileDiff, String> {
    let summary = crate::specs::git_summary(project)?;
    let change = summary
        .changes
        .iter()
        .find(|c| c.path == path)
        .ok_or("Verb only shows diffs of files Git lists as changed")?;
    let mut diff = FileDiff {
        path: path.to_owned(),
        code: change.code.clone(),
        hunks: Vec::new(),
        added: 0,
        removed: 0,
        reason: None,
    };

    if change.code == "??" || !has_head(project) {
        // Untracked (or no commit yet): the whole file is new.
        let preview = crate::hub::preview(project, path)?;
        match preview.text {
            Some(text) => {
                let lines: Vec<DiffLine> = text
                    .lines()
                    .take(MAX_LINES)
                    .enumerate()
                    .map(|(i, line)| DiffLine {
                        kind: "add",
                        text: line.to_owned(),
                        old: None,
                        new: Some(i as u32 + 1),
                    })
                    .collect();
                let total = text.lines().count();
                if total > MAX_LINES {
                    diff.reason = Some(format!("showing the first {MAX_LINES} of {total} lines"));
                }
                diff.added = total as u32;
                diff.hunks.push(Hunk {
                    header: format!("@@ -0,0 +1,{total} @@ new file"),
                    lines,
                });
            }
            None => diff.reason = preview.reason,
        }
        return Ok(diff);
    }

    // A rename is listed as "old -> new"; Git needs both paths to show it as one change.
    let paths: Vec<&str> = match path.split_once(" -> ") {
        Some((from, to)) => vec![from, to],
        None => vec![path],
    };
    let mut args = vec![
        "diff",
        "--no-color",
        "--no-ext-diff",
        "--no-textconv",
        "-M",
        "-U3",
        "HEAD",
        "--",
    ];
    args.extend(paths);
    let text = git(project, &args)?;
    if text.len() > MAX_DIFF_BYTES {
        diff.reason = Some("this diff is too large to show here (over 400 KB)".to_owned());
        return Ok(diff);
    }
    let (mut hunks, added, removed, binary) = parse_unified(&text);
    diff.added = added;
    diff.removed = removed;
    if binary {
        diff.reason = Some("binary file".to_owned());
    }
    let total: usize = hunks.iter().map(|h| h.lines.len()).sum();
    if total > MAX_LINES {
        let mut budget = MAX_LINES;
        hunks.retain_mut(|h| {
            if budget == 0 {
                return false;
            }
            h.lines.truncate(budget);
            budget -= h.lines.len();
            true
        });
        diff.reason = Some(format!("showing the first {MAX_LINES} of {total} lines"));
    }
    diff.hunks = hunks;
    if diff.hunks.is_empty() && diff.reason.is_none() {
        diff.reason = Some("no line changes (a mode or permission change)".to_owned());
    }
    Ok(diff)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn a_unified_diff_parses_with_line_numbers() {
        let text = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -2,3 +2,4 @@ fn x\n keep\n-old\n+new\n+more\n tail\n\\ No newline at end of file\n";
        let (hunks, added, removed, binary) = parse_unified(text);
        assert_eq!((added, removed, binary), (2, 1, false));
        assert_eq!(hunks.len(), 1);
        let l = &hunks[0].lines;
        assert_eq!((l[0].kind, l[0].old, l[0].new), ("ctx", Some(2), Some(2)));
        assert_eq!((l[1].kind, l[1].old, l[1].new), ("del", Some(3), None));
        assert_eq!((l[2].kind, l[2].old, l[2].new), ("add", None, Some(3)));
        assert_eq!((l[4].kind, l[4].old, l[4].new), ("ctx", Some(4), Some(5)));
        assert_eq!(l.len(), 5, "the no-newline marker is not a line");
    }

    #[test]
    fn binary_files_are_reported_not_parsed() {
        let (hunks, _, _, binary) = parse_unified("Binary files a/i.png and b/i.png differ\n");
        assert!(binary);
        assert!(hunks.is_empty());
    }

    fn repo() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "verb-diff-{}-{}",
            std::process::id(),
            rand_suffix()
        ));
        fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| {
            assert!(Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .output()
                .unwrap()
                .status
                .success());
        };
        run(&["init", "-q"]);
        run(&["config", "user.email", "t@example.com"]);
        run(&["config", "user.name", "T"]);
        fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
        fs::write(dir.join(".gitignore"), ".env\n").unwrap();
        run(&["add", "-A"]);
        run(&["commit", "-qm", "init"]);
        dir
    }

    fn rand_suffix() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }

    #[test]
    fn changed_and_new_files_diff_but_ignored_and_unchanged_ones_never_do() {
        let dir = repo();
        fs::write(dir.join("a.txt"), "one\n2\nthree\n").unwrap();
        fs::write(dir.join("new.txt"), "hello\nworld\n").unwrap();
        fs::write(dir.join(".env"), "SECRET=x\n").unwrap();

        let changed = file_diff(&dir, "a.txt").unwrap();
        assert_eq!((changed.added, changed.removed), (1, 1));
        assert!(changed.hunks[0]
            .lines
            .iter()
            .any(|l| l.kind == "del" && l.text == "two"));

        let new = file_diff(&dir, "new.txt").unwrap();
        assert_eq!((new.code.as_str(), new.added), ("??", 2));

        assert!(
            file_diff(&dir, ".env").is_err(),
            "ignored files are not listed, so never read"
        );
        assert!(
            file_diff(&dir, ".gitignore").is_err(),
            "unchanged files have nothing to show"
        );
        assert!(
            file_diff(&dir, "--output=/tmp/x").is_err(),
            "never reaches git as an option"
        );

        let summary = crate::specs::git_summary(&dir).unwrap();
        let counts = numstat(&dir, &summary.changes);
        assert_eq!(counts.get("a.txt"), Some(&(1, 1)));
        assert_eq!(counts.get("new.txt"), Some(&(2, 0)));
        let _ = fs::remove_dir_all(dir);
    }
}
