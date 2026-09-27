//! One Verb project can contain several Git worktrees. Native agent conversations still belong
//! to their actual checkout path; only Verb's memory, tasks, and fetch receipts share this ID.

use crate::fsutil::atomic_write;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
    let output = crate::exec::git(project)?
        .args(args)
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8(output.stdout).ok()?.trim().to_owned())
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
    let common = git_output(&checkout, &["rev-parse", "--git-common-dir"])
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

pub(crate) fn create_isolated_checkout(project: &Path) -> Result<PathBuf, String> {
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
                        "branch": git.branch, "changedFiles": git.changed_files})
        );
    } else {
        println!(
            "Verb project: {}\nPrimary checkout: {}\nThis workspace: {}\nBranch: {} · {} changed",
            identity.id,
            identity.anchor.display(),
            project.display(),
            git.branch.as_deref().unwrap_or("detached/unknown"),
            git.changed_files,
        );
    }
    Ok(())
}
