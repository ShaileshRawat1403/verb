use std::path::PathBuf;
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(std::env::var_os("CARGO_MANIFEST_DIR")?)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .args(["-c", "core.fsmonitor=false"])
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn main() {
    let package =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory"));
    let source_tree = git(&["rev-parse", "--show-toplevel"])
        .and_then(|p| PathBuf::from(p).canonicalize().ok())
        .is_some_and(|p| {
            package
                .parent()
                .and_then(|q| q.canonicalize().ok())
                .as_ref()
                == Some(&p)
                || package.canonicalize().ok().as_ref() == Some(&p)
        });
    let revision = source_tree
        .then(|| git(&["rev-parse", "--verify", "HEAD"]))
        .flatten()
        .filter(|s| s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit()));
    let dirty = source_tree
        .then(|| git(&["status", "--porcelain", "--untracked-files=normal"]))
        .flatten()
        .map(|s| !s.is_empty());
    println!(
        "cargo:rustc-env=VERB_BUILD_REVISION={}",
        revision.as_deref().unwrap_or("unknown")
    );
    println!(
        "cargo:rustc-env=VERB_BUILD_DIRTY={}",
        dirty.map_or("unknown", |d| if d { "true" } else { "false" })
    );
    for path in ["src", "web/dist", "Cargo.toml", "Cargo.lock", "build.rs"] {
        println!("cargo:rerun-if-changed={path}");
    }
    // Worktrees may keep HEAD/index in a separate directory from shared refs.
    for item in ["HEAD", "index", "refs", "packed-refs"] {
        if let Some(path) = git(&["rev-parse", "--git-path", item]) {
            let path = PathBuf::from(path);
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}
