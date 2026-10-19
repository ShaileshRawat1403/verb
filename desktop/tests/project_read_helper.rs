//! The batched guest reads keep the normal preview and no-repository-code-execution guards.
use std::path::Path;
use std::process::{Command, Output};

fn git(root: &Path, args: &[&str]) {
    assert!(Command::new("git")
        .current_dir(root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .args(args)
        .output()
        .unwrap()
        .status
        .success());
}

fn helper(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_verb"))
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .current_dir(root)
        .arg("__project-read")
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn project_reads_are_batched_but_never_run_repo_hooks_or_preview_ignored_files() {
    let root = std::env::temp_dir().join(format!("verb-project-read-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    git(&root, &["init", "-q", "-b", "main"]);
    std::fs::write(root.join("sample"), "before\n").unwrap();
    std::fs::write(root.join(".gitattributes"), "sample filter=fixture\n").unwrap();
    std::fs::write(root.join(".gitignore"), "ignored-uat.txt\n").unwrap();
    git(&root, &["add", "."]);
    git(
        &root,
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
    git(&root, &["config", "core.fsmonitor", "touch monitor-ran"]);
    git(
        &root,
        &["config", "filter.fixture.clean", "touch filter-ran; cat"],
    );
    git(&root, &["config", "filter.fixture.required", "true"]);
    std::fs::write(root.join("sample"), "after!\n").unwrap();
    std::fs::write(root.join("ignored-uat.txt"), "synthetic ignored fixture\n").unwrap();
    for args in [&["files"][..], &["git"], &["preview", "sample"]] {
        let result = helper(&root, args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        match args[0] {
            "files" => {
                assert!(value["files"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|f| f["path"] == "sample"));
                assert!(!value["files"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|f| f["path"] == "ignored-uat.txt"));
            }
            "git" => {
                assert_eq!(value["branch"], "main");
                assert!(!value["changes"].as_array().unwrap().is_empty());
            }
            "preview" => assert_eq!(value["text"], "after!\n"),
            _ => unreachable!(),
        }
        assert!(!root.join("monitor-ran").exists());
        assert!(!root.join("filter-ran").exists());
    }
    for args in [
        &["unknown"][..],
        &["files", "extra"],
        &["preview", "../sample"],
        &["preview", "/etc/passwd"],
        &["preview", "ignored-uat.txt"],
    ] {
        assert!(!helper(&root, args).status.success(), "{args:?}");
    }
    std::fs::remove_dir_all(root).unwrap();
}
