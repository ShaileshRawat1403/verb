//! Repository states in which the next Git command is riskier than usual.
//!
//! Backlog C5. The TUI vision lists "a Git operation gets risky" as a band trigger, and the rule is
//! the same as everywhere else in Verb: the band appears from an observed fact, never from a guess.
//! Verb cannot see a command before it runs (it deliberately records no command text), so it does
//! not pretend to warn about `git reset --hard` in advance. What it *can* observe is the state a
//! repository is in, and several states are exactly the ones where people lose work:
//!
//! * an operation left half-way: rebase, `git am`, merge, cherry-pick, revert, bisect;
//! * paths with unresolved conflicts;
//! * a detached HEAD, where new commits belong to no branch;
//! * a branch whose upstream has diverged or been deleted (as of the last fetch -- Verb never
//!   fetches, so it says whose knowledge this is).
//!
//! Every reading is taken now and says so. Wording follows C2: counts, never file names or branch
//! names, so a warning can travel into an assistant prompt without disclosing a client or a feature.
//! Each warning carries the *safe* way out, as text; Verb runs nothing.

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Operation {
    Rebase,
    Am,
    Merge,
    CherryPick,
    Revert,
    Bisect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Upstream {
    /// No upstream is configured, which is a normal state and not a warning.
    None,
    Tracking {
        ahead: u64,
        behind: u64,
    },
    /// Configured, but the remote branch no longer exists as of the last fetch.
    Gone,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepoState {
    pub operation: Option<Operation>,
    pub unmerged: usize,
    pub detached: bool,
    pub upstream: Upstream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Level {
    /// Something is unfinished or will reject a plain command.
    Caution,
    /// Work can be lost by the obvious next step.
    Risk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Warning {
    /// Stable identifier for programs: `operation-in-progress`, `unmerged-paths`, …
    pub code: &'static str,
    pub level: Level,
    /// The observed fact, in plain words.
    pub fact: String,
    /// The way out that keeps everything, as a command the user may choose to run.
    pub safe_next: String,
}

impl Warning {
    pub(crate) fn to_json(&self) -> String {
        format!(
            "{{\"code\":\"{}\",\"level\":\"{}\",\"fact\":\"{}\",\"safeNext\":\"{}\"}}",
            self.code,
            match self.level {
                Level::Caution => "caution",
                Level::Risk => "risk",
            },
            crate::json_escape(&self.fact),
            crate::json_escape(&self.safe_next)
        )
    }
}

/// `None` outside a Git repository, or when Git itself could not run.
pub(crate) fn observe(project: &Path) -> Option<RepoState> {
    let git_dir = git(project, &["rev-parse", "--absolute-git-dir"]).map(PathBuf::from)?;
    let operation = operation_in(&git_dir);
    let unmerged = git(project, &["status", "--porcelain=v1"])
        .map(|status| count_unmerged(&status))
        .unwrap_or(0);
    let head = git(project, &["symbolic-ref", "-q", "HEAD"]);
    let detached = head.is_none();
    let upstream = match head.as_deref() {
        Some(reference) => git(
            project,
            &[
                "for-each-ref",
                "--format=%(upstream)%09%(upstream:track,nobracket)",
                reference,
            ],
        )
        .map(|line| parse_upstream(&line))
        .unwrap_or(Upstream::Unknown),
        None => Upstream::None,
    };
    Some(RepoState {
        operation,
        unmerged,
        detached,
        upstream,
    })
}

/// The markers Git itself leaves in its directory while an operation is unfinished.
fn operation_in(git_dir: &Path) -> Option<Operation> {
    let exists = |name: &str| git_dir.join(name).exists();
    if exists("rebase-merge") {
        Some(Operation::Rebase)
    } else if exists("rebase-apply") {
        // `git am` and the old apply-based rebase share this directory; `applying` marks `am`.
        if exists("rebase-apply/applying") {
            Some(Operation::Am)
        } else {
            Some(Operation::Rebase)
        }
    } else if exists("MERGE_HEAD") {
        Some(Operation::Merge)
    } else if exists("CHERRY_PICK_HEAD") {
        Some(Operation::CherryPick)
    } else if exists("REVERT_HEAD") {
        Some(Operation::Revert)
    } else if exists("BISECT_LOG") {
        Some(Operation::Bisect)
    } else {
        None
    }
}

/// Porcelain v1 unmerged states: `DD AU UD UA DU AA UU`.
fn count_unmerged(status: &str) -> usize {
    status
        .lines()
        .filter(|line| {
            matches!(
                line.get(..2),
                Some("DD" | "AU" | "UD" | "UA" | "DU" | "AA" | "UU")
            )
        })
        .count()
}

/// `refs/remotes/origin/main\tahead 2, behind 1`, `…\tgone`, `\t` (no upstream).
fn parse_upstream(line: &str) -> Upstream {
    let (upstream, track) = line.split_once('\t').unwrap_or((line, ""));
    if upstream.trim().is_empty() {
        return Upstream::None;
    }
    let track = track.trim();
    if track == "gone" {
        return Upstream::Gone;
    }
    let mut ahead = 0;
    let mut behind = 0;
    for part in track.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let Some((word, count)) = part.split_once(' ') else {
            return Upstream::Unknown;
        };
        let Ok(count) = count.parse() else {
            return Upstream::Unknown;
        };
        match word {
            "ahead" => ahead = count,
            "behind" => behind = count,
            _ => return Upstream::Unknown,
        }
    }
    Upstream::Tracking { ahead, behind }
}

impl RepoState {
    /// The warnings this state justifies, most urgent first. A healthy repository yields none.
    pub(crate) fn warnings(&self) -> Vec<Warning> {
        let mut warnings = Vec::new();
        if let Some(operation) = self.operation {
            let (fact, safe_next) = match operation {
                Operation::Rebase => (
                    "A rebase is in progress.",
                    "git rebase --continue once resolved, or git rebase --abort to return to where it started",
                ),
                Operation::Am => (
                    "A git am (patch apply) is in progress.",
                    "git am --continue once resolved, or git am --abort to return to where it started",
                ),
                Operation::Merge => (
                    "A merge is in progress.",
                    "git commit once resolved, or git merge --abort to return to where it started",
                ),
                Operation::CherryPick => (
                    "A cherry-pick is in progress.",
                    "git cherry-pick --continue once resolved, or git cherry-pick --abort",
                ),
                Operation::Revert => (
                    "A revert is in progress.",
                    "git revert --continue once resolved, or git revert --abort",
                ),
                Operation::Bisect => (
                    "A bisect is in progress; HEAD is wherever the search left it.",
                    "git bisect reset returns to the commit you started from",
                ),
            };
            warnings.push(Warning {
                code: "operation-in-progress",
                level: Level::Caution,
                fact: fact.to_owned(),
                safe_next: safe_next.to_owned(),
            });
        }
        if self.unmerged > 0 {
            warnings.push(Warning {
                code: "unmerged-paths",
                level: Level::Caution,
                fact: format!(
                    "{} {} unresolved conflicts.",
                    self.unmerged,
                    if self.unmerged == 1 {
                        "file has"
                    } else {
                        "files have"
                    }
                ),
                safe_next: "git status lists them; resolve each, then git add it".to_owned(),
            });
        }
        // During a rebase or bisect a detached HEAD is expected and already explained above.
        if self.detached && self.operation.is_none() {
            warnings.push(Warning {
                code: "detached-head",
                level: Level::Risk,
                fact: "HEAD is detached: new commits here belong to no branch.".to_owned(),
                safe_next: "git switch -c NEW-BRANCH keeps any commits made here".to_owned(),
            });
        }
        match self.upstream {
            Upstream::Tracking { ahead, behind } if ahead > 0 && behind > 0 => {
                warnings.push(Warning {
                    code: "diverged",
                    level: Level::Risk,
                    fact: format!(
                        "This branch and its upstream have diverged as of the last fetch: {ahead} ahead, {behind} behind. A plain push will be rejected, and a force push would discard {behind} upstream {}.",
                        if behind == 1 { "commit" } else { "commits" }
                    ),
                    safe_next: "git pull --rebase (or merge) first; if you must overwrite, git push --force-with-lease".to_owned(),
                });
            }
            Upstream::Gone => warnings.push(Warning {
                code: "upstream-gone",
                level: Level::Caution,
                fact: "This branch's upstream no longer exists as of the last fetch.".to_owned(),
                safe_next: "git push -u origin HEAD recreates it, if that is what you want"
                    .to_owned(),
            }),
            _ => {}
        }
        warnings
    }
}

fn git(project: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(project)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn run(dir: &Path, args: &[&str]) {
        let status = try_run(dir, args);
        assert!(status.status.success(), "git {args:?}: {status:?}");
    }

    fn try_run(dir: &Path, args: &[&str]) -> std::process::Output {
        Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap()
    }

    fn repo() -> Option<PathBuf> {
        Command::new("git").arg("--version").output().ok()?;
        let root = std::env::temp_dir().join(format!("verb-gitstate-{}", crate::new_id()));
        fs::create_dir_all(&root).unwrap();
        run(&root, &["init", "-q", "-b", "main"]);
        fs::write(root.join("a.txt"), "one\n").unwrap();
        run(&root, &["add", "."]);
        run(&root, &["commit", "-q", "-m", "one"]);
        Some(root)
    }

    #[test]
    fn upstream_tracking_is_parsed_from_for_each_ref() {
        assert_eq!(parse_upstream("\t"), Upstream::None);
        assert_eq!(parse_upstream(""), Upstream::None);
        assert_eq!(
            parse_upstream("refs/remotes/origin/main\tahead 2, behind 1"),
            Upstream::Tracking {
                ahead: 2,
                behind: 1
            }
        );
        assert_eq!(
            parse_upstream("refs/remotes/origin/main\t"),
            Upstream::Tracking {
                ahead: 0,
                behind: 0
            }
        );
        assert_eq!(
            parse_upstream("refs/remotes/origin/main\tgone"),
            Upstream::Gone
        );
        assert_eq!(
            parse_upstream("refs/remotes/origin/main\tsideways 3"),
            Upstream::Unknown
        );
    }

    #[test]
    fn only_unmerged_porcelain_codes_count_as_conflicts() {
        let status = "UU a.txt\nAA b.txt\n M c.txt\nMM d.txt\n?? e.txt\nDU f.txt\n";
        assert_eq!(count_unmerged(status), 3);
    }

    #[test]
    fn a_healthy_repository_has_no_warnings() {
        let state = RepoState {
            operation: None,
            unmerged: 0,
            detached: false,
            upstream: Upstream::Tracking {
                ahead: 3,
                behind: 0,
            },
        };
        assert!(state.warnings().is_empty());
    }

    #[test]
    fn warnings_name_counts_never_branches_or_files() {
        let state = RepoState {
            operation: Some(Operation::Rebase),
            unmerged: 2,
            detached: true,
            upstream: Upstream::Tracking {
                ahead: 1,
                behind: 4,
            },
        };
        let warnings = state.warnings();
        let codes: Vec<&str> = warnings.iter().map(|w| w.code).collect();
        // Detached HEAD is expected mid-rebase and is not repeated.
        assert_eq!(
            codes,
            ["operation-in-progress", "unmerged-paths", "diverged"]
        );
        assert!(warnings[2].fact.contains("1 ahead, 4 behind"));
        assert!(warnings[2].safe_next.contains("--force-with-lease"));
    }

    #[test]
    fn a_real_detached_head_and_merge_conflict_are_observed() {
        let Some(root) = repo() else { return };
        let state = observe(&root).unwrap();
        assert_eq!(state.operation, None);
        assert!(!state.detached);
        assert_eq!(state.upstream, Upstream::None);

        run(&root, &["switch", "-q", "-c", "side"]);
        fs::write(root.join("a.txt"), "side\n").unwrap();
        run(&root, &["commit", "-q", "-am", "side"]);
        run(&root, &["switch", "-q", "main"]);
        fs::write(root.join("a.txt"), "main\n").unwrap();
        run(&root, &["commit", "-q", "-am", "main"]);
        // Expected to fail with a conflict; the state it leaves is what is under test.
        assert!(!try_run(&root, &["merge", "-q", "side"]).status.success());
        let state = observe(&root).unwrap();
        assert_eq!(state.operation, Some(Operation::Merge));
        assert_eq!(state.unmerged, 1);

        run(&root, &["merge", "--abort"]);
        run(&root, &["switch", "-q", "--detach", "HEAD~1"]);
        let state = observe(&root).unwrap();
        assert!(state.detached);
        assert_eq!(state.warnings()[0].code, "detached-head");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn outside_a_repository_there_is_no_state_rather_than_a_clean_one() {
        let root = std::env::temp_dir().join(format!("verb-gitstate-none-{}", crate::new_id()));
        fs::create_dir_all(&root).unwrap();
        // GIT_CEILING_DIRECTORIES is not needed: temp_dir is not inside a repository on CI.
        if git(&root, &["rev-parse", "--absolute-git-dir"]).is_none() {
            assert_eq!(observe(&root), None);
        }
        fs::remove_dir_all(root).unwrap();
    }
}
