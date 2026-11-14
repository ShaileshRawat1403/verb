//! The batched boundary helper must retain the ordinary observer's no-code-execution guards.
use std::path::Path;
use std::process::{Command, Output};

fn git(directory: &Path, args: &[&str]) {
    assert!(Command::new("git")
        .current_dir(directory)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .args(args)
        .output()
        .unwrap()
        .status
        .success());
}

fn snapshot(directory: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_verb"))
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .current_dir(directory)
        .arg("__git-snapshot")
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn batched_snapshot_observes_changes_without_running_repository_commands() {
    let directory = std::env::temp_dir().join(format!("verb-snapshot-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    git(&directory, &["init", "-q", "-b", "main"]);
    std::fs::write(directory.join(".gitattributes"), "sample filter=fixture\n").unwrap();
    std::fs::write(directory.join("sample"), "before\n").unwrap();
    git(&directory, &["add", "."]);
    git(
        &directory,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "fixture",
        ],
    );
    git(
        &directory,
        &["config", "filter.fixture.clean", "touch filter-ran; cat"],
    );
    git(&directory, &["config", "filter.fixture.required", "true"]);
    git(
        &directory,
        &["config", "core.fsmonitor", "touch monitor-ran"],
    );
    // Same size, different content: Git may need a clean filter to compare it.
    std::fs::write(directory.join("sample"), "after!\n").unwrap();
    let result = snapshot(&directory, &[]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let facts: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(facts["branch"], "main");
    assert_eq!(facts["changes_known"], true);
    assert_eq!(facts["changed_files"], 1);
    assert!(!directory.join("filter-ran").exists());
    assert!(!directory.join("monitor-ran").exists());
    assert!(!snapshot(&directory, &["unexpected"]).status.success());
    std::fs::write(directory.join(".git/HEAD"), "broken fixture").unwrap();
    let broken = snapshot(&directory, &[]);
    let facts: serde_json::Value = serde_json::from_slice(&broken.stdout).unwrap();
    assert_eq!(facts["changes_known"], false);
    assert_eq!(facts["status"], "Unavailable");
    std::fs::remove_dir_all(directory).unwrap();
}
