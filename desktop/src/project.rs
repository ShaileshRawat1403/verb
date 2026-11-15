//! One Verb project can contain several Git worktrees. Native agent conversations still belong
//! to their actual checkout path; only Verb's memory, tasks, and fetch receipts share this ID.

use crate::fsutil::atomic_write;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::{hex_encode, new_id, state_root};

#[derive(Clone, Debug)]
pub(crate) struct ProjectIdentity {
    pub id: String,
    pub anchor: PathBuf,
    pub store: PathBuf,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectRecord {
    schema_version: u8,
    key: String,
    id: String,
    anchor: PathBuf,
}

struct Location {
    key: String,
    anchor: PathBuf,
    checkout: PathBuf,
    git: bool,
}

fn git_output(project: &Path, args: &[&str]) -> Option<String> {
    let output = crate::exec::read_output(&mut crate::exec::git_for(project, args)?).ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8(output.stdout).ok()?.trim().to_owned())
}

/// `.git` as this checkout sees it. A main checkout's `.git` directory is the common dir itself,
/// so its device, inode and birth time identify the answer (its mtime moves on every index write,
/// so it is not used; birth time tells a recreated `.git` apart from a recycled inode). A linked
/// worktree's `.git` file names its gitdir, so its inode, size and nanosecond mtime are used:
/// re-pointing the worktree rewrites it. Without a birth time a directory is not cached.
#[cfg(unix)]
fn dot_git(checkout: &Path) -> Option<(u64, u64, u64, i64, i64, bool)> {
    use std::os::unix::fs::MetadataExt;
    let meta = fs::symlink_metadata(checkout.join(".git")).ok()?;
    if meta.is_dir() {
        let born = meta
            .created()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?;
        Some((
            meta.dev(),
            meta.ino(),
            0,
            born.as_secs() as i64,
            i64::from(born.subsec_nanos()),
            true,
        ))
    } else if meta.is_file() {
        Some((
            meta.dev(),
            meta.ino(),
            meta.len(),
            meta.mtime(),
            meta.mtime_nsec(),
            false,
        ))
    } else {
        None
    }
}

#[cfg(not(unix))]
fn dot_git(_: &Path) -> Option<(u64, u64, u64, i64, i64, bool)> {
    None
}

type CommonDirs = HashMap<PathBuf, ((u64, u64, u64, i64, i64, bool), String)>;
static COMMON_DIRS: std::sync::LazyLock<std::sync::Mutex<CommonDirs>> =
    std::sync::LazyLock::new(Default::default);

/// `git rev-parse --git-common-dir`, which every work item, hub and session read asks for. It
/// only changes when the checkout's `.git` does, so it is asked again only then. On a node each
/// ask is a boundary entry. A checkout without a readable `.git` is asked every time.
fn common_dir(checkout: &Path) -> Option<String> {
    let identity = dot_git(checkout);
    if let Some(identity) = identity {
        if let Some((seen, value)) = COMMON_DIRS.lock().ok()?.get(checkout) {
            if *seen == identity {
                return Some(value.clone());
            }
        }
    }
    let value = git_output(checkout, &["rev-parse", "--git-common-dir"])?;
    if let (Some(identity), Ok(mut cache)) = (identity, COMMON_DIRS.lock()) {
        if !value.is_empty() && dot_git(checkout) == Some(identity) {
            if cache.len() > 64 {
                cache.clear();
            }
            cache.insert(checkout.to_path_buf(), (identity, value.clone()));
        }
    }
    Some(value)
}

fn location(project: &Path) -> Result<Location, String> {
    let checkout = fs::canonicalize(project)
        .or_else(|error| {
            if project.is_absolute() && error.kind() == std::io::ErrorKind::NotFound {
                Ok(project.to_path_buf())
            } else {
                Err(error)
            }
        })
        .map_err(|error| format!("could not open project {}: {error}", project.display()))?;
    let common = common_dir(&checkout)
        .filter(|value| !value.is_empty())
        .and_then(|value| {
            let path = PathBuf::from(value);
            fs::canonicalize(if path.is_absolute() {
                path
            } else {
                checkout.join(path)
            })
            .ok()
        });
    if let Some(common) = common {
        let anchor = if common.file_name().is_some_and(|name| name == ".git") {
            common.parent().unwrap_or(&checkout).to_path_buf()
        } else {
            checkout.clone()
        };
        Ok(Location {
            key: format!("git:{}", common.display()),
            anchor,
            checkout,
            git: true,
        })
    } else {
        // A failed Git probe is not evidence that an existing checkout became
        // a plain directory. In particular, its execution controller may still
        // be starting. Refuse before minting another ID/work store; supervision
        // can retry startup once Git is available.
        if checkout
            .ancestors()
            .any(|path| fs::symlink_metadata(path.join(".git")).is_ok())
        {
            return Err("Git project identity unavailable; retry when Git and its execution boundary are ready".into());
        }
        Ok(Location {
            key: format!("dir:{}", checkout.display()),
            anchor: checkout.clone(),
            checkout,
            git: false,
        })
    }
}

pub(crate) fn same_project(left: &Path, right: &Path) -> bool {
    left == right
        || matches!((location(left), location(right)), (Ok(left), Ok(right)) if left.key == right.key)
}

pub(crate) fn identity(project: &Path) -> Result<ProjectIdentity, String> {
    let location = location(project)?;
    let root = state_root()?;
    let registry = root.join("projects");
    fs::create_dir_all(&registry)
        .map_err(|error| format!("could not create project registry: {error}"))?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(registry.join(".lock"))
        .map_err(|error| format!("could not open project registry lock: {error}"))?;
    lock.lock()
        .map_err(|error| format!("could not lock project registry: {error}"))?;
    let key_hash = format!("{:x}", Sha256::digest(location.key.as_bytes()));
    let record_path = registry.join(format!("{key_hash}.json"));
    let record = match read_record(&record_path) {
        Ok(value) => {
            let record: ProjectRecord = serde_json::from_str(&value).map_err(|error| {
                format!(
                    "invalid Verb project record {}: {error}",
                    record_path.display()
                )
            })?;
            if record.schema_version != 1
                || record.key != location.key
                || record.id.len() != 32
                || !record.id.bytes().all(|byte| byte.is_ascii_hexdigit())
                || !record.anchor.is_absolute()
            {
                return Err(format!(
                    "invalid Verb project identity: {}",
                    record_path.display()
                ));
            }
            record
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let record = ProjectRecord {
                schema_version: 1,
                key: location.key.clone(),
                id: new_id(),
                anchor: location.anchor.clone(),
            };
            let bytes = serde_json::to_vec_pretty(&record).map_err(|error| error.to_string())?;
            atomic_write(&record_path, &bytes)?;
            record
        }
        Err(error) => return Err(format!("could not read Verb project record: {error}")),
    };
    let store = root.join("work").join(format!("p-{}", record.id));
    // Older builds keyed their work store by the checkout path. Preserve those records before
    // publishing any new work in the ID-keyed store. A second populated legacy store is an
    // explicit conflict, never silently discarded or merged.
    let mut candidates = vec![record.anchor.clone()];
    if location.checkout != record.anchor {
        candidates.push(location.checkout);
    }
    for path in candidates {
        let legacy = root.join("work").join(hex_encode(&path));
        if !has_work_data(&legacy)? {
            continue;
        }
        if store.exists() {
            return Err(format!(
                "two Verb work stores need a manual merge: {} and {}",
                store.display(),
                legacy.display()
            ));
        }
        fs::create_dir_all(store.parent().ok_or("invalid work store path")?)
            .map_err(|error| format!("could not create work store: {error}"))?;
        fs::rename(&legacy, &store)
            .map_err(|error| format!("could not migrate Verb work store: {error}"))?;
    }
    Ok(ProjectIdentity {
        id: record.id,
        anchor: record.anchor,
        store,
    })
}

fn read_record(path: &Path) -> std::io::Result<String> {
    let mut bytes = Vec::new();
    File::open(path)?.take(4097).read_to_end(&mut bytes)?;
    if bytes.len() > 4096 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "project record exceeds 4096 bytes",
        ));
    }
    String::from_utf8(bytes)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

fn has_work_data(path: &Path) -> Result<bool, String> {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("could not inspect old work store: {error}")),
    };
    for entry in entries {
        let entry = entry.map_err(|error| format!("could not inspect old work store: {error}"))?;
        if entry.file_name() != ".lock" {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Separate execution accounts cannot access or register worktrees in the host's private home.
/// Keep this explicit until the operator can provision additional checkout roots safely.
pub(crate) fn isolated_checkouts_supported() -> bool {
    matches!(crate::isolation::profile(), Ok(None))
}

pub(crate) fn create_isolated_checkout(project: &Path) -> Result<PathBuf, String> {
    // Check before Git probes, directories or branch creation: a failed worktree add can leave
    // a new branch even when the destination is inaccessible to the execution account.
    if !isolated_checkouts_supported() {
        return Err("Isolated Git checkouts are unavailable on this host. Use the current checkout instead.".to_owned());
    }
    let location = location(project)?;
    if !location.git
        || git_output(&location.checkout, &["rev-parse", "--is-inside-work-tree"]).as_deref()
            != Some("true")
    {
        return Err("isolated agents need a non-bare Git checkout".to_owned());
    }
    let resolved = identity(project)?;
    let suffix = new_id();
    let path = state_root()?
        .join("worktrees")
        .join(&resolved.id)
        .join(&suffix);
    fs::create_dir_all(path.parent().ok_or("invalid worktree path")?)
        .map_err(|error| format!("could not create worktree directory: {error}"))?;
    let branch = format!("verb/{}", &suffix[..12]);
    let result = crate::exec::user_git(&location.checkout)
        .ok_or("git was not found on an absolute PATH entry outside the project")?
        .args(["worktree", "add", "-b", &branch])
        .arg(&path)
        .arg("HEAD")
        .output()
        .map_err(|error| format!("could not start git worktree: {error}"))?;
    if !result.status.success() {
        return Err(format!(
            "git could not create an isolated worktree: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        ));
    }
    if identity(&path)?.id != resolved.id {
        return Err("the new checkout did not join the expected Verb project".to_owned());
    }
    Ok(path)
}

pub(crate) fn command(project: &Path, args: &[String], json: bool) -> Result<(), String> {
    match args {
        [] => show_status(project, json),
        [action] if action == "status" => show_status(project, json),
        [action] if action == "worktree" => {
            let path = create_isolated_checkout(project)?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({"projectId": identity(&path)?.id, "workspace": path,
                    "branch": crate::git_snapshot(&path).branch, "sourceEditsCopied": false})
                );
            } else {
                println!("Isolated workspace created at {}\nIt starts from committed HEAD; uncommitted edits in the source checkout were not copied.", path.display());
            }
            Ok(())
        }
        _ => Err("usage: verb project [status] [--json] | worktree".to_owned()),
    }
}

fn show_status(project: &Path, json: bool) -> Result<(), String> {
    let identity = identity(project)?;
    let git = crate::git_snapshot(project);
    if json {
        println!(
            "{}",
            serde_json::json!({"projectId": identity.id, "anchor": identity.anchor, "workspace": project,
                        "branch": git.branch, "gitStatus": git.status.as_str(), "changedFiles": git.changes_known.then_some(git.changed_files)})
        );
    } else {
        println!(
            "Verb project: {}\nPrimary checkout: {}\nThis workspace: {}\nBranch: {} · {}",
            identity.id,
            identity.anchor.display(),
            project.display(),
            git.branch_label(),
            git.change_summary(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    #[test]
    fn common_dir_is_reused_until_the_checkout_is_repointed() {
        let root = std::env::temp_dir().join(format!("verb-common-dir-{}", new_id()));
        let main = root.join("main");
        fs::create_dir_all(&main).unwrap();
        git(&main, &["init", "-q", "-b", "main"]);
        fs::write(main.join("a"), "a").unwrap();
        git(&main, &["add", "a"]);
        git(&main, &["commit", "-q", "-m", "a"]);
        let main = fs::canonicalize(&main).unwrap();
        assert_eq!(common_dir(&main).as_deref(), Some(".git"));
        if dot_git(&main).is_none() {
            return; // no birth time on this filesystem: nothing is cached
        }
        // Ordinary work rewrites files inside .git; the answer is still reused.
        let seen = dot_git(&main);
        fs::write(main.join("b"), "b").unwrap();
        git(&main, &["add", "b"]);
        assert_eq!(dot_git(&main), seen);

        let linked = root.join("linked");
        git(&main, &["worktree", "add", "-q", linked.to_str().unwrap()]);
        let linked = fs::canonicalize(&linked).unwrap();
        let common = common_dir(&linked).unwrap();
        assert_eq!(fs::canonicalize(&common).unwrap(), main.join(".git"));

        // A new repository in the same place is a new .git directory: asked again.
        fs::remove_dir_all(main.join(".git")).unwrap();
        git(&main, &["init", "-q", "-b", "main"]);
        assert_ne!(dot_git(&main), seen);
        assert_eq!(common_dir(&main).as_deref(), Some(".git"));
        // A re-pointed worktree file is a new identity too.
        let before = dot_git(&linked);
        fs::write(linked.join(".git"), "gitdir: /nowhere/at/all\n").unwrap();
        assert_ne!(dot_git(&linked), before);
        assert_eq!(common_dir(&linked), None);
        fs::remove_dir_all(root).unwrap();
    }
}
