//! Transient execution failures must never split a Git project's ledger identity.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};

fn git(directory: &Path, args: &[&str]) {
    let result = Command::new("/usr/bin/git")
        .current_dir(directory)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .args(args)
        .output()
        .unwrap();
    assert!(result.status.success());
}

fn project(root: &Path, directory: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_verb"))
        .env_clear()
        .env("HOME", root.join("home"))
        .env("VERB_STATE_DIR", root.join("state"))
        .env("VERB_TEST_GIT_DOWN", root.join("git-down"))
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", root.join("bin").display()),
        )
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .current_dir(directory)
        .args(["project", "--json"])
        .output()
        .unwrap()
}

fn records(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(root.join("state/projects"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|value| value == "json"))
        .map(|path| {
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                fs::read(path).unwrap(),
            )
        })
        .collect()
}

#[test]
fn unavailable_git_preserves_identity_and_work_for_checkout_and_linked_worktree() {
    let root = std::env::temp_dir().join(format!("verb-identity-{}", std::process::id()));
    for name in ["", "project", "home", "bin", "plain-folder"] {
        fs::create_dir(root.join(name)).unwrap();
    }
    let main = root.join("project");
    git(&main, &["init", "-q", "-b", "main"]);
    git(
        &main,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--allow-empty",
            "-qm",
            "fixture",
        ],
    );
    let linked = root.join("linked");
    git(
        &main,
        &[
            "worktree",
            "add",
            "--detach",
            linked.to_str().unwrap(),
            "HEAD",
        ],
    );
    let wrapper = root.join("bin/git");
    fs::write(&wrapper, "#!/bin/sh\nif test -e \"$VERB_TEST_GIT_DOWN\"; then exit 128; fi\nexec /usr/bin/git \"$@\"\n").unwrap();
    fs::set_permissions(wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    let baseline = project(&root, &main);
    assert!(baseline.status.success());
    let baseline: serde_json::Value = serde_json::from_slice(&baseline.stdout).unwrap();
    let id = baseline["projectId"].as_str().unwrap();
    let ledger = root.join("state/work").join(format!("p-{id}"));
    fs::create_dir_all(&ledger).unwrap();
    fs::write(ledger.join("memory.md"), "synthetic shared memory\n").unwrap();
    let before = records(&root);
    for directory in [&main, &linked] {
        fs::write(root.join("git-down"), "synthetic execution outage").unwrap();
        let unavailable = project(&root, directory);
        assert!(!unavailable.status.success());
        assert!(String::from_utf8_lossy(&unavailable.stderr)
            .contains("Git project identity unavailable"));
        assert_eq!(
            records(&root),
            before,
            "failed probes must not mint a folder ID"
        );
        assert_eq!(
            fs::read_to_string(ledger.join("memory.md")).unwrap(),
            "synthetic shared memory\n"
        );
        fs::remove_file(root.join("git-down")).unwrap();
        let recovered = project(&root, directory);
        assert!(recovered.status.success());
        let recovered: serde_json::Value = serde_json::from_slice(&recovered.stdout).unwrap();
        assert_eq!(recovered["projectId"], id);
        assert_eq!(records(&root), before);
    }
    // Ordinary folders still work when Git is absent; only existing Git markers
    // make an unavailable observation unsafe for identity selection.
    fs::write(root.join("git-down"), "synthetic execution outage").unwrap();
    assert!(project(&root, &root.join("plain-folder")).status.success());
    fs::remove_dir_all(root).unwrap();
}
