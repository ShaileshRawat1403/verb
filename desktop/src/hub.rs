//! The project context hub: what the project is (the Project Brief), what is in it (the file tree),
//! and the context every agent shares (a Verb-maintained section of AGENTS.md and CLAUDE.md).
//!
//! Nudge, never force: nothing here is required. The hub suggests the next useful step and records
//! what it writes. Two safety lines:
//!
//! - The file tree and previews only cover files Git tracks or would track
//!   (`git ls-files --cached --others --exclude-standard`). Ignored files such as `.env` are never
//!   listed or readable, so the hub cannot become a way to leak secrets.
//! - Agent files are only ever changed between Verb's markers. Everything else in them belongs to the
//!   user and is preserved byte for byte.

use crate::specs;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

pub(crate) const BRIEF_PATH: &str = "docs/project/BRIEF.md";
const AGENT_FILES: [&str; 2] = ["AGENTS.md", "CLAUDE.md"];
const START: &str = "<!-- verb:context:start -->";
const END: &str = "<!-- verb:context:end -->";
const MAX_FILES: usize = 20_000;
const MAX_PREVIEW: u64 = 200 * 1024;

#[derive(Serialize, Debug)]
pub(crate) struct FileEntry {
    pub path: String,
    pub status: Option<String>,
}

#[derive(Serialize, Debug)]
pub(crate) struct FileList {
    pub files: Vec<FileEntry>,
    pub truncated: bool,
}

#[derive(Serialize, Debug)]
pub(crate) struct Preview {
    pub path: String,
    pub text: Option<String>,
    pub reason: Option<String>,
    pub bytes: u64,
}

#[derive(Serialize, Debug)]
pub(crate) struct BriefStatus {
    pub path: &'static str,
    pub exists: bool,
    pub sections: Vec<(String, bool)>,
}

#[derive(Serialize, Debug)]
pub(crate) struct AgentFile {
    pub name: &'static str,
    pub exists: bool,
    /// "missing", "no-section", "outdated" or "current".
    pub state: &'static str,
}

#[derive(Serialize, Debug)]
pub(crate) struct Nudge {
    pub id: &'static str,
    pub text: String,
    pub action: &'static str,
}

#[derive(Serialize, Debug)]
pub(crate) struct Hub {
    pub brief: BriefStatus,
    pub documents: Vec<String>,
    pub agent_files: Vec<AgentFile>,
    pub nudges: Vec<Nudge>,
}

fn git(project: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(project)
        .args(args)
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

fn visible_files(project: &Path) -> Result<Vec<String>, String> {
    let raw = git(
        project,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
    )?;
    let mut files: Vec<String> = raw
        .split('\0')
        .filter(|f| !f.is_empty())
        .map(str::to_owned)
        .collect();
    files.sort();
    files.dedup();
    Ok(files)
}

/// Every file Git tracks or would track, with its working-tree status.
pub(crate) fn files(project: &Path) -> Result<FileList, String> {
    let mut status: HashMap<String, String> = HashMap::new();
    for line in git(project, &["status", "--porcelain=v1", "-uall"])?.lines() {
        if line.len() > 3 {
            let path = line[3..].rsplit(" -> ").next().unwrap_or(&line[3..]);
            status.insert(
                path.trim_matches('"').to_owned(),
                line[..2].trim().to_owned(),
            );
        }
    }
    let all = visible_files(project)?;
    let truncated = all.len() > MAX_FILES;
    let files = all
        .into_iter()
        .take(MAX_FILES)
        .map(|path| FileEntry {
            status: status.get(&path).cloned(),
            path,
        })
        .collect();
    Ok(FileList { files, truncated })
}

fn safe_relative(path: &str) -> Result<PathBuf, String> {
    let p = Path::new(path);
    if path.is_empty()
        || p.is_absolute()
        || p.components().any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("invalid path".to_owned());
    }
    Ok(p.to_path_buf())
}

/// A read-only preview of one visible text file.
pub(crate) fn preview(project: &Path, path: &str) -> Result<Preview, String> {
    let relative = safe_relative(path)?;
    if !visible_files(project)?.iter().any(|f| f == path) {
        return Err(
            "Verb only shows files Git tracks or would track; this one is ignored or absent"
                .to_owned(),
        );
    }
    let full = project.join(&relative);
    // Symlinks could point outside the project; resolve and check.
    let canonical = full.canonicalize().map_err(|e| e.to_string())?;
    let root = project.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.starts_with(&root) {
        return Err("that file points outside the project".to_owned());
    }
    let bytes = fs::metadata(&canonical).map_err(|e| e.to_string())?.len();
    let unreadable = |reason: &str| Preview {
        path: path.to_owned(),
        text: None,
        reason: Some(reason.to_owned()),
        bytes,
    };
    if bytes > MAX_PREVIEW {
        return Ok(unreadable("too large to preview (over 200 KB)"));
    }
    let raw = fs::read(&canonical).map_err(|e| e.to_string())?;
    if raw.iter().take(8000).any(|b| *b == 0) {
        return Ok(unreadable("binary file"));
    }
    Ok(Preview {
        path: path.to_owned(),
        text: Some(String::from_utf8_lossy(&raw).into_owned()),
        reason: None,
        bytes,
    })
}

// ---------------------------------------------------------------------------------- the brief

const BRIEF_SECTIONS: [&str; 5] = [
    "Problem",
    "Who it is for",
    "Scope",
    "Constraints",
    "Glossary",
];

fn section_text(raw: &str, heading: &str) -> String {
    let marker = format!("## {heading}");
    let mut lines = raw.lines();
    if !lines.any(|l| l.trim_end() == marker) {
        return String::new();
    }
    lines
        .take_while(|l| !l.starts_with("## "))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

fn filled(text: &str) -> bool {
    let t = text.trim();
    // A section still holding its `_placeholder_` hint does not count as written.
    let placeholder = t.starts_with('_') && t.ends_with('_');
    !t.is_empty() && !placeholder
}

pub(crate) struct NewBrief {
    pub problem: String,
    pub users: String,
    pub scope: String,
    pub constraints: String,
    pub glossary: String,
}

pub(crate) fn create_brief(project: &Path, input: NewBrief, actor: &str) -> Result<(), String> {
    let path = project.join(BRIEF_PATH);
    if path.exists() {
        return Err(format!("{BRIEF_PATH} already exists; edit it directly"));
    }
    let name = project
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "this project".to_owned());
    let or_hint = |text: &str, hint: &str| {
        if text.trim().is_empty() {
            format!("_{hint}_")
        } else {
            text.trim().to_owned()
        }
    };
    let body = format!(
        "# Project Brief · {name}\n\n\
         A short, living description of the project. Specs link here, and Verb shares the summary with \
         every agent through AGENTS.md and CLAUDE.md.\n\n\
         ## Problem\n\n{}\n\n## Who it is for\n\n{}\n\n## Scope\n\n{}\n\n\
         ## Constraints\n\n{}\n\n## Glossary\n\n{}\n\n## Audit trail\n\n\
         <!-- Written by Verb: one line per change, never edited. -->\n",
        or_hint(&input.problem, "What problem does this project solve, and why now?"),
        or_hint(&input.users, "Who uses it, and what are they trying to get done?"),
        or_hint(&input.scope, "What is in scope, and what is deliberately not?"),
        or_hint(&input.constraints, "Deadlines, platforms, budgets, rules it must follow."),
        or_hint(&input.glossary, "Words with a specific meaning in this project."),
    );
    fs::create_dir_all(path.parent().expect("brief has a parent")).map_err(|e| e.to_string())?;
    fs::write(&path, body).map_err(|e| e.to_string())?;
    specs::record(&path, actor, "created the project brief")
}

fn brief_status(project: &Path) -> BriefStatus {
    let raw = fs::read_to_string(project.join(BRIEF_PATH)).ok();
    BriefStatus {
        path: BRIEF_PATH,
        exists: raw.is_some(),
        sections: BRIEF_SECTIONS
            .iter()
            .map(|h| {
                (
                    (*h).to_owned(),
                    raw.as_deref().is_some_and(|r| filled(&section_text(r, h))),
                )
            })
            .collect(),
    }
}

// ------------------------------------------------------------------------- agent context sync

fn first_paragraph(text: &str) -> String {
    let text = text.trim();
    let para = text.split("\n\n").next().unwrap_or(text);
    let one_line = para.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.len() > 400 {
        format!(
            "{}…",
            &one_line[..one_line
                .char_indices()
                .nth(400)
                .map_or(one_line.len(), |(i, _)| i)]
        )
    } else {
        one_line
    }
}

/// The section Verb maintains in AGENTS.md and CLAUDE.md. Deterministic: same inputs, same text, so
/// "outdated" means something changed rather than a timestamp moving.
pub(crate) fn context_section(project: &Path) -> String {
    let mut out = format!(
        "{START}\n## Project context (maintained by Verb)\n\n\
         This section is generated from the project's brief and specs. Edit those, not this; Verb\n\
         rewrites everything between the markers and leaves the rest of this file alone.\n"
    );
    if let Ok(raw) = fs::read_to_string(project.join(BRIEF_PATH)) {
        out.push_str(&format!("\n**Brief:** `{BRIEF_PATH}`\n\n"));
        for heading in BRIEF_SECTIONS {
            let text = section_text(&raw, heading);
            if filled(&text) {
                out.push_str(&format!("- **{heading}:** {}\n", first_paragraph(&text)));
            }
        }
    } else {
        out.push_str("\nNo project brief yet.\n");
    }
    let active: Vec<_> = specs::list(project)
        .into_iter()
        .filter(|s| s.stage != "ship")
        .collect();
    if !active.is_empty() {
        out.push_str("\n### Active specs\n\n");
        for spec in &active {
            let open: Vec<&str> = spec
                .criteria
                .iter()
                .filter(|c| !c.done)
                .map(|c| c.text.as_str())
                .collect();
            out.push_str(&format!(
                "- **{} · {}** (`{}`, stage: {}, {}/{} criteria proven){}\n",
                spec.id,
                spec.title,
                spec.file,
                spec.stage,
                spec.criteria_done,
                spec.criteria.len(),
                if open.is_empty() {
                    String::new()
                } else {
                    format!(". Open: {}", open.join("; "))
                }
            ));
        }
    }
    out.push_str(
        "\n### Working agreements\n\n\
         - Read the relevant spec before changing code, and stay within its Out of scope section.\n\
         - Start commit messages with `spec:NNN` when the work belongs to a spec.\n\
         - Never edit an `## Audit trail` section; Verb appends to it.\n",
    );
    out.push_str(END);
    out.push('\n');
    out
}

fn current_section(raw: &str) -> Option<&str> {
    let start = raw.find(START)?;
    let end = raw[start..].find(END)? + start + END.len();
    let end = if raw[end..].starts_with('\n') {
        end + 1
    } else {
        end
    };
    Some(&raw[start..end])
}

fn agent_file_state(project: &Path, name: &'static str, wanted: &str) -> AgentFile {
    match fs::read_to_string(project.join(name)) {
        Err(_) => AgentFile {
            name,
            exists: false,
            state: "missing",
        },
        Ok(raw) => AgentFile {
            name,
            exists: true,
            state: match current_section(&raw) {
                None => "no-section",
                Some(section) if section == wanted => "current",
                Some(_) => "outdated",
            },
        },
    }
}

/// Writes the Verb section into AGENTS.md and CLAUDE.md, creating them if needed. Returns the files
/// that changed; records the sync in the brief's audit trail when there is a brief.
pub(crate) fn sync_agent_files(project: &Path, actor: &str) -> Result<Vec<&'static str>, String> {
    let wanted = context_section(project);
    let mut changed = Vec::new();
    for name in AGENT_FILES {
        let path = project.join(name);
        let raw = fs::read_to_string(&path).unwrap_or_default();
        let next = match current_section(&raw) {
            Some(section) if section == wanted => continue,
            Some(section) => raw.replacen(section, &wanted, 1),
            None if raw.trim().is_empty() => {
                format!("# {}\n\n{wanted}", name.trim_end_matches(".md"))
            }
            None => format!("{}\n\n{wanted}", raw.trim_end()),
        };
        fs::write(&path, next).map_err(|e| format!("could not write {name}: {e}"))?;
        changed.push(name);
    }
    let brief = project.join(BRIEF_PATH);
    if !changed.is_empty() && brief.exists() {
        specs::record(
            &brief,
            actor,
            &format!("synced agent context into {}", changed.join(" and ")),
        )?;
    }
    Ok(changed)
}

// ------------------------------------------------------------------------------- the hub view

pub(crate) fn hub(project: &Path) -> Hub {
    let brief = brief_status(project);
    let wanted = context_section(project);
    let agent_files: Vec<AgentFile> = AGENT_FILES
        .iter()
        .map(|name| agent_file_state(project, name, &wanted))
        .collect();
    let mut documents: Vec<String> = fs::read_dir(project.join("docs/project"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.ends_with(".md")
                .then(|| format!("docs/project/{name}"))
        })
        .collect();
    documents.sort();
    let specs = specs::list(project);
    let mut nudges = Vec::new();
    if !brief.exists {
        nudges.push(Nudge {
            id: "write-brief",
            text: if specs.len() >= 2 {
                format!("{} specs and no project brief yet. A brief takes about two minutes and gives every agent the same picture.", specs.len())
            } else {
                "Start with a short project brief: what this is, who it is for, and what is in scope.".to_owned()
            },
            action: "write-brief",
        });
    } else if brief.sections.iter().any(|(_, ok)| !ok) {
        let missing: Vec<&str> = brief
            .sections
            .iter()
            .filter(|(_, ok)| !ok)
            .map(|(h, _)| h.as_str())
            .collect();
        nudges.push(Nudge {
            id: "fill-brief",
            text: format!("Your brief could say more about: {}.", missing.join(", ")),
            action: "open-brief",
        });
    }
    if agent_files.iter().any(|f| f.state != "current") {
        nudges.push(Nudge {
            id: "sync-agents",
            text: if agent_files.iter().all(|f| f.state == "missing") {
                "Agents don't share any project context yet. Sync to give Claude, Codex and others the same brief and specs.".to_owned()
            } else {
                "Agent context is out of date with your brief and specs.".to_owned()
            },
            action: "sync-agents",
        });
    }
    for spec in specs
        .iter()
        .filter(|s| s.criteria.is_empty() && s.stage != "ship")
    {
        nudges.push(Nudge {
            id: "spec-criteria",
            text: format!(
                "Spec {} has no acceptance criteria, so there is nothing to prove yet.",
                spec.id
            ),
            action: "open-spec",
        });
    }
    Hub {
        brief,
        documents,
        agent_files,
        nudges,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "verb-hub-{}-{}-{n}",
            std::process::id(),
            crate::now_millis()
        ));
        fs::create_dir_all(&dir).unwrap();
        for args in [
            vec!["init", "--quiet", "-b", "main"],
            vec!["config", "user.name", "Hub Tester"],
            vec!["config", "user.email", "hub@example.com"],
        ] {
            git(&dir, &args).unwrap();
        }
        fs::write(dir.join(".gitignore"), ".env\n").unwrap();
        fs::write(dir.join(".env"), "SECRET=hunter2\n").unwrap();
        fs::write(dir.join("README.md"), "# hub\n").unwrap();
        git(&dir, &["add", "-A"]).unwrap();
        git(&dir, &["commit", "--quiet", "-m", "init"]).unwrap();
        dir
    }

    #[test]
    fn ignored_files_are_neither_listed_nor_readable() {
        let project = repo();
        let list = files(&project).unwrap();
        let names: Vec<_> = list.files.iter().map(|f| f.path.as_str()).collect();
        assert!(names.contains(&"README.md"));
        assert!(names.contains(&".gitignore"));
        assert!(!names.contains(&".env"), "{names:?}");
        assert!(preview(&project, ".env").is_err());
        assert!(preview(&project, "../etc/passwd").is_err());
        assert!(preview(&project, "/etc/passwd").is_err());
        assert_eq!(
            preview(&project, "README.md").unwrap().text.as_deref(),
            Some("# hub\n")
        );
    }

    #[test]
    fn untracked_files_show_with_their_status() {
        let project = repo();
        fs::write(project.join("new.txt"), "x").unwrap();
        let list = files(&project).unwrap();
        let entry = list.files.iter().find(|f| f.path == "new.txt").unwrap();
        assert_eq!(entry.status.as_deref(), Some("??"));
    }

    #[test]
    fn binary_files_are_not_previewed() {
        let project = repo();
        fs::write(project.join("blob.bin"), [0u8, 1, 2, 3]).unwrap();
        let p = preview(&project, "blob.bin").unwrap();
        assert!(p.text.is_none());
        assert_eq!(p.reason.as_deref(), Some("binary file"));
    }

    #[test]
    fn the_hub_nudges_toward_a_brief_and_a_sync() {
        let project = repo();
        let ids: Vec<_> = hub(&project).nudges.iter().map(|n| n.id).collect();
        assert_eq!(ids, vec!["write-brief", "sync-agents"]);
        create_brief(
            &project,
            NewBrief {
                problem: "Shops lose returning customers.".into(),
                users: "Returning shoppers".into(),
                scope: String::new(),
                constraints: String::new(),
                glossary: String::new(),
            },
            "Hub Tester via test",
        )
        .unwrap();
        let h = hub(&project);
        assert!(h.brief.exists);
        assert_eq!(h.nudges[0].id, "fill-brief");
        assert!(h.nudges[0].text.contains("Scope, Constraints, Glossary"));
    }

    #[test]
    fn sync_only_touches_the_marked_section_and_is_idempotent() {
        let project = repo();
        fs::write(
            project.join("AGENTS.md"),
            "# My rules\n\nAlways use tabs.\n",
        )
        .unwrap();
        create_brief(
            &project,
            NewBrief {
                problem: "Shops lose returning customers.".into(),
                users: String::new(),
                scope: String::new(),
                constraints: String::new(),
                glossary: String::new(),
            },
            "A",
        )
        .unwrap();
        let changed = sync_agent_files(&project, "A via test").unwrap();
        assert_eq!(changed, vec!["AGENTS.md", "CLAUDE.md"]);
        let agents = fs::read_to_string(project.join("AGENTS.md")).unwrap();
        assert!(
            agents.starts_with("# My rules\n\nAlways use tabs.\n"),
            "user text preserved"
        );
        assert!(agents.contains("**Problem:** Shops lose returning customers."));
        assert!(
            sync_agent_files(&project, "A").unwrap().is_empty(),
            "second sync changes nothing"
        );
        assert!(hub(&project)
            .agent_files
            .iter()
            .all(|f| f.state == "current"));

        // A user edit outside the markers survives a later sync.
        fs::write(
            project.join("AGENTS.md"),
            agents.replace("Always use tabs.", "Always use spaces."),
        )
        .unwrap();
        specs::create(
            &project,
            specs::NewSpec {
                title: "Login".into(),
                problem: String::new(),
                users: String::new(),
                criteria: vec!["Works".into()],
                out_of_scope: String::new(),
            },
            "A",
        )
        .unwrap();
        assert_eq!(hub(&project).agent_files[0].state, "outdated");
        sync_agent_files(&project, "A").unwrap();
        let agents = fs::read_to_string(project.join("AGENTS.md")).unwrap();
        assert!(agents.contains("Always use spaces."));
        assert!(agents.contains("**001 · Login**"));
        assert_eq!(agents.matches(START).count(), 1);
        let brief = fs::read_to_string(project.join(BRIEF_PATH)).unwrap();
        assert!(brief.contains("synced agent context into AGENTS.md and CLAUDE.md"));
    }
}
