//! Explicit repair of the prepared-index/ref-publication gap. Never replay a commit.
use super::text;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const LIMIT: u64 = 2 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema: u8,
    pid: u32,
    branch: String,
    before: String,
    commit: String,
    original_index: Option<String>,
    prepared_index: String,
    temp_name: String,
    lock_id: (u64, u64),
    audit: Option<(String, String, String)>,
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Status {
    pub version: String,
    pub message: String,
    pub can_repair: bool,
    pub commit: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    pub version: String,
}
fn receipt_path(index: &Path) -> PathBuf {
    index.with_file_name("verb-commit-recovery.json")
}
fn index_path(project: &Path) -> Result<PathBuf, String> {
    if crate::isolation::external() {
        return Err(
            "Git repair requires a native-local workspace. Use the terminal on this host.".into(),
        );
    }
    super::workspace_root(project)?;
    Ok(PathBuf::from(text(
        project,
        &["rev-parse", "--path-format=absolute", "--git-path", "index"],
        None,
    )?))
}
fn bytes(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > limit {
        return Err("Unexpected recovery file; inspect Git in the terminal.".into());
    }
    let mut out = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit + 1)
        .read_to_end(&mut out)
        .map_err(|e| e.to_string())?;
    if out.len() as u64 > limit {
        return Err("Recovery file exceeded its limit".into());
    }
    Ok(out)
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn hash(path: &Path) -> Result<Option<String>, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(Some(digest(&bytes(path, 64 * 1024 * 1024)?))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
#[cfg(unix)]
fn identity(path: &Path) -> Result<(u64, u64), String> {
    use std::os::unix::fs::MetadataExt;
    let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !m.is_file() {
        return Err("Unexpected Git lock type".into());
    }
    Ok((m.dev(), m.ino()))
}
#[cfg(not(unix))]
fn identity(_: &Path) -> Result<(u64, u64), String> {
    Err("Guided Git repair needs a Unix host".into())
}
fn sync_parent(index: &Path) -> Result<(), String> {
    fs::File::open(index.parent().ok_or("Git index parent missing")?)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())
}

pub(super) fn prepare(
    project: &Path,
    index: &Path,
    temp: &Path,
    snapshot: &super::Snapshot,
    commit: &str,
    audit: &Option<(PathBuf, String, String)>,
) -> Result<(), String> {
    let audit = audit
        .as_ref()
        .map(
            |(path, before, after)| -> Result<(String, String, String), String> {
                Ok((
                    path.strip_prefix(project)
                        .map_err(|_| "Spec is outside workspace")?
                        .to_str()
                        .ok_or("Spec path must be UTF-8")?
                        .to_owned(),
                    before.clone(),
                    after.clone(),
                ))
            },
        )
        .transpose()?;
    let record = Receipt {
        schema: 1,
        pid: std::process::id(),
        branch: snapshot.branch.clone(),
        before: snapshot.head.clone(),
        commit: commit.into(),
        original_index: hash(index)?,
        prepared_index: hash(temp)?.ok_or("Prepared index disappeared")?,
        temp_name: temp
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("Invalid index name")?
            .into(),
        lock_id: identity(&index.with_file_name("index.lock"))?,
        audit,
    };
    let data = serde_json::to_vec(&record).map_err(|e| e.to_string())?;
    if data.len() as u64 > LIMIT {
        return Err(
            "Spec is too large for durable guided commit recovery; use the terminal.".into(),
        );
    }
    fs::File::open(temp)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    sync_parent(index)?;
    let path = receipt_path(index);
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let reservation = options
        .open(&path)
        .map_err(|_| "An earlier commit needs review before another commit can run")?;
    drop(reservation);
    // Publishing the ref is forbidden unless this durable write succeeds.
    if let Err(error) = crate::fsutil::atomic_write(&path, &data) {
        let _ = fs::remove_file(path);
        return Err(error);
    }
    Ok(())
}
pub(super) fn complete(index: &Path) -> Result<(), String> {
    sync_parent(index)?;
    fs::remove_file(receipt_path(index)).map_err(|e| e.to_string())?;
    sync_parent(index)
}
fn load(index: &Path) -> Result<(Receipt, String), String> {
    let data = bytes(&receipt_path(index), LIMIT)?;
    let record: Receipt = serde_json::from_slice(&data)
        .map_err(|_| "Unreadable Git recovery record; inspect it in the terminal")?;
    let oid =
        |s: &str| (s.len() == 40 || s.len() == 64) && s.bytes().all(|c| c.is_ascii_hexdigit());
    if record.pid == 0
        || record.pid > i32::MAX as u32
        || record.schema != 1
        || !record.branch.starts_with("refs/heads/")
        || !oid(&record.commit)
        || (!record.before.is_empty() && !oid(&record.before))
        || !record.temp_name.starts_with("verb-commit-")
        || Path::new(&record.temp_name).components().count() != 1
    {
        return Err("Unsupported Git recovery identity; use the terminal".into());
    }
    Ok((record, digest(&data)))
}
fn current_ref(project: &Path, branch: &str) -> Result<String, String> {
    let rows = text(
        project,
        &[
            "for-each-ref",
            "--format=%(refname)%00%(objectname)",
            branch,
        ],
        None,
    )?;
    Ok(rows
        .lines()
        .find_map(|line| {
            line.split_once('\0')
                .filter(|(name, _)| *name == branch)
                .map(|(_, oid)| oid.to_owned())
        })
        .unwrap_or_default())
}
fn inspect(project: &Path, index: &Path, r: &Receipt) -> Result<bool, String> {
    if r.pid != std::process::id() {
        #[cfg(unix)]
        {
            if unsafe { libc::kill(r.pid as i32, 0) } == 0
                || std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
            {
                return Err("The original Verb process may still be running. Wait or inspect Git in the terminal.".into());
            }
        }
        #[cfg(not(unix))]
        {
            return Err(
                "Cannot establish that the original process ended; use the terminal".into(),
            );
        }
    }
    if text(project, &["symbolic-ref", "--quiet", "HEAD"], None)? != r.branch {
        return Err(
            "The active branch changed; inspect the interrupted commit in the terminal.".into(),
        );
    }
    let head = current_ref(project, &r.branch)?;
    let committed = head == r.commit;
    if !committed && head != r.before {
        return Err(
            "Git history changed since the interrupted operation. No repair was applied.".into(),
        );
    }
    let actual = hash(index)?;
    if actual != r.original_index && (!committed || actual.as_ref() != Some(&r.prepared_index)) {
        return Err(
            "The staging area changed. Repair will not overwrite it; use the terminal.".into(),
        );
    }
    let lock = index.with_file_name("index.lock");
    if fs::symlink_metadata(&lock).is_ok()
        && (identity(&lock)? != r.lock_id || hash(&lock)?.as_ref() != Some(&r.prepared_index))
    {
        return Err("Another Git lock is present. Repair will not remove or replace it.".into());
    }
    let temp = index.with_file_name(&r.temp_name);
    let prepared = hash(&temp)?;
    if prepared.is_some() && prepared.as_ref() != Some(&r.prepared_index) {
        return Err("The prepared index changed; inspect Git in the terminal.".into());
    }
    if actual.as_ref() != Some(&r.prepared_index) && prepared.is_none() {
        return Err(
            "The prepared recovery index is missing. Inspect Git history and status in the terminal."
                .into(),
        );
    }
    Ok(committed)
}
pub(crate) fn status(project: &Path) -> Result<Option<Status>, String> {
    let index = index_path(project)?;
    if fs::symlink_metadata(receipt_path(&index)).is_err() {
        return Ok(None);
    }
    let (record, version) = match load(&index) {
        Ok(r) => r,
        Err(message) => {
            return Ok(Some(Status {
                version: String::new(),
                message,
                can_repair: false,
                commit: None,
            }))
        }
    };
    if record.pid == std::process::id() && super::TRANSACTION.try_lock().is_err() {
        return Ok(Some(Status {
            version,
            message: "A Git transaction is active. Wait, then reopen the review.".into(),
            can_repair: false,
            commit: None,
        }));
    }
    match inspect(project, &index, &record) {
        Ok(true) => Ok(Some(Status { version, message: format!("Commit {} was recorded. Review and repair its staging state before continuing; do not resend the commit.", &record.commit[..8]), can_repair: true, commit: Some(record.commit) })),
        Ok(false) => Ok(Some(Status { version, message: "The interrupted attempt did not publish a commit. Repair its temporary state, then review your files again.".into(), can_repair: true, commit: None })),
        Err(message) => Ok(Some(Status { version, message, can_repair: false, commit: None })),
    }
}
pub(crate) fn repair(project: &Path, request: &Request) -> Result<String, String> {
    let index = index_path(project)?;
    let (record, version) = load(&index)?;
    if version != request.version {
        return Err("Recovery changed; close and reopen the review before repairing.".into());
    }
    let guard_path = index.with_file_name("verb-commit-repair.lock");
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let _guard_file = options
        .open(&guard_path)
        .map_err(|_| "Git recovery is already active or its lock needs terminal inspection")?;
    let _guard = RepairGuard {
        id: identity(&guard_path)?,
        path: guard_path,
    };
    let committed = inspect(project, &index, &record)?;
    let lock = index.with_file_name("index.lock");
    if fs::symlink_metadata(&lock).is_err() {
        let data =
            bytes(&index.with_file_name(&record.temp_name), 64 * 1024 * 1024).or_else(|error| {
                if hash(&index)?.as_ref() == Some(&record.prepared_index) {
                    bytes(&index, 64 * 1024 * 1024)
                } else {
                    Err(error)
                }
            })?;
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&lock)
            .map_err(|_| "Git became busy; reopen recovery")?;
        file.write_all(&data)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
    }
    let owned_lock = identity(&lock)?;
    let expected_head = if committed {
        &record.commit
    } else {
        &record.before
    };
    let actual = hash(&index)?;
    if current_ref(project, &record.branch)? != *expected_head
        || text(project, &["symbolic-ref", "--quiet", "HEAD"], None)? != record.branch
        || (actual != record.original_index
            && (!committed || actual.as_ref() != Some(&record.prepared_index)))
        || hash(&lock)?.as_ref() != Some(&record.prepared_index)
    {
        return Err(
            "Git changed during repair. No index was replaced; inspect its lock in the terminal."
                .into(),
        );
    }
    if committed && actual.as_ref() != Some(&record.prepared_index) {
        if identity(&lock)? != owned_lock {
            return Err("Git lock changed; nothing was replaced".into());
        }
        fs::rename(&lock, &index).map_err(|e| e.to_string())?;
        sync_parent(&index)?;
    } else if !committed {
        if let Some((path, before, after)) = &record.audit {
            super::file(project, path)?;
            let target = project.join(path);
            if fs::read_to_string(&target).ok().as_ref() == Some(after) {
                crate::specs::write_atomic(&target, before)?;
            }
        }
    }
    if fs::symlink_metadata(&lock).is_ok() {
        if identity(&lock)? != owned_lock || hash(&lock)?.as_ref() != Some(&record.prepared_index) {
            return Err("Git lock changed; leaving it untouched".into());
        }
        fs::remove_file(&lock).map_err(|e| e.to_string())?;
    }
    let temp = index.with_file_name(&record.temp_name);
    if temp.exists() {
        if hash(&temp)?.as_ref() != Some(&record.prepared_index) {
            return Err("Prepared index changed; leaving it untouched".into());
        }
        fs::remove_file(temp).map_err(|e| e.to_string())?;
    }
    complete(&index)?;
    Ok(if committed {
        format!(
            "Commit {} confirmed; staging repaired. No new commit was created.",
            &record.commit[..8]
        )
    } else {
        "Temporary commit state cleared. Your working files remain; reopen review before committing.".into()
    })
}

struct RepairGuard {
    id: (u64, u64),
    path: PathBuf,
}
impl Drop for RepairGuard {
    fn drop(&mut self) {
        if identity(&self.path).ok() == Some(self.id) {
            let _ = fs::remove_file(&self.path);
        }
    }
}
