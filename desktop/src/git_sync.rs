//! User-reviewed Git transfers. No automatic network access, history rewriting or merge policy.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

static TRANSFER: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Review {
    pub branch: String,
    pub local_oid: String,
    pub remote: String,
    pub server: String,
    pub remote_ref: String,
    pub tracking_ref: String,
    pub remote_oid: String,
    pub destination_version: String,
    pub ahead: u32,
    pub behind: u32,
    pub outgoing: Vec<String>,
    pub incoming: Vec<String>,
    pub blocked: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    pub action: String,
    pub review: Review,
}

fn read_before(project: &Path, args: &[&str], deadline: Instant) -> Result<String, String> {
    if Instant::now() >= deadline {
        return Err("Git review took too long. Close and reopen it before trying again.".into());
    }
    let mut command = crate::exec::git(project)
        .ok_or("Git could not be prepared. Check this project's access.")?;
    command.args(args);
    let output = crate::exec::read_output_before(&mut command, deadline).map_err(|_| {
        "Git did not finish reading this project. Refresh before trying again.".to_owned()
    })?;
    if !output.status.success() {
        return Err("Git could not read this project's branch or linked remote. Check its Git setup in a terminal.".into());
    }
    String::from_utf8(output.stdout)
        .map(|s| s.trim_end().to_owned())
        .map_err(|_| "Git returned an unreadable branch configuration.".into())
}

fn server_label(url: &str) -> Result<String, String> {
    if url.contains("::") && !url.contains("://") {
        return Err(
            "This remote uses a custom transport or address. Review and transfer it in a terminal."
                .into(),
        );
    }
    let host = if let Some((scheme, rest)) = url.split_once("://") {
        if scheme == "file" {
            return Ok("local Git repository".into());
        }
        if !matches!(scheme, "https" | "http" | "ssh" | "git") {
            return Err(
                "This remote uses a custom transport. Review and transfer it in a terminal.".into(),
            );
        }
        rest.split(['/', '?', '#'])
            .next()
            .unwrap_or("")
            .rsplit('@')
            .next()
            .unwrap_or("")
    } else if let Some((authority, _)) = url.split_once(':') {
        authority.rsplit('@').next().unwrap_or("")
    } else {
        return Ok("local Git repository".into());
    };
    if host.is_empty()
        || !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-:[]".contains(&b))
    {
        return Err("The remote server address needs review in a terminal.".into());
    }
    Ok(host.into())
}

pub(crate) fn review(project: &Path) -> Result<Review, String> {
    let deadline = Instant::now() + Duration::from_secs(30);
    let read = |project: &Path, args: &[&str]| read_before(project, args, deadline);
    let branch = read(project, &["symbolic-ref", "--quiet", "HEAD"]).map_err(|_| {
        "Could not read an active branch. Check Git access and choose a branch in a terminal before transferring work."
            .to_owned()
    })?;
    let local_oid = read(project, &["rev-parse", "--verify", "HEAD^{commit}"])
        .map_err(|_| "Could not read the current commit. Check Git access, or save your first commit if this is a new repository.".to_owned())?;
    let row = read(
        project,
        &[
            "for-each-ref",
            "--format=%(refname)%00%(upstream)%00%(upstream:remotename)%00%(upstream:remoteref)",
            &branch,
        ],
    )?;
    let fields: Vec<_> = row.lines().next().unwrap_or("").split('\0').collect();
    if fields.len() != 4
        || fields[0] != branch
        || !fields[1].starts_with("refs/remotes/")
        || !fields[3].starts_with("refs/heads/")
        || fields[2].is_empty()
        || !fields[2]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        || fields[2].starts_with('-')
        || fields[2] == "."
    {
        return Err("This branch needs a linked remote branch before guided transfers can run. Set its upstream in a terminal, then reopen this review.".into());
    }
    let remote = fields[2];
    let tracking = read(
        project,
        &["for-each-ref", "--format=%(refname)%00%(symref)", fields[1]],
    )?;
    if tracking.lines().next() != Some(&format!("{}\0", fields[1])) {
        return Err("The linked tracking branch is missing or indirect. Review its upstream setup in a terminal before transferring work.".into());
    }
    // A separate push URL can send work somewhere other than the place just reviewed.
    // Do not display URLs: they can contain credentials.
    let fetch_url = read(project, &["remote", "get-url", "--all", remote])?;
    let push_url = read(project, &["remote", "get-url", "--push", "--all", remote])?;
    if fetch_url.lines().count() != 1 || fetch_url != push_url {
        return Err("This remote has multiple destinations or a separate upload destination. Use a terminal to review that setup before transferring work.".into());
    }
    let destination_version = format!("{:x}", Sha256::digest(fetch_url.as_bytes()));
    let server = server_label(&fetch_url)?;
    let remote_oid = read(
        project,
        &[
            "rev-parse",
            "--verify",
            &format!("{}^{{commit}}", fields[1]),
        ],
    )?;
    let counts = read(
        project,
        &[
            "rev-list",
            "--left-right",
            "--count",
            &format!("{local_oid}...{remote_oid}"),
        ],
    )?;
    let mut counts = counts.split_whitespace();
    let ahead = counts
        .next()
        .and_then(|n| n.parse::<u32>().ok())
        .ok_or("Git comparison unavailable.")?;
    let behind = counts
        .next()
        .and_then(|n| n.parse::<u32>().ok())
        .ok_or("Git comparison unavailable.")?;
    let commits = |range: &str| -> Result<Vec<String>, String> {
        Ok(read(
            project,
            &[
                "log",
                "--no-show-signature",
                "--no-decorate",
                "--no-color",
                "--no-notes",
                "--max-count=20",
                "--format=%h %s",
                range,
            ],
        )?
        .lines()
        .map(str::to_owned)
        .collect())
    };
    let outgoing = commits(&format!("{remote_oid}..{local_oid}"))?;
    let incoming = commits(&format!("{local_oid}..{remote_oid}"))?;
    let changes = read(project, &["status", "--porcelain=v1", "-uall"])?;
    let mut blocked = (!changes.is_empty()).then(|| "Save your changed files to history before transferring commits. Resolve any conflicted files first.".to_owned());
    let mut markers = Vec::new();
    for marker in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "rebase-merge",
        "rebase-apply",
        "sequencer",
        "BISECT_START",
    ] {
        // rev-parse resolves worktree-specific Git paths. Test existence inside the execution
        // boundary too, rather than trusting the host's filesystem view.
        markers.push(read(project, &["rev-parse", "--git-path", marker])?);
    }
    let program = crate::exec::trusted_program("bash", project)
        .ok_or("Could not inspect an unfinished Git operation.")?;
    let mut command = crate::isolation::command(&program, project, "")?;
    command
        .current_dir(project)
        .env_remove("BASH_ENV")
        .env_remove("ENV")
        .args([
            "--noprofile",
            "--norc",
            "-c",
            "for path; do if test -e \"$path\"; then exit 1; fi; done; exit 0",
            "verb-git-check",
        ])
        .args(markers);
    let output = crate::exec::read_output_before(&mut command, deadline)
        .map_err(|_| "Could not inspect an unfinished Git operation.")?;
    let in_progress = match output.status.code() {
        Some(0) => false,
        Some(1) => true,
        _ => return Err("Could not inspect an unfinished Git operation.".into()),
    };
    if in_progress {
        blocked = Some("Finish the merge, rebase or other Git operation already in progress before transferring work. Verb will not discard or resolve it automatically.".into());
    } else if ahead > 0 && behind > 0 {
        blocked = Some("Both this branch and its remote have new commits. Ask a developer or your coding agent to review how to combine them in a terminal, then check again. Verb will not overwrite either history.".into());
    }
    Ok(Review {
        branch,
        local_oid,
        remote: remote.into(),
        server,
        remote_ref: fields[3].into(),
        tracking_ref: fields[1].into(),
        remote_oid,
        destination_version,
        ahead,
        behind,
        outgoing,
        incoming,
        blocked,
    })
}

fn run(project: &Path, args: &[&str]) -> Result<(), String> {
    let mut command = crate::exec::user_git(project).ok_or("Git could not be prepared.")?;
    command.args(args);
    // No raw stderr in the response: Git transports can echo credential-bearing URLs.
    let output = crate::exec::git_operation_output(&mut command)
        .map_err(|_| "Git did not finish within its limits. The outcome is unconfirmed; check the remote again before retrying. A remote or hook may have completed part of the operation.".to_owned())?;
    if !output.status.success() {
        return Err("Git could not complete the transfer. Check your connection, sign-in and repository permissions in a terminal, then check the remote again. If histories changed, review them before retrying. Verb has not confirmed completion.".into());
    }
    Ok(())
}

pub(crate) fn transfer(project: &Path, request: Request) -> Result<Review, String> {
    if !matches!(request.action.as_str(), "check" | "upload" | "download") {
        return Err("Unknown Git transfer action.".into());
    }
    let _transfer = TRANSFER
        .try_lock()
        .map_err(|_| "Another Git transfer is running. Wait for it to finish, then check again.")?;
    let before = review(project)?;
    if before != request.review {
        return Err(
            "This project's Git state changed. Reopen the review before transferring work.".into(),
        );
    }
    if request.action != "check" {
        if let Some(reason) = &before.blocked {
            return Err(reason.clone());
        }
    }
    // Fetch only the reviewed remote branch into its tracking ref. No tags, pruning,
    // submodules, working-tree changes or FETCH_HEAD shared with an owner's pull.
    let refspec = format!("+{}:{}", before.remote_ref, before.tracking_ref);
    run(
        project,
        &[
            "fetch",
            "--no-tags",
            "--no-prune",
            "--no-prune-tags",
            "--no-recurse-submodules",
            "--no-write-fetch-head",
            "--no-auto-maintenance",
            "--refmap=",
            "--",
            &before.remote,
            &refspec,
        ],
    )?;
    let fresh = review(project)?;
    if request.action == "check" {
        return Ok(fresh);
    }
    if fresh != before {
        return Err("The remote or this project changed. Reopen the review to see the latest commits before continuing.".into());
    }
    match request.action.as_str() {
        "upload" if fresh.ahead > 0 && fresh.behind == 0 => {
            let refspec = format!("{}:{}", fresh.local_oid, fresh.remote_ref);
            run(
                project,
                &[
                    "push",
                    "--porcelain",
                    "--no-follow-tags",
                    "--recurse-submodules=no",
                    "--",
                    &fresh.remote,
                    &refspec,
                ],
            )?;
        }
        "download" if fresh.behind > 0 && fresh.ahead == 0 => {
            run(
                project,
                &[
                    "merge",
                    "--ff-only",
                    "--no-edit",
                    "--no-autostash",
                    &fresh.remote_oid,
                ],
            )?;
        }
        _ => return Err(
            "There are no reviewed commits available for that transfer. Check the remote again."
                .into(),
        ),
    }
    review(project)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    static SERIAL: Mutex<()> = Mutex::new(());

    #[test]
    fn server_labels_never_include_url_credentials_or_queries() {
        assert_eq!(
            server_label("https://person:synthetic-secret@github.com/owner/repo?token=hidden")
                .unwrap(),
            "github.com"
        );
        assert_eq!(
            server_label("git@github.com:owner/repo.git").unwrap(),
            "github.com"
        );
        assert_eq!(
            server_label("ssh://person@git.example.invalid:2222/repo").unwrap(),
            "git.example.invalid:2222"
        );
        assert_eq!(
            server_label("/tmp/fixture.git").unwrap(),
            "local Git repository"
        );
        assert!(server_label("ext://custom-helper").is_err());
        assert!(server_label("ext::custom-helper").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejected_upload_preserves_history_and_does_not_echo_transport_secrets() {
        use std::os::unix::fs::PermissionsExt;
        let _serial = SERIAL.lock().unwrap();
        let f = Fixture::new();
        f.commit(&f.local, "local contribution\n");
        let remote_before = git(&f.remote, &["rev-parse", "main"]);
        let hook = f.local.join(".git/hooks/pre-push");
        fs::write(
            &hook,
            "#!/bin/sh\necho 'https://synthetic-secret@example.invalid/repo' >&2\nexit 1\n",
        )
        .unwrap();
        fs::set_permissions(hook, fs::Permissions::from_mode(0o700)).unwrap();
        let error = f.act("upload").unwrap_err();
        assert!(!error.contains("synthetic-secret"));
        assert!(error.contains("not confirmed completion"));
        assert_eq!(git(&f.remote, &["rev-parse", "main"]), remote_before);
        assert_eq!(review(&f.local).unwrap().ahead, 1);
        assert_eq!(
            fs::read_to_string(f.local.join("work.txt")).unwrap(),
            "local contribution\n"
        );
    }
    struct Fixture {
        root: PathBuf,
        local: PathBuf,
        other: PathBuf,
        remote: PathBuf,
    }
    fn git(at: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(at)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim_end().into()
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("verb-transfer-{}", crate::new_id()));
            fs::create_dir_all(&root).unwrap();
            let remote = root.join("remote.git");
            let local = root.join("local");
            let other = root.join("other");
            git(
                &root,
                &[
                    "init",
                    "--bare",
                    "-q",
                    "-b",
                    "main",
                    remote.to_str().unwrap(),
                ],
            );
            git(
                &root,
                &[
                    "clone",
                    "-q",
                    remote.to_str().unwrap(),
                    local.to_str().unwrap(),
                ],
            );
            git(&local, &["config", "user.name", "Transfer Fixture"]);
            git(&local, &["config", "user.email", "fixture@example.invalid"]);
            fs::write(local.join("work.txt"), "initial\n").unwrap();
            git(&local, &["add", "."]);
            git(&local, &["commit", "-qm", "initial"]);
            git(&local, &["push", "-q", "-u", "origin", "main"]);
            git(
                &root,
                &[
                    "clone",
                    "-q",
                    remote.to_str().unwrap(),
                    other.to_str().unwrap(),
                ],
            );
            git(&other, &["config", "user.name", "Other Fixture"]);
            git(&other, &["config", "user.email", "other@example.invalid"]);
            Self {
                root,
                local,
                other,
                remote,
            }
        }
        fn commit(&self, at: &Path, text: &str) {
            fs::write(at.join("work.txt"), text).unwrap();
            git(at, &["add", "."]);
            git(at, &["commit", "-qm", "fixture change"]);
        }
        fn act(&self, action: &str) -> Result<Review, String> {
            transfer(
                &self.local,
                Request {
                    action: action.into(),
                    review: review(&self.local)?,
                },
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn uploads_exact_commits_and_downloads_only_reviewed_fast_forwards() {
        let _serial = SERIAL.lock().unwrap();
        let f = Fixture::new();
        f.commit(&f.local, "from local\n");
        git(&f.local, &["tag", "-a", "private-tag", "-m", "keep local"]);
        git(&f.local, &["branch", "private-work"]);
        git(&f.local, &["config", "push.followTags", "true"]);
        assert_eq!(f.act("check").unwrap().ahead, 1);
        f.act("upload").unwrap();
        assert_eq!(
            git(&f.remote, &["for-each-ref", "--format=%(refname)"]),
            "refs/heads/main"
        );
        assert_eq!(
            git(&f.local, &["rev-parse", "HEAD"]),
            git(&f.remote, &["rev-parse", "main"])
        );
        git(&f.other, &["pull", "--ff-only", "-q"]);
        f.commit(&f.other, "from remote\n");
        git(&f.other, &["push", "-q"]);
        let before = git(&f.local, &["rev-parse", "HEAD"]);
        assert!(f.act("download").unwrap_err().contains("Reopen the review"));
        assert_eq!(git(&f.local, &["rev-parse", "HEAD"]), before);
        assert_eq!(f.act("check").unwrap().behind, 1);
        f.act("download").unwrap();
        assert_eq!(
            fs::read_to_string(f.local.join("work.txt")).unwrap(),
            "from remote\n"
        );
        assert!(git(&f.local, &["status", "--porcelain"]).is_empty());
    }

    #[test]
    fn refuses_dirty_diverged_stale_and_in_progress_work_without_discarding_it() {
        let _serial = SERIAL.lock().unwrap();
        let f = Fixture::new();
        let old = review(&f.local).unwrap();
        f.commit(&f.local, "local work\n");
        assert!(transfer(
            &f.local,
            Request {
                action: "upload".into(),
                review: old
            }
        )
        .unwrap_err()
        .contains("state changed"));
        fs::write(f.local.join("keep.txt"), "unsaved owner work").unwrap();
        assert!(f
            .act("upload")
            .unwrap_err()
            .contains("Save your changed files"));
        assert_eq!(
            fs::read_to_string(f.local.join("keep.txt")).unwrap(),
            "unsaved owner work"
        );
        fs::remove_file(f.local.join("keep.txt")).unwrap();
        fs::create_dir(f.local.join(".git/rebase-merge")).unwrap();
        assert!(f.act("upload").unwrap_err().contains("already in progress"));
        fs::remove_dir(f.local.join(".git/rebase-merge")).unwrap();
        f.commit(&f.other, "different remote work\n");
        git(&f.other, &["push", "-q"]);
        let checked = f.act("check").unwrap();
        assert!(checked.blocked.unwrap().contains("Both this branch"));
        let local = git(&f.local, &["rev-parse", "HEAD"]);
        let remote = git(&f.remote, &["rev-parse", "main"]);
        assert!(f.act("upload").is_err());
        assert!(f.act("download").is_err());
        assert_eq!(git(&f.local, &["rev-parse", "HEAD"]), local);
        assert_eq!(git(&f.remote, &["rev-parse", "main"]), remote);
        // An actual unresolved merge is preserved for the user to resolve.
        let output = Command::new("git")
            .arg("-C")
            .arg(&f.local)
            .args(["merge", "origin/main"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        let conflicted = fs::read_to_string(f.local.join("work.txt")).unwrap();
        assert!(conflicted.contains("<<<<<<<"));
        assert!(f.act("download").is_err());
        assert_eq!(
            fs::read_to_string(f.local.join("work.txt")).unwrap(),
            conflicted
        );
        assert!(f.local.join(".git/MERGE_HEAD").exists());
    }

    #[test]
    fn destination_changes_and_indirect_tracking_refs_require_a_new_review() {
        let _serial = SERIAL.lock().unwrap();
        let f = Fixture::new();
        let old = review(&f.local).unwrap();
        let second = f.root.join("second.git");
        git(
            &f.root,
            &[
                "clone",
                "--bare",
                "-q",
                f.remote.to_str().unwrap(),
                second.to_str().unwrap(),
            ],
        );
        git(
            &f.local,
            &["remote", "set-url", "origin", second.to_str().unwrap()],
        );
        assert!(transfer(
            &f.local,
            Request {
                action: "check".into(),
                review: old
            }
        )
        .unwrap_err()
        .contains("state changed"));
        git(
            &f.local,
            &[
                "remote",
                "set-url",
                "--push",
                "origin",
                "https://synthetic-secret@example.invalid/repo",
            ],
        );
        let error = review(&f.local).unwrap_err();
        assert!(error.contains("separate upload destination"));
        assert!(!error.contains("synthetic-secret"));
        git(
            &f.local,
            &["config", "--unset-all", "remote.origin.pushurl"],
        );
        git(
            &f.local,
            &[
                "symbolic-ref",
                "refs/remotes/origin/main",
                "refs/heads/main",
            ],
        );
        assert!(review(&f.local).unwrap_err().contains("indirect"));
    }
}
