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
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub(crate) const BRIEF_PATH: &str = "docs/project/BRIEF.md";
const AGENT_FILES: [&str; 2] = ["AGENTS.md", "CLAUDE.md"];
const START: &str = "<!-- verb:context:start -->";
const END: &str = "<!-- verb:context:end -->";
const MAX_FILES: usize = 20_000;
const MAX_PREVIEW: u64 = 200 * 1024;

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct FileEntry {
    pub path: String,
    pub status: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct FileList {
    pub files: Vec<FileEntry>,
    pub truncated: bool,
}

#[derive(Deserialize, Serialize, Debug)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
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
    pub context_bytes: Option<usize>,
    pub context_preview: Option<String>,
    pub context_warning: Option<String>,
}

fn git(project: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = crate::exec::git(project).ok_or("could not prepare git")?;
    crate::exec::git_text(crate::exec::read_output(command.args(args)))
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
    if crate::isolation::external() {
        return crate::project_read::request(project, "files", &[]);
    }
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
    if crate::isolation::external() {
        return crate::project_read::request(project, "preview", &[path]);
    }
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

/// The text under `## heading` in the brief (for Ask Verb).
pub(crate) fn brief_section(raw: &str, heading: &str) -> String {
    section_text(raw, heading)
}

/// Whether a brief section has real content rather than its placeholder hint.
pub(crate) fn brief_section_filled(text: &str) -> bool {
    filled(text)
}

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

const CONTEXT_LIMIT: usize = 8192;

#[derive(Default)]
struct ContextParts {
    brief: Vec<String>,
    notes: Vec<String>,
    specs: Vec<String>,
    glossary: Option<String>,
    specific: Vec<String>,
}

impl ContextParts {
    fn render(&self) -> String {
        let mut out = format!("{START}\n## Project context (maintained by Verb)\n");
        for (heading, lines) in [
            ("Brief", &self.brief),
            ("Current spec", &self.specific),
            ("Specs in progress", &self.specs),
            ("Shared notes (newest first)", &self.notes),
        ] {
            if !lines.is_empty() {
                out.push_str(&format!("\n### {heading}\n\n{}\n", lines.join("\n")));
            }
        }
        if let Some(glossary) = &self.glossary {
            out.push_str(&format!("\n### Glossary\n\n{glossary}\n"));
        }
        out
    }

    fn bounded(mut self, limit: usize, command: &str) -> (String, bool) {
        let mut omitted = [0; 5];
        loop {
            let mut out = self.render();
            let omission = [
                "notes",
                "in-progress specs",
                "glossary sections",
                "brief sections",
                "spec sections",
            ]
            .into_iter()
            .zip(omitted)
            .filter(|(_, count)| *count > 0)
            .map(|(name, count)| format!("{count} {name}"))
            .collect::<Vec<_>>();
            if !omission.is_empty() {
                out.push_str(&format!(
                    "\nOmitted {}. Run `{command}` for the full context.\n",
                    omission.join(", ")
                ));
            }
            out.push_str(END);
            out.push('\n');
            if out.len() <= limit {
                return (out, omitted[3] > 0 || omitted[4] > 0);
            }
            if !self.notes.is_empty() {
                let mut excess = out.len() - limit;
                while let Some(note) = self.notes.pop() {
                    omitted[0] += 1;
                    excess = excess.saturating_sub(note.len() + 1);
                    if excess == 0 {
                        break;
                    }
                }
            } else if self.specs.len() > 10 {
                omitted[1] += self.specs.len() - 10;
                self.specs.truncate(10);
            } else if self.glossary.take().is_some() {
                omitted[2] += 1;
            } else if self.specs.pop().is_some() {
                omitted[1] += 1;
            } else if self.specific.pop().is_some() {
                omitted[4] += 1;
            } else if self.brief.pop().is_some() {
                omitted[3] += 1;
            } else {
                return (String::new(), true);
            }
        }
    }
}

fn context_text(value: &str) -> String {
    value
        .replace(START, "[Verb context start marker]")
        .replace(END, "[Verb context end marker]")
}

fn visible_context_file(
    project: &Path,
    path: &str,
    files: &std::collections::HashSet<String>,
) -> Option<String> {
    if !files.contains(path) {
        return None;
    }
    let root = project.canonicalize().ok()?;
    let relative = safe_relative(path).ok()?;
    let expected = root.join(relative);
    if expected.canonicalize().ok()? != expected {
        return None;
    }
    read_agent_file(project, path).ok().flatten()
}

fn context_parts(
    project: &Path,
    spec_id: Option<&str>,
    launch: bool,
) -> Result<ContextParts, String> {
    let paths = if crate::isolation::external() {
        let result = files(project)?;
        if result.truncated {
            return Err("The visible checkout file list is incomplete".into());
        }
        result.files.into_iter().map(|file| file.path).collect()
    } else {
        visible_files(project)?
    };
    let files = paths.into_iter().collect::<std::collections::HashSet<_>>();
    let mut parts = ContextParts::default();
    if let Some(raw) = visible_context_file(project, BRIEF_PATH, &files) {
        for heading in ["Problem", "Scope", "Constraints", "Who it is for"] {
            let text = section_text(&raw, heading);
            if filled(&text) {
                parts
                    .brief
                    .push(format!("**{heading}:** {}", context_text(&text)));
            }
        }
        let glossary = section_text(&raw, "Glossary");
        if filled(&glossary) {
            parts.glossary = Some(context_text(&glossary));
        }
    }
    let notes = crate::transcript::redact(&crate::workbench::shared_memory(project)?);
    let mut paragraphs = Vec::new();
    let mut current = Vec::new();
    for line in notes.lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                paragraphs.push(current.join("\n"));
                current.clear();
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        paragraphs.push(current.join("\n"));
    }
    parts.notes = paragraphs
        .into_iter()
        .rev()
        .map(|note| format!("- {}", context_text(&note).replace('\n', "\n  ")))
        .collect();

    let mut specs: Vec<_> = files
        .iter()
        .filter_map(|path| {
            let relative = Path::new(path);
            if relative.parent() != Some(Path::new("specs"))
                || relative
                    .extension()
                    .and_then(|extension| extension.to_str())
                    != Some("md")
            {
                return None;
            }
            let raw = visible_context_file(project, path, &files)?;
            specs::parse_contents(&project.join(relative), project, &raw)
        })
        .collect();
    if let Some(id) = spec_id {
        let mut matches = specs.iter().filter(|spec| spec.id == id);
        let spec = matches.next().ok_or("No visible spec with that ID")?;
        if matches.next().is_some() {
            return Err("Multiple visible specs have that ID".into());
        }
        parts.specific.push(format!(
            "Spec {} · {} (`{}`)",
            spec.id,
            context_text(&spec.title),
            spec.file
        ));
        if let Some(work_item) = &spec.work_item {
            if crate::work_items::valid_work_item_id(work_item) {
                let brd =
                    visible_context_file(project, &format!("work/{work_item}/brd.md"), &files);
                let prd =
                    visible_context_file(project, &format!("work/{work_item}/prd.md"), &files);
                parts.specific.extend(
                    crate::work_items::linked_context(spec, brd, prd)
                        .into_iter()
                        .map(|text| context_text(&text)),
                );
            }
        } else {
            parts.specific.push("No work item is linked.".into());
        }
        parts.specific.extend(
            spec.body
                .split("\n## ")
                .enumerate()
                .filter(|(index, section)| {
                    !launch
                        || !matches!(
                            section
                                .lines()
                                .next()
                                .unwrap_or("")
                                .trim_end()
                                .trim_start_matches("## "),
                            "Audit trail" | "Traceability"
                        )
                        || (*index == 0 && !section.starts_with("## "))
                })
                .map(|(_, section)| {
                    let section = context_text(section);
                    if section.starts_with('#') {
                        section
                    } else {
                        format!("## {section}")
                    }
                }),
        );
    } else {
        specs.retain(|spec| matches!(spec.stage.as_str(), "plan" | "build" | "verify" | "review"));
        specs.sort_by(|left, right| {
            let left_at = left
                .audit
                .iter()
                .map(|entry| entry.at.as_str())
                .max()
                .unwrap_or("");
            right
                .audit
                .iter()
                .map(|entry| entry.at.as_str())
                .max()
                .unwrap_or("")
                .cmp(left_at)
                .then_with(|| left.id.cmp(&right.id))
                .then_with(|| left.file.cmp(&right.file))
        });
        parts.specs = specs
            .iter()
            .map(|spec| {
                format!(
                    "- **{} · {}** (`{}`, {}) · {}",
                    spec.id,
                    context_text(&spec.title).replace('\n', " "),
                    spec.file,
                    spec.stage,
                    spec.work_item
                        .as_deref()
                        .filter(|id| crate::work_items::valid_work_item_id(id))
                        .unwrap_or("unlinked")
                )
            })
            .collect();
    }
    Ok(parts)
}

pub(crate) fn full_context(project: &Path, spec: Option<&str>) -> Result<String, String> {
    let mut out = context_parts(project, spec, false)?.render();
    out.push_str(END);
    out.push('\n');
    Ok(out)
}

pub(crate) fn spec_launch_context(
    project: &Path,
    id: &str,
    limit: usize,
) -> Result<String, String> {
    let command = format!("verb shared spec {id}");
    Ok(context_parts(project, Some(id), true)?
        .bounded(limit.min(CONTEXT_LIMIT), &command)
        .0)
}

fn context_section(project: &Path) -> Result<(String, bool), String> {
    Ok(context_parts(project, None, false)?.bounded(CONTEXT_LIMIT, "verb shared full"))
}

fn current_section(raw: &str) -> Result<Option<&str>, String> {
    match (
        raw.match_indices(START).collect::<Vec<_>>().as_slice(),
        raw.match_indices(END).collect::<Vec<_>>().as_slice(),
    ) {
        ([], []) => Ok(None),
        ([(start, _)], [(end, _)]) if end > start => {
            let end = end + END.len();
            Ok(Some(&raw[*start..end]))
        }
        _ => Err("Malformed or duplicate Verb context markers".into()),
    }
}

fn read_agent_file(project: &Path, name: &str) -> Result<Option<String>, String> {
    use std::io::Read;
    let path = project.join(name);
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Cannot read {name}: {error}")),
    };
    if !file
        .metadata()
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Err(format!("{name} must be a regular file"));
    }
    let mut raw = String::new();
    file.take(MAX_PREVIEW + 1)
        .read_to_string(&mut raw)
        .map_err(|error| error.to_string())?;
    if raw.len() as u64 > MAX_PREVIEW {
        return Err(format!("{name} exceeds 200 KiB"));
    }
    Ok(Some(raw))
}

pub(crate) fn checkout_context(project: &Path) -> Result<String, String> {
    let agents =
        read_agent_file(project, "AGENTS.md")?.ok_or("No published AGENTS.md in this checkout")?;
    let claude =
        read_agent_file(project, "CLAUDE.md")?.ok_or("No published CLAUDE.md in this checkout")?;
    let block = current_section(&agents)?.ok_or("No published Verb context in this checkout")?;
    if block.len() > CONTEXT_LIMIT || current_section(&claude)? != Some(block) {
        return Err(
            "Published context is inconsistent. Ask the owner to sync this checkout.".into(),
        );
    }
    Ok(format!("{block}\n"))
}

fn agent_file_state(project: &Path, name: &'static str, wanted: Option<&str>) -> AgentFile {
    let (exists, state, problem) = match read_agent_file(project, name) {
        Ok(None) => (false, "missing", None),
        Err(error) => (true, "outdated", Some(error)),
        Ok(Some(raw)) => match current_section(&raw) {
            Err(error) => (true, "outdated", Some(error)),
            Ok(None) => (true, "no-section", None),
            Ok(Some(section)) if Some(section) == wanted => (true, "current", None),
            Ok(Some(_)) => (true, "outdated", None),
        },
    };
    AgentFile {
        name,
        exists,
        state,
        problem,
    }
}

#[derive(Debug)]
pub(crate) struct AgentSync {
    pub changed: Vec<&'static str>,
    pub notice: String,
}

fn protect_agent_files_from_git(project: &Path) -> Result<String, String> {
    let tracked = git(
        project,
        &["ls-files", "-z", "--cached", "--", "AGENTS.md", "CLAUDE.md"],
    )?;
    let tracked: Vec<_> = tracked.split_terminator('\0').collect();
    if !tracked.is_empty() {
        return Ok(format!(
            "Git exclusions were left unchanged. Shared notes will be committed with {} if you commit its changes.",
            tracked.join(" and ")
        ));
    }

    let exclude = git(
        project,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "info/exclude",
        ],
    )?;
    let path = Path::new(exclude.trim_end_matches(['\r', '\n']));
    if !path.is_absolute() {
        return Err("Git did not provide an absolute info/exclude path".into());
    }
    let parent = path.parent().ok_or("Git info/exclude has no directory")?;
    if !fs::symlink_metadata(parent)
        .map_err(|error| error.to_string())?
        .is_dir()
    {
        return Err("Git info/exclude must be inside a real info directory".into());
    }
    let raw = read_agent_file(
        project,
        path.to_str().ok_or("Git info/exclude is not UTF-8")?,
    )?
    .unwrap_or_default();
    let mut next = raw.clone();
    let mut added = Vec::new();
    for name in AGENT_FILES {
        let pattern = format!("/{name}");
        let explicit = raw
            .lines()
            .rev()
            .find(|line| line.trim_start_matches('!').trim_start_matches('/') == name);
        if explicit.is_some_and(|line| !line.starts_with('!')) {
            continue;
        }
        if !next.is_empty() && !next.ends_with('\n') {
            next.push('\n');
        }
        next.push_str(&pattern);
        next.push('\n');
        added.push(name);
    }
    if added.is_empty() {
        return Ok("AGENTS.md and CLAUDE.md already have local Git exclusions.".into());
    }
    crate::fsutil::atomic_write(path, next.as_bytes())?;
    Ok(format!(
        "Added {} to local Git info/exclude.",
        added.join(" and ")
    ))
}

/// Publishes both files under the same project lock. Validate both marker pairs before writing.
pub(crate) fn sync_agent_files(project: &Path, actor: &str) -> Result<AgentSync, String> {
    let identity = crate::project::identity(project)?;
    let _lock = crate::workbench::lock_project(&identity.store)?;
    let (wanted, _) = context_section(project)?;
    let block = wanted.trim_end_matches('\n');
    let mut writes = Vec::new();
    for name in AGENT_FILES {
        let raw = read_agent_file(project, name)?.unwrap_or_default();
        let next = match current_section(&raw).map_err(|error| format!("{name}: {error}"))? {
            Some(section) if section == block => continue,
            Some(section) => raw.replacen(section, block, 1),
            None if raw.is_empty() => format!("# {}\n\n{wanted}", name.trim_end_matches(".md")),
            None => format!("{raw}\n\n{wanted}"),
        };
        if next.len() as u64 > MAX_PREVIEW {
            return Err(format!("{name} would exceed 200 KiB"));
        }
        writes.push((name, next));
    }
    let notice = protect_agent_files_from_git(project)?;
    let mut changed = Vec::new();
    for (name, next) in writes {
        specs::write_atomic(&project.join(name), &next)?;
        changed.push(name);
    }
    let brief = project.join(BRIEF_PATH);
    let local_brief = project
        .canonicalize()
        .ok()
        .zip(brief.canonicalize().ok())
        .is_some_and(|(root, target)| target == root.join(BRIEF_PATH) && target.is_file());
    if !changed.is_empty() && local_brief {
        specs::record(
            &brief,
            actor,
            &format!("synced agent context into {}", changed.join(" and ")),
        )?;
    }
    Ok(AgentSync { changed, notice })
}

// ------------------------------------------------------------------------------- the hub view

pub(crate) fn hub(project: &Path) -> Hub {
    let brief = brief_status(project);
    let context = context_section(project);
    let wanted = context
        .as_ref()
        .ok()
        .map(|(text, _)| text.trim_end_matches('\n'));
    let agent_files: Vec<AgentFile> = AGENT_FILES
        .iter()
        .map(|name| agent_file_state(project, name, wanted))
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
        context_bytes: context.as_ref().ok().map(|(text, _)| text.len()),
        context_preview: context.as_ref().ok().map(|(text, _)| text.clone()),
        context_warning: match context {
            Ok((_, true)) => {
                Some("Required context was omitted. Shorten the project brief.".into())
            }
            Err(error) => Some(error),
            Ok((_, false)) => None,
        },
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
        assert_eq!(changed.changed, vec!["AGENTS.md", "CLAUDE.md"]);
        let agents = fs::read_to_string(project.join("AGENTS.md")).unwrap();
        assert!(
            agents.starts_with("# My rules\n\nAlways use tabs.\n"),
            "user text preserved"
        );
        assert!(agents.contains("**Problem:** Shops lose returning customers."));
        assert!(
            sync_agent_files(&project, "A").unwrap().changed.is_empty(),
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
        specs::set_stage(&project, "001", "plan", "", "A", None, None).unwrap();
        assert_eq!(hub(&project).agent_files[0].state, "outdated");
        sync_agent_files(&project, "A").unwrap();
        let agents = fs::read_to_string(project.join("AGENTS.md")).unwrap();
        assert!(agents.contains("Always use spaces."));
        assert!(agents.contains("**001 · Login**"));
        assert_eq!(agents.matches(START).count(), 1);
        let brief = fs::read_to_string(project.join(BRIEF_PATH)).unwrap();
        assert!(brief.contains("synced agent context into AGENTS.md and CLAUDE.md"));
    }
    #[test]
    fn context_bound_drops_old_notes_and_preserves_section_boundaries() {
        let parts = ContextParts {
            brief: vec!["Problem stays".into()],
            notes: vec!["newest".into(), "old".repeat(4000)],
            ..ContextParts::default()
        };
        let (text, warning) = parts.bounded(CONTEXT_LIMIT, "verb shared full");
        assert!(!warning);
        assert!(text.len() <= CONTEXT_LIMIT);
        assert!(text.contains("Problem stays"));
        assert!(text.contains("newest"));
        assert!(text.contains("Omitted 1 notes"));
        assert!(text.contains("verb shared full"));
        assert!(text.ends_with(&format!("{END}\n")));

        let parts = ContextParts {
            brief: vec!["Problem stays".into(), "é".repeat(4500)],
            ..ContextParts::default()
        };
        let (text, warning) = parts.bounded(CONTEXT_LIMIT, "verb shared full");
        assert!(warning);
        assert!(text.len() <= CONTEXT_LIMIT);
        assert!(text.contains("Omitted 1 brief sections"));
        assert!(!text.contains('é'));
    }

    #[test]
    fn sync_preflights_both_files_and_checkout_reads_only_matching_blocks() {
        let project = repo();
        let original = format!("before\n{START}\nold\n{END}after");
        fs::write(project.join("AGENTS.md"), &original).unwrap();
        fs::write(
            project.join("CLAUDE.md"),
            format!("{START}\n{START}\n{END}"),
        )
        .unwrap();
        assert!(sync_agent_files(&project, "Owner")
            .unwrap_err()
            .contains("duplicate"));
        assert_eq!(
            fs::read_to_string(project.join("AGENTS.md")).unwrap(),
            original
        );
        fs::write(project.join("CLAUDE.md"), "owner instructions\n").unwrap();
        sync_agent_files(&project, "Owner").unwrap();
        assert!(fs::read_to_string(project.join("AGENTS.md"))
            .unwrap()
            .ends_with("<!-- verb:context:end -->after"));
        assert!(checkout_context(&project).unwrap().contains(START));
        fs::write(project.join("CLAUDE.md"), format!("{START}\nwrong\n{END}")).unwrap();
        assert!(checkout_context(&project).is_err());
        assert_eq!(hub(&project).agent_files[1].state, "outdated");
    }
    #[test]
    fn launch_keeps_the_brief_and_omits_internal_spec_sections() {
        let project = repo();
        fs::create_dir_all(project.join("docs/project")).unwrap();
        fs::write(project.join(BRIEF_PATH), "## Problem\nThe problem\n## Scope\nThe scope\n## Constraints\nThe constraints\n## Who it is for\nThe owner\n## Glossary\nTerms").unwrap();
        fs::create_dir(project.join("specs")).unwrap();
        fs::write(project.join("specs/001-example.md"), format!(
            "---\nid: 001\ntitle: Example\nstage: plan\n---\n# Example\n## Details\n{}\n## Audit trail\n{}\n## Traceability\n- 1 · private-key · R1\n",
            "Details ".repeat(500), "Audit entry ".repeat(500),
        )).unwrap();
        let launch = spec_launch_context(&project, "001", 600).unwrap();
        for phrase in ["The problem", "The scope", "The constraints", "The owner"] {
            assert!(launch.contains(phrase), "{launch}");
        }
        assert!(!launch.contains("Audit entry") && !launch.contains("private-key"));
        assert!(launch.contains("spec sections"));
    }
    #[test]
    fn sync_excludes_untracked_agent_files_and_warns_without_changing_git_if_tracked() {
        let project = repo();
        let exclude = project.join(".git/info/exclude");
        fs::write(&exclude, "# owner rules\n/custom").unwrap();
        let gitignore = fs::read(project.join(".gitignore")).ok();
        let first = sync_agent_files(&project, "Owner").unwrap();
        assert!(first.notice.contains("Added AGENTS.md and CLAUDE.md"));
        let local = fs::read_to_string(&exclude).unwrap();
        assert_eq!(local, "# owner rules\n/custom\n/AGENTS.md\n/CLAUDE.md\n");
        assert!(git(&project, &["check-ignore", "AGENTS.md", "CLAUDE.md"]).is_ok());
        let second = sync_agent_files(&project, "Owner").unwrap();
        assert!(second.notice.contains("already have local Git exclusions"));
        assert_eq!(fs::read_to_string(&exclude).unwrap(), local);

        git(&project, &["add", "-f", "AGENTS.md"]).unwrap();
        fs::write(&exclude, "# owner rules only\n").unwrap();
        let tracked = sync_agent_files(&project, "Owner").unwrap();
        assert!(tracked
            .notice
            .contains("Shared notes will be committed with AGENTS.md"));
        assert_eq!(
            fs::read_to_string(&exclude).unwrap(),
            "# owner rules only\n"
        );
        assert_eq!(fs::read(project.join(".gitignore")).ok(), gitignore);
        assert_eq!(
            git(&project, &["ls-files", "--", "AGENTS.md", "CLAUDE.md"])
                .unwrap()
                .trim(),
            "AGENTS.md"
        );
    }
}
