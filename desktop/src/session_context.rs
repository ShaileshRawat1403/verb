//! Durable facts about spec-bound terminals and their reported outcomes.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

pub(crate) const DELIVERY_BYTES: usize = 8192;
pub(crate) const DELIVERY_LIMIT: usize = 64;
const ENTRY_BYTES: usize = 4096;
const NOTE_BYTES: usize = 128 * 1024;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Assignment {
    pub spec_id: Option<String>,
    pub work_item_id: Option<String>,
    pub role: Option<crate::specs::AssignmentRole>,
    pub deliveries: Vec<Delivery>,
    pub binding_audit: Vec<String>,
    pub git_start: Option<GitState>,
    pub terminal_ended_at: Option<u128>,
    pub draft_path: Option<String>,
    pub planning_attention: Option<PlanningAttention>,
    pub planning_handled: Option<Handled>,
    pub waiting_handled: Option<Handled>,
    pub reported_outcome: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanningAttention {
    pub paths: Vec<String>,
    pub unavailable: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct Handled {
    pub actor: String,
    pub at: String,
}

fn publication_receipt(store: &Path, project: &Path, path: &str) -> PathBuf {
    let key = digest(format!("{}\0{path}", project.display()).as_bytes());
    store.join("planning-publications").join(key)
}

pub(crate) fn record_draft_publication(
    store: &Path,
    project: &Path,
    path: &str,
    revision: &str,
) -> Result<(), String> {
    let receipt = publication_receipt(store, project, path);
    let data = format!("{}\n{revision}", crate::now_millis());
    crate::fsutil::atomic_write(&receipt, data.as_bytes())
}

fn is_draft_publication(session: &crate::Session, path: &str) -> bool {
    let Ok(project) = session.project_id.canonicalize() else {
        return false;
    };
    let Ok(identity) = crate::project::identity(&project) else {
        return false;
    };
    let Ok(receipt) = fs::read_to_string(publication_receipt(&identity.store, &project, path))
    else {
        return false;
    };
    let Some((at, revision)) = receipt.split_once('\n') else {
        return false;
    };
    at.parse::<u128>().is_ok_and(|at| at > session.created_at)
        && read(&project.join(path), 200 * 1024)
            .ok()
            .flatten()
            .is_some_and(|text| digest(text.as_bytes()) == revision)
}

pub(crate) fn check_planning_changes(session: &mut crate::Session) {
    let assignment = &session.assignment;
    let Some(draft) = &assignment.draft_path else {
        return;
    };
    let changed = assignment
        .git_start
        .as_ref()
        .and_then(|start| start.changed_paths(&session.project_id));
    session.assignment.planning_attention = match changed {
        Some(mut paths) => {
            let published = draft.replace(".draft.md", ".md");
            paths.retain(|path| {
                path != draft && (path != &published || !is_draft_publication(session, path))
            });
            (!paths.is_empty()).then_some(PlanningAttention {
                paths,
                unavailable: false,
            })
        }
        None => Some(PlanningAttention {
            paths: Vec::new(),
            unavailable: true,
        }),
    };
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Delivery {
    pub at: String,
    pub trigger: String,
    pub spec_id: Option<String>,
    pub spec_revision: Option<String>,
    pub requirements: Vec<String>,
    pub goals: Vec<String>,
    pub note_count: usize,
    pub bytes: usize,
    pub omission: Option<String>,
    pub hash: String,
}

pub(crate) fn reported_outcome(text: &str) -> Option<String> {
    let lines: Vec<_> = text.lines().collect();
    let (index, rest) = lines.iter().enumerate().rev().find_map(|(index, line)| {
        let line = line.trim().trim_start_matches('#').trim().trim_matches('*');
        let (label, rest) = line.split_once(':').unwrap_or((line, ""));
        matches!(
            label.trim().to_ascii_lowercase().as_str(),
            "summary" | "outcome"
        )
        .then_some((index, rest.trim_matches('*').trim()))
    })?;
    let details = std::iter::once(rest)
        .chain(
            lines[index + 1..]
                .iter()
                .copied()
                .take_while(|line| !line.starts_with('#')),
        )
        .collect::<Vec<_>>()
        .join("\n");
    let redacted = crate::transcript::redact(details.trim());
    (!redacted.is_empty()).then(|| redacted.chars().take(1000).collect())
}

pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn read(path: &Path, limit: usize) -> Result<Option<String>, String> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    if !file
        .metadata()
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Err("Expected a regular file".into());
    }
    let mut text = String::new();
    file.take(limit as u64 + 1)
        .read_to_string(&mut text)
        .map_err(|error| error.to_string())?;
    if text.len() > limit {
        return Err("File exceeds the supported size".into());
    }
    Ok(Some(text))
}

fn context_path(store: &Path, hash: &str) -> Result<PathBuf, String> {
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Invalid context hash".into());
    }
    Ok(store.join(format!("delivered-context-{hash}.txt")))
}

pub(crate) fn store_delivery(store: &Path, text: &str, hash: &str) -> Result<(), String> {
    if text.len() > DELIVERY_BYTES || digest(text.as_bytes()) != hash {
        return Err("Invalid or oversized delivery".into());
    }
    let _lock = crate::workbench::lock_project(store)?;
    crate::fsutil::atomic_write(&context_path(store, hash)?, text.as_bytes())
}

pub(crate) fn delivered_text(store: &Path, delivery: &Delivery) -> Result<String, String> {
    let text = read(&context_path(store, &delivery.hash)?, DELIVERY_BYTES)?
        .ok_or("Delivered text is unavailable")?;
    if text.len() != delivery.bytes || digest(text.as_bytes()) != delivery.hash {
        return Err("Delivered text no longer matches its record".into());
    }
    Ok(text)
}

fn notes_path(project: &Path, spec_id: &str) -> Result<PathBuf, String> {
    if spec_id.is_empty() || !spec_id.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("Invalid spec ID".into());
    }
    let project = project.canonicalize().map_err(|error| error.to_string())?;
    let dir = project.join("specs/sessions");
    for path in [project.join("specs"), dir.clone()] {
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() => (),
            Ok(_) => return Err("Session notes require real directories".into()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(dir.join(format!("{spec_id}.md")))
}

pub(crate) fn notes(project: &Path, spec_id: &str) -> Result<Vec<String>, String> {
    let text = read(&notes_path(project, spec_id)?, NOTE_BYTES)?.unwrap_or_default();
    let mut entries: Vec<_> = text
        .split("\n<!-- Verb session: ")
        .skip(1)
        .filter_map(|entry| {
            entry
                .split_once(" -->\n")
                .map(|(_, note)| note.trim().to_owned())
        })
        .collect();
    entries.reverse();
    Ok(entries)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Note {
    pub outcome: String,
    pub files: String,
    pub checks: String,
}

pub(crate) fn save_note(
    project: &Path,
    store: &Path,
    session: &crate::Session,
    note: Note,
    observed_files: Option<&str>,
) -> Result<(), String> {
    if session.assignment.terminal_ended_at.is_none() || session.state == crate::SessionState::Live
    {
        return Err("Session notes can only be saved after the session ends".into());
    }
    if note.outcome.trim().is_empty() {
        return Err("Enter a reported outcome".into());
    }
    if note.outcome.len() + note.files.len() + note.checks.len() > ENTRY_BYTES {
        return Err("Session note exceeds 4096 bytes. Shorten it and try again".into());
    }
    let spec_id = session
        .assignment
        .spec_id
        .as_deref()
        .ok_or("This session has no spec")?;
    let _lock = crate::workbench::lock_project(store)?;
    let path = notes_path(project, spec_id)?;
    let mut previous = read(&path, NOTE_BYTES)?.unwrap_or_default();
    let marker = format!("<!-- Verb session: {} -->", session.id);
    if previous.lines().any(|line| line == marker) {
        return Err("A note already exists for this session".into());
    }
    let directory = path.parent().ok_or("Invalid session notes path")?;
    match fs::read_dir(directory) {
        Ok(entries) => {
            for entry in entries {
                let other = entry.map_err(|error| error.to_string())?.path();
                if other != path
                    && other
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .is_some_and(|id| {
                            !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_digit())
                        })
                    && other.extension().is_some_and(|ext| ext == "md")
                    && read(&other, NOTE_BYTES)?
                        .is_some_and(|text| text.lines().any(|line| line == marker))
                {
                    return Err("A note already exists for this session".into());
                }
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.to_string()),
    }
    let clean = |text: &str| {
        crate::transcript::redact(text)
            .replace(|ch: char| ch.is_control() && !ch.is_whitespace(), "")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .replace("<!--", "&lt;!--")
            .replace("-->", "--&gt;")
    };
    let files_label = if observed_files == Some(note.files.as_str()) {
        "observed by Verb"
    } else {
        "reported, unverified"
    };
    let entry = format!(
        "\n{marker}\n## {} · {} · {}\n\n- Outcome (reported, unverified): {}\n- Files ({files_label}): {}\n- Checks (reported, unverified): {}\n",
        crate::iso8601(crate::now_millis()),
        clean(session.display_agent()),
        session.assignment.role.map_or("Role unrecorded", |role| role.label()),
        clean(&note.outcome),
        clean(&note.files),
        clean(&note.checks),
    );
    if entry.len() > ENTRY_BYTES {
        return Err(
            "Session note exceeds 4096 bytes after formatting. Shorten it and try again".into(),
        );
    }
    if previous.len() + entry.len() > NOTE_BYTES {
        return Err("Session notes reached the 128 KiB cap. Existing notes were preserved".into());
    }
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    previous.push_str(&entry);
    crate::specs::write_atomic(&path, &previous)
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct GitState {
    head: String,
    dirty: std::collections::BTreeMap<String, Option<String>>,
}

fn git(project: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = crate::exec::git(project)?;
    let out = crate::exec::read_output(cmd.args(args)).ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

impl GitState {
    pub(crate) fn read(project: &Path) -> Option<Self> {
        let head = git(project, &["rev-parse", "--verify", "HEAD"])
            .or_else(|| git(project, &["symbolic-ref", "-q", "HEAD"]).map(|_| String::new()))?
            .trim()
            .to_owned();
        let status = git(
            project,
            &[
                "status",
                "--porcelain=v1",
                "-z",
                "--no-renames",
                "--untracked-files=all",
            ],
        )?;
        let mut dirty = std::collections::BTreeMap::new();
        for entry in status.split_terminator('\0') {
            let path = entry.get(3..)?;
            if path.len() > 1024 || dirty.len() >= 128 {
                return None;
            }
            let file = project.join(crate::hub::safe_relative(path).ok()?);
            let value = match fs::symlink_metadata(&file) {
                Ok(meta) if meta.is_symlink() => Some(digest(
                    fs::read_link(&file).ok()?.as_os_str().as_encoded_bytes(),
                )),
                Ok(meta) if meta.is_file() && meta.len() <= 1024 * 1024 => {
                    if file.canonicalize().ok()? != project.canonicalize().ok()?.join(path) {
                        return None;
                    }
                    let mut options = fs::OpenOptions::new();
                    options.read(true);
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::OpenOptionsExt;
                        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
                    }
                    let input = options.open(file).ok()?;
                    if !input.metadata().ok()?.is_file() {
                        return None;
                    }
                    let mut bytes = Vec::new();
                    input.take(1024 * 1024 + 1).read_to_end(&mut bytes).ok()?;
                    if bytes.len() > 1024 * 1024 {
                        return None;
                    }
                    Some(digest(&bytes))
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                _ => return None,
            };
            dirty.insert(
                path.to_owned(),
                Some(format!("{}:{}", &entry[..2], value.unwrap_or_default())),
            );
        }
        Some(Self { head, dirty })
    }

    pub(crate) fn changes(&self, project: &Path) -> Option<String> {
        let text = self.changed_paths(project)?.join("\n");
        (text.len() <= 1500).then_some(text)
    }

    fn changed_paths(&self, project: &Path) -> Option<Vec<String>> {
        let current = Self::read(project)?;
        let diff = if self.head == current.head {
            String::new()
        } else if self.head.is_empty() || current.head.is_empty() {
            let head = if self.head.is_empty() {
                &current.head
            } else {
                &self.head
            };
            git(project, &["ls-tree", "-r", "--name-only", "-z", head])?
        } else {
            git(
                project,
                &[
                    "diff",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--no-renames",
                    "--name-only",
                    "-z",
                    &self.head,
                    &current.head,
                    "--",
                ],
            )?
        };
        let mut paths: std::collections::BTreeSet<_> = diff.split_terminator('\0').collect();
        for path in self.dirty.keys().chain(current.dirty.keys()) {
            if self.dirty.get(path) != current.dirty.get(path) {
                paths.insert(path);
            }
        }
        Some(paths.into_iter().map(str::to_owned).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publication_receipt_matches_only_the_published_bytes() {
        let root = std::env::temp_dir().join(format!("verb-publication-{}", crate::new_id()));
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        let project = project.canonicalize().unwrap();
        let identity = crate::project::identity(&project).unwrap();
        let mut session = crate::Session::new(project.clone(), crate::Agent::Claude);
        session.created_at = 0;
        let path = "prd.md";
        fs::write(project.join(path), "published").unwrap();
        assert!(!is_draft_publication(&session, path));
        record_draft_publication(&identity.store, &project, path, &digest(b"published")).unwrap();
        assert!(is_draft_publication(&session, path));
        fs::write(project.join(path), "changed by agent").unwrap();
        assert!(!is_draft_publication(&session, path));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn preserves_notes_at_the_cap_and_reads_latest_first() {
        let root = std::env::temp_dir().join(format!("verb-session-note-{}", crate::new_id()));
        let project = root.join("project");
        let store = root.join("store");
        fs::create_dir_all(&project).unwrap();
        let mut session = crate::Session::new(project.clone(), crate::Agent::Claude);
        session.state = crate::SessionState::Recoverable;
        session.assignment.spec_id = Some("001".into());
        session.assignment.terminal_ended_at = Some(crate::now_millis());
        let draft = |outcome: &str| Note {
            outcome: outcome.into(),
            files: String::new(),
            checks: String::new(),
        };
        save_note(&project, &store, &session, draft("older outcome"), None).unwrap();
        session.id = crate::new_id();
        save_note(&project, &store, &session, draft("newer outcome"), None).unwrap();
        let entries = notes(&project, "001").unwrap();
        assert!(entries[0].contains("newer outcome"));
        assert!(entries[1].contains("older outcome"));
        session.assignment.spec_id = Some("002".into());
        assert!(save_note(
            &project,
            &store,
            &session,
            draft("duplicate on another spec"),
            None
        )
        .is_err());
        session.assignment.spec_id = Some("001".into());
        let path = notes_path(&project, "001").unwrap();
        let previous = fs::read(&path).unwrap();
        session.id = crate::new_id();
        assert!(save_note(
            &project,
            &store,
            &session,
            draft(&"a".repeat(ENTRY_BYTES)),
            None
        )
        .is_err());
        assert_eq!(fs::read(&path).unwrap(), previous);
        fs::write(&path, " ".repeat(NOTE_BYTES)).unwrap();
        assert!(save_note(&project, &store, &session, draft("another outcome"), None).is_err());
        assert_eq!(fs::metadata(&path).unwrap().len(), NOTE_BYTES as u64);
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod outcome_tests {
    #[test]
    fn prefill_uses_only_explicit_reports() {
        assert_eq!(super::reported_outcome("Working on it now."), None);
        assert_eq!(
            super::reported_outcome("## Summary\nUpdated the form.\n## Checks\nTests passed")
                .as_deref(),
            Some("Updated the form.")
        );
    }
}
