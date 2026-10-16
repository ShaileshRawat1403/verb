//! Read-only Git presentation facts. Failed reads stay unknown, never an observed clean tree.
use std::path::{Path, PathBuf};
use std::process::Stdio;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum GitStatus {
    Branch,
    Unborn,
    Detached,
    NotRepository,
    Unavailable,
}
impl GitStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Branch => "branch",
            Self::Unborn => "unborn",
            Self::Detached => "detached",
            Self::NotRepository => "not_repository",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct GitSnapshot {
    pub root: Option<PathBuf>,
    pub branch: Option<String>,
    pub changed_files: usize,
    pub status: GitStatus,
    pub changes_known: bool,
}

impl GitSnapshot {
    pub(crate) fn branch_label(&self) -> &str {
        self.branch.as_deref().unwrap_or(match self.status {
            GitStatus::Detached => "Detached HEAD",
            GitStatus::NotRepository => "Not a Git repository",
            _ => "Git status unavailable",
        })
    }
    pub(crate) fn change_summary(&self) -> String {
        if self.changes_known {
            format!("{} changed", self.changed_files)
        } else {
            "Changes unavailable".to_owned()
        }
    }
}

enum ReadFailure {
    NotRepository,
    Unavailable,
}
fn read(args: &[&str], directory: &Path) -> Result<String, ReadFailure> {
    let output = crate::exec::git(directory)
        .ok_or(ReadFailure::Unavailable)?
        .env("LC_ALL", "C")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|_| ReadFailure::Unavailable)?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else if String::from_utf8_lossy(&output.stderr).contains("not a git repository") {
        Err(ReadFailure::NotRepository)
    } else {
        Err(ReadFailure::Unavailable)
    }
}

pub(crate) fn output(args: &[&str], directory: &Path) -> Option<String> {
    read(args, directory).ok()
}

pub(crate) fn snapshot(project: &Path) -> GitSnapshot {
    if crate::isolation::external() {
        // Enter the boundary once. The trusted helper runs these same guarded reads
        // under the project UID, rather than creating a namespace for each Git call.
        return isolated_snapshot(project).unwrap_or(GitSnapshot {
            root: None,
            branch: None,
            changed_files: 0,
            status: GitStatus::Unavailable,
            changes_known: false,
        });
    }
    local_snapshot(project)
}

fn isolated_snapshot(project: &Path) -> Option<GitSnapshot> {
    let output = crate::isolation::command(Path::new("/usr/bin/timeout"), project, "")
        .ok()?
        .args([
            "--kill-after=1s",
            "10s",
            "/usr/local/bin/verb-recovery",
            "__git-snapshot",
        ])
        .stdin(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() || output.stdout.len() > 65536 {
        return None;
    }
    let mut facts: GitSnapshot = serde_json::from_slice(&output.stdout).ok()?;
    // Guest and host paths differ. The helper confirms that the registered
    // workspace itself is the repository root before reporting a root at all.
    facts.root = facts.root.map(|_| project.to_path_buf());
    Some(facts)
}

/// Internal helper, already inside the project boundary. Never re-enter it or
/// discover the project root through the normal CLI startup path.
pub(crate) fn helper(args: &[String]) -> Result<(), String> {
    if !args.is_empty() || crate::isolation::profile()?.is_some() {
        return Err("invalid Git snapshot helper invocation".into());
    }
    let directory = std::env::current_dir().map_err(|error| error.to_string())?;
    let mut facts = local_snapshot(&directory);
    if facts.root.as_deref() != Some(directory.as_path()) {
        facts.root = None;
    }
    println!(
        "{}",
        serde_json::to_string(&facts).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn local_snapshot(project: &Path) -> GitSnapshot {
    let root = output(&["rev-parse", "--show-toplevel"], project)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    let mut facts = GitSnapshot {
        root,
        branch: None,
        changed_files: 0,
        status: GitStatus::Unavailable,
        changes_known: false,
    };
    match read(&["status", "--porcelain=v2", "--branch"], project) {
        Ok(raw) if facts.root.is_some() => {
            let head = raw
                .lines()
                .find_map(|line| line.strip_prefix("# branch.head "));
            facts.status = match head {
                Some("(detached)") => GitStatus::Detached,
                Some(name) => {
                    facts.branch = Some(name.to_owned());
                    if raw.lines().any(|line| line == "# branch.oid (initial)") {
                        GitStatus::Unborn
                    } else {
                        GitStatus::Branch
                    }
                }
                None => GitStatus::Unavailable,
            };
            facts.changes_known = facts.status != GitStatus::Unavailable;
            facts.changed_files = raw
                .lines()
                .filter(|line| !line.starts_with('#') && !line.is_empty())
                .count();
        }
        Err(ReadFailure::NotRepository) if facts.root.is_none() => {
            // A broken/unreadable repository must not be called a plain folder.
            let marker = project
                .ancestors()
                .any(|path| std::fs::symlink_metadata(path.join(".git")).is_ok());
            if !marker {
                facts.status = GitStatus::NotRepository;
            }
        }
        _ => {}
    }
    facts
}

#[cfg(test)]
mod tests {
    use super::*;
    fn folder() -> PathBuf {
        let path = std::env::temp_dir().join(format!("verb-gitview-{}", crate::new_id()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }
    fn git(path: &Path, args: &[&str]) {
        let result = std::process::Command::new("git")
            .current_dir(path)
            .args(args)
            .env("GIT_AUTHOR_NAME", "Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
            .env("GIT_COMMITTER_NAME", "Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(result.status.success());
    }
    #[test]
    fn new_branch_and_detached_checkout_have_observed_distinct_states() {
        let path = folder();
        git(&path, &["init", "-q", "-b", "main"]);
        let fresh = snapshot(&path);
        assert_eq!(fresh.status, GitStatus::Unborn);
        assert_eq!(fresh.branch.as_deref(), Some("main"));
        assert!(fresh.changes_known);
        std::fs::write(path.join("sample.txt"), "fixture").unwrap();
        assert_eq!(snapshot(&path).changed_files, 1);
        git(&path, &["add", "."]);
        git(&path, &["commit", "-qm", "fixture"]);
        assert_eq!(snapshot(&path).status, GitStatus::Branch);
        git(&path, &["checkout", "--detach", "-q"]);
        let detached = snapshot(&path);
        assert_eq!(detached.status, GitStatus::Detached);
        assert!(detached.branch.is_none());
        assert!(detached.changes_known);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn non_git_folder_has_no_observed_change_count() {
        let path = folder();
        let facts = snapshot(&path);
        assert_eq!(facts.status, GitStatus::NotRepository);
        assert!(!facts.changes_known);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn unreadable_or_invalid_checkout_does_not_become_clean_or_non_git() {
        let path = folder();
        git(&path, &["init", "-q", "-b", "main"]);
        std::fs::write(path.join(".git/HEAD"), "invalid fixture").unwrap();
        let facts = snapshot(&path);
        assert_eq!(facts.status, GitStatus::Unavailable);
        assert!(!facts.changes_known);
        let missing = snapshot(&path.join("missing"));
        assert_eq!(missing.status, GitStatus::Unavailable);
        assert!(!missing.changes_known);
        std::fs::remove_dir_all(path).unwrap();
    }
}
