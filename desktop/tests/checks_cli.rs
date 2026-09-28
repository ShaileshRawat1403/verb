//! `verb check`, `verb runtime` and `verb good`, driven through the real binary against a real
//! repository. The unit tests cover parsing and wording; these cover the wiring, the JSON contract
//! and the privacy promise that none of these outputs carries a file or branch name.

#![cfg(unix)]

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(label: &str) -> Option<Self> {
        Command::new("git").arg("--version").output().ok()?;
        let root = std::env::temp_dir().join(format!(
            "verb-checks-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("repo")).unwrap();
        let scratch = Self { root };
        scratch.git(&["init", "-q", "-b", "feature/acme-login"]);
        fs::write(scratch.repo().join("app.txt"), "one\n").unwrap();
        fs::write(scratch.repo().join(".nvmrc"), "99\n").unwrap();
        scratch.git(&["add", "."]);
        scratch.git(&["commit", "-q", "-m", "one"]);
        Some(scratch)
    }

    fn repo(&self) -> PathBuf {
        self.root.join("repo")
    }

    fn git(&self, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(self.repo())
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

    fn verb(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_verb"))
            .args(args)
            .current_dir(self.repo())
            .env("VERB_STATE_DIR", self.root.join("state"))
            .env("HOME", self.root.join("home"))
            .output()
            .unwrap()
    }

    fn json(&self, args: &[&str]) -> Value {
        let output = self.verb(args);
        assert!(output.status.success(), "verb {args:?}: {output:?}");
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "verb {args:?} printed invalid JSON ({error}): {}",
                String::from_utf8_lossy(&output.stdout)
            )
        })
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn assert_no_names(text: &str, path: &Path) {
    assert!(!text.contains("acme"), "branch name leaked: {text}");
    assert!(!text.contains("app.txt"), "file name leaked: {text}");
    assert!(
        !text.contains(&*path.to_string_lossy()),
        "path leaked: {text}"
    );
}

#[test]
fn check_reports_a_declared_runtime_that_does_not_match() {
    let Some(s) = Scratch::new("runtime") else {
        return;
    };
    let report = s.json(&["check", "--json"]);
    assert_eq!(report["schemaVersion"], 1);
    assert_eq!(report["repository"], serde_json::json!([]));
    let node = &report["runtimes"][0];
    assert_eq!(node["runtime"], "node");
    assert_eq!(node["source"], ".nvmrc");
    assert_eq!(node["wants"], "99");
    // Missing when no node is installed, mismatch when one is: never satisfied.
    assert!(
        matches!(node["verdict"].as_str(), Some("mismatch" | "missing")),
        "{node}"
    );
    assert_eq!(report["clear"], false);
    assert_eq!(report["lastKnownGood"], Value::Null);
    assert_no_names(&report.to_string(), &s.repo());

    let runtime = s.json(&["runtime", "--json"]);
    assert_eq!(runtime["runtimes"][0]["source"], ".nvmrc");
}

#[test]
fn good_marks_compares_and_forgets_without_storing_names() {
    let Some(s) = Scratch::new("good") else {
        return;
    };
    let status = s.json(&["good", "--json"]);
    assert_eq!(status["mark"], Value::Null);

    let mark = s.json(&["good", "mark", "--json"]);
    assert_eq!(mark["uncommitted"], 0);
    assert_eq!(mark["fingerprinted"], true);

    let same = s.json(&["good", "--json"]);
    assert_eq!(same["distance"]["identical"], true);

    fs::write(s.repo().join("app.txt"), "two\n").unwrap();
    s.git(&["commit", "-q", "-am", "two"]);
    let moved = s.json(&["good", "--json"]);
    assert_eq!(moved["distance"]["identical"], false);
    assert_eq!(moved["distance"]["commitsSince"], 1);
    assert_eq!(moved["distance"]["filesDiffer"], 1);
    assert_no_names(&moved.to_string(), &s.repo());

    // Names are available on request, read live, and only there.
    let files = s.json(&["good", "files", "--json"]);
    assert_eq!(files["files"], serde_json::json!(["app.txt"]));

    let text = String::from_utf8(s.verb(&["good"]).stdout).unwrap();
    assert!(text.contains("1 commit, 1 file differs"), "{text}");
    assert!(text.contains("git stash push -u"), "{text}");

    // The project registry records its anchor path by design; the mark itself must not.
    let marks: Vec<PathBuf> = walk(&s.root.join("state"))
        .into_iter()
        .filter(|path| path.to_string_lossy().contains("last-known-good"))
        .collect();
    assert_eq!(marks.len(), 1, "{marks:?}");
    for entry in marks {
        let stored = fs::read_to_string(&entry).unwrap_or_default();
        assert_no_names(&stored, &s.repo());
    }

    assert_eq!(s.json(&["good", "forget", "--json"])["forgotten"], true);
    assert_eq!(s.json(&["good", "--json"])["mark"], Value::Null);
}

#[test]
fn check_names_an_unfinished_merge_and_its_safe_exit() {
    let Some(s) = Scratch::new("merge") else {
        return;
    };
    s.git(&["switch", "-q", "-c", "side"]);
    fs::write(s.repo().join("app.txt"), "side\n").unwrap();
    s.git(&["commit", "-q", "-am", "side"]);
    s.git(&["switch", "-q", "feature/acme-login"]);
    fs::write(s.repo().join("app.txt"), "main\n").unwrap();
    s.git(&["commit", "-q", "-am", "main"]);
    let merge = Command::new("git")
        .args(["merge", "-q", "side"])
        .current_dir(s.repo())
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(!merge.status.success(), "the merge was meant to conflict");

    let report = s.json(&["check", "--json"]);
    let codes: Vec<&str> = report["repository"]
        .as_array()
        .unwrap()
        .iter()
        .map(|warning| warning["code"].as_str().unwrap())
        .collect();
    assert_eq!(codes, ["operation-in-progress", "unmerged-paths"]);
    assert!(report["repository"][0]["safeNext"]
        .as_str()
        .unwrap()
        .contains("git merge --abort"));
    assert_no_names(&report.to_string(), &s.repo());
}

fn walk(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return found;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            found.extend(walk(&path));
        } else {
            found.push(path);
        }
    }
    found
}
