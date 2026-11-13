use std::path::Path;
use std::process::Command;

pub fn fixture(root: &Path) {
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .current_dir(root)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "{args:?}: {:?}", output);
    };
    git(&["init", "-q"]);
    git(&["config", "status.renames", "copies"]);
    let source = "copy source line 1\ncopy source line 2\ncopy source line 3\ncopy source line 4\n";
    let renamed = "rename source line 1\nrename source line 2\nrename source line 3\n";
    std::fs::write(root.join("copy source"), source).unwrap();
    std::fs::write(root.join("old -> name"), renamed).unwrap();
    std::fs::write(root.join("literal -> name"), "before\n").unwrap();
    git(&["add", "."]);
    git(&[
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.invalid",
        "commit",
        "-qm",
        "fixture",
    ]);
    git(&["mv", "old -> name", "new -> name"]);
    std::fs::write(root.join("new -> name"), format!("{renamed}extra\n")).unwrap();
    std::fs::write(root.join("copied -> name"), source).unwrap();
    std::fs::write(root.join("copy source"), format!("{source}extra\n")).unwrap();
    git(&["add", "--", "copy source", "copied -> name", "new -> name"]);
    std::fs::write(root.join("literal -> name"), "after\n").unwrap();
}

pub fn assert_summary(summary: &serde_json::Value) {
    let changes = summary["changes"].as_array().unwrap();
    for (path, code, source) in [
        ("new -> name", "R", Some("old -> name")),
        ("copied -> name", "C", Some("copy source")),
        ("literal -> name", "M", None),
    ] {
        let change = changes
            .iter()
            .find(|change| change["path"] == path)
            .unwrap_or_else(|| panic!("Missing {path}: {summary}"));
        assert_eq!(change["code"], code, "{change}");
        assert_eq!(change["previous_path"].as_str(), source, "{change}");
    }
}

#[allow(dead_code)]
pub fn assert_previews(mut request: impl FnMut(&str) -> (u16, serde_json::Value)) {
    for (path, code) in [
        ("new%20-%3E%20name", "R"),
        ("copied%20-%3E%20name", "C"),
        ("literal%20-%3E%20name", "M"),
    ] {
        let (status, diff) = request(&format!("/api/git/diff?path={path}"));
        assert_eq!(status, 200, "{diff}");
        assert_eq!(diff["code"], code, "{diff}");
        assert_eq!(diff["added"], if code == "C" { 0 } else { 1 }, "{diff}");
    }
}
