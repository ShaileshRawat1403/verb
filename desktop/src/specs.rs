//! Spec-driven development: the unit of work in the workbench.
//!
//! A spec is a Markdown file in `<project>/specs/`, committed with the code it describes, so it is
//! readable on GitHub, by agents, and by people who never open Verb. Verb adds three things on top of
//! the file: a stage (spec → plan → build → verify → review → ship), acceptance criteria that are
//! ticked off with evidence, and an audit trail.
//!
//! Auditability is the rule, not a feature. Every change Verb makes to a spec appends one line to
//! its `## Audit trail` section (when, who, what), and the file lives in Git, so the trail itself is
//! versioned. Verb never rewrites or deletes earlier audit lines. Gates warn and record instead of
//! blocking: moving on with criteria unproven is allowed, and the trail says so.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) const STAGES: [&str; 6] = ["spec", "plan", "build", "verify", "review", "ship"];
const CRITERIA_HEADING: &str = "## Acceptance criteria";
const AUDIT_HEADING: &str = "## Audit trail";

#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Criterion {
    pub index: usize,
    pub text: String,
    pub done: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct AuditEntry {
    pub at: String,
    pub actor: String,
    pub action: String,
}

#[derive(Serialize, Clone, Debug)]
pub(crate) struct Spec {
    pub id: String,
    pub title: String,
    pub stage: String,
    pub branch: String,
    pub created: String,
    pub file: String,
    pub criteria: Vec<Criterion>,
    pub criteria_done: usize,
    pub audit: Vec<AuditEntry>,
    pub body: String,
}

#[derive(Serialize, Debug)]
pub(crate) struct Change {
    pub code: String,
    pub path: String,
    /// Lines added and removed against the last commit; absent for binary files.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub added: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed: Option<u32>,
}

#[derive(Serialize, Debug)]
pub(crate) struct GitSummary {
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub changes: Vec<Change>,
    pub recent: Vec<Commit>,
}

#[derive(Serialize, Debug)]
pub(crate) struct Commit {
    pub sha: String,
    pub subject: String,
    pub author: String,
    pub when: String,
}

fn specs_dir(project: &Path) -> PathBuf {
    project.join("specs")
}

/// Who is acting, for the audit trail: the project's Git identity, plus the surface used.
pub(crate) fn actor(project: &Path, via: &str) -> String {
    let name = git(project, &["config", "user.name"])
        .ok()
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "unknown user".to_owned());
    format!("{name} via {via}")
}

// ------------------------------------------------------------------------------------- parsing

fn front_matter(raw: &str) -> Option<(Vec<(String, String)>, &str)> {
    let rest = raw.strip_prefix("---\n")?;
    let end = rest.find("\n---\n")?;
    let fields = rest[..end]
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned()))
        .collect();
    Some((fields, &rest[end + 5..]))
}

/// Lines of the section that starts with `heading`, up to the next `## ` heading.
fn section<'a>(body: &'a str, heading: &str) -> Vec<&'a str> {
    let mut lines = body.lines();
    if !lines.any(|line| line.trim_end() == heading) {
        return Vec::new();
    }
    lines.take_while(|line| !line.starts_with("## ")).collect()
}

fn parse_criteria(body: &str) -> Vec<Criterion> {
    section(body, CRITERIA_HEADING)
        .into_iter()
        .filter_map(|line| {
            let line = line.trim_start();
            let (done, text) = match line.strip_prefix("- [ ] ") {
                Some(text) => (false, text),
                None => (
                    true,
                    line.strip_prefix("- [x] ")
                        .or_else(|| line.strip_prefix("- [X] "))?,
                ),
            };
            Some((done, text.trim().to_owned()))
        })
        .enumerate()
        .map(|(index, (done, text))| Criterion { index, text, done })
        .collect()
}

fn parse_audit(body: &str) -> Vec<AuditEntry> {
    section(body, AUDIT_HEADING)
        .into_iter()
        .filter_map(|line| {
            let entry = line.trim_start().strip_prefix("- ")?;
            let mut parts = entry.splitn(3, " · ");
            Some(AuditEntry {
                at: parts.next()?.to_owned(),
                actor: parts.next()?.to_owned(),
                action: parts.next()?.to_owned(),
            })
        })
        .collect()
}

fn parse(file: &Path, project: &Path) -> Option<Spec> {
    let raw = fs::read_to_string(file).ok()?;
    let (fields, body) = front_matter(&raw)?;
    let field = |key: &str| {
        fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    let id = field("id");
    if id.is_empty() {
        return None;
    }
    let criteria = parse_criteria(body);
    Some(Spec {
        title: field("title"),
        stage: Some(field("stage"))
            .filter(|stage| STAGES.contains(&stage.as_str()))
            .unwrap_or_else(|| "spec".to_owned()),
        branch: field("branch"),
        created: field("created"),
        file: file
            .strip_prefix(project)
            .unwrap_or(file)
            .display()
            .to_string(),
        criteria_done: criteria.iter().filter(|c| c.done).count(),
        criteria,
        audit: parse_audit(body),
        body: body.trim_start().to_owned(),
        id,
    })
}

// --------------------------------------------------------------------------------------- reading

pub(crate) fn list(project: &Path) -> Vec<Spec> {
    let Ok(entries) = fs::read_dir(specs_dir(project)) else {
        return Vec::new();
    };
    let mut specs: Vec<Spec> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .filter_map(|path| parse(&path, project))
        .collect();
    specs.sort_by(|a, b| a.id.cmp(&b.id));
    specs
}

pub(crate) fn find(project: &Path, id: &str) -> Result<(PathBuf, Spec), String> {
    if !id.chars().all(|c| c.is_ascii_digit()) || id.is_empty() {
        return Err("invalid spec id".to_owned());
    }
    let dir = specs_dir(project);
    let entries = fs::read_dir(&dir).map_err(|_| "this project has no specs yet".to_owned())?;
    for path in entries.flatten().map(|entry| entry.path()) {
        if path.extension().is_some_and(|ext| ext == "md") {
            if let Some(spec) = parse(&path, project).filter(|spec| spec.id == id) {
                return Ok((path, spec));
            }
        }
    }
    Err(format!("no spec {id}"))
}

// --------------------------------------------------------------------------------------- writing

fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let mut out = out.trim_end_matches('-').to_owned();
    out.truncate(48);
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() {
        "spec".to_owned()
    } else {
        out
    }
}

fn audit_line(actor: &str, action: &str) -> String {
    let at = crate::iso8601(crate::now_millis());
    // One line per entry; the separators must not be forged by the content itself.
    let clean = |s: &str| s.replace(['\n', '\r'], " ").replace(" · ", " - ");
    format!("- {at} · {} · {}", clean(actor), clean(action))
}

fn write_atomic(path: &Path, contents: &str) -> Result<(), String> {
    let tmp = path.with_extension("md.verb-tmp");
    fs::write(&tmp, contents).map_err(|e| format!("could not write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("could not replace {}: {e}", path.display()))
}

fn set_field(raw: &str, key: &str, value: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_front = false;
    let mut seen_open = false;
    for line in raw.split_inclusive('\n') {
        if line.trim_end() == "---" {
            in_front = !seen_open;
            seen_open = true;
        } else if in_front && line.split_once(':').is_some_and(|(k, _)| k.trim() == key) {
            out.push_str(&format!("{key}: {value}\n"));
            continue;
        }
        out.push_str(line);
    }
    out
}

fn append_audit(raw: &str, line: &str) -> String {
    let mut out = raw.trim_end().to_owned();
    if !raw.lines().any(|l| l.trim_end() == AUDIT_HEADING) {
        out.push_str(&format!(
            "\n\n{AUDIT_HEADING}\n\n<!-- Written by Verb: one line per change, never edited. -->\n"
        ));
    }
    out.push('\n');
    out.push_str(line);
    out.push('\n');
    out
}

pub(crate) struct NewSpec {
    pub title: String,
    pub problem: String,
    pub users: String,
    pub criteria: Vec<String>,
    pub out_of_scope: String,
}

pub(crate) fn create(project: &Path, input: NewSpec, actor: &str) -> Result<Spec, String> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err("a spec needs a title".to_owned());
    }
    if title.len() > 120 {
        return Err("keep the title under 120 characters".to_owned());
    }
    let dir = specs_dir(project);
    fs::create_dir_all(&dir).map_err(|e| format!("could not create specs/: {e}"))?;
    let next = list(project)
        .iter()
        .filter_map(|spec| spec.id.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    let id = format!("{next:03}");
    let slug = slug(title);
    let branch = format!("spec/{id}-{slug}");
    let created = crate::iso8601(crate::now_millis())[..10].to_owned();
    let or_todo = |text: &str, hint: &str| {
        let text = text.trim();
        if text.is_empty() {
            format!("_{hint}_")
        } else {
            text.to_owned()
        }
    };
    let criteria: Vec<String> = input
        .criteria
        .iter()
        .map(|c| c.trim())
        .filter(|c| !c.is_empty())
        .map(|c| format!("- [ ] {}", c.replace('\n', " ")))
        .collect();
    let criteria = if criteria.is_empty() {
        "- [ ] _Describe one observable result that proves this works._".to_owned()
    } else {
        criteria.join("\n")
    };
    let contents = format!(
        "---\nid: {id}\ntitle: {title}\nstage: spec\nbranch: {branch}\ncreated: {created}\n---\n\n\
         # {id} · {title}\n\n\
         ## Problem\n\n{problem}\n\n\
         ## Who it is for\n\n{users}\n\n\
         {CRITERIA_HEADING}\n\n{criteria}\n\n\
         ## Out of scope\n\n{scope}\n\n\
         {AUDIT_HEADING}\n\n<!-- Written by Verb: one line per change, never edited. -->\n{audit}\n",
        problem = or_todo(&input.problem, "What is wrong or missing today, and why it matters."),
        users = or_todo(&input.users, "Who will use this, and what they are trying to do."),
        scope = or_todo(&input.out_of_scope, "What this deliberately does not do."),
        audit = audit_line(actor, &format!("created spec \"{title}\"")),
    );
    let path = dir.join(format!("{id}-{slug}.md"));
    if path.exists() {
        return Err(format!("{} already exists", path.display()));
    }
    write_atomic(&path, &contents)?;
    find(project, &id).map(|(_, spec)| spec)
}

/// Moves a spec to `stage`. Never blocks; returns the warnings it recorded in the audit trail.
pub(crate) fn set_stage(
    project: &Path,
    id: &str,
    stage: &str,
    note: &str,
    actor: &str,
) -> Result<(Spec, Vec<String>), String> {
    let target = STAGES
        .iter()
        .position(|s| *s == stage)
        .ok_or("unknown stage")?;
    let (path, spec) = find(project, id)?;
    let current = STAGES.iter().position(|s| *s == spec.stage).unwrap_or(0);
    if target == current {
        return Ok((spec, Vec::new()));
    }
    let mut warnings = Vec::new();
    if target > current + 1 {
        warnings.push(format!(
            "skipped {}",
            STAGES[current + 1..target].join(", ")
        ));
    }
    let open = spec.criteria.len() - spec.criteria_done;
    if target >= 4 && open > 0 {
        warnings.push(format!(
            "{open} of {} acceptance criteria not yet proven",
            spec.criteria.len()
        ));
    }
    let mut action = format!("stage {} → {stage}", spec.stage);
    if !warnings.is_empty() {
        action.push_str(&format!(" (warning: {})", warnings.join("; ")));
    }
    if !note.trim().is_empty() {
        action.push_str(&format!(" — {}", note.trim()));
    }
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let raw = set_field(&raw, "stage", stage);
    write_atomic(&path, &append_audit(&raw, &audit_line(actor, &action)))?;
    Ok((find(project, id)?.1, warnings))
}

/// Marks criterion `index` proven or reopened. Proving one asks for evidence, kept in the trail.
pub(crate) fn set_criterion(
    project: &Path,
    id: &str,
    index: usize,
    done: bool,
    evidence: &str,
    actor: &str,
) -> Result<Spec, String> {
    let (path, spec) = find(project, id)?;
    let criterion = spec
        .criteria
        .get(index)
        .ok_or("no such acceptance criterion")?;
    if criterion.done == done {
        return Ok(spec);
    }
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut out = String::with_capacity(raw.len());
    let mut seen = 0usize;
    let mut in_section = false;
    for line in raw.split_inclusive('\n') {
        let trimmed = line.trim_end();
        if trimmed.starts_with("## ") {
            in_section = trimmed == CRITERIA_HEADING;
        }
        let is_item = in_section
            && (trimmed.trim_start().starts_with("- [ ] ")
                || trimmed.trim_start().starts_with("- [x] ")
                || trimmed.trim_start().starts_with("- [X] "));
        if is_item {
            if seen == index {
                let mark = if done { "- [x] " } else { "- [ ] " };
                let indent = &line[..line.len() - line.trim_start().len()];
                let rest = &line.trim_start()[6..];
                out.push_str(indent);
                out.push_str(mark);
                out.push_str(rest);
                seen += 1;
                continue;
            }
            seen += 1;
        }
        out.push_str(line);
    }
    let mut action = if done {
        format!("criterion {} proven: {}", index + 1, criterion.text)
    } else {
        format!("criterion {} reopened: {}", index + 1, criterion.text)
    };
    if !evidence.trim().is_empty() {
        action.push_str(&format!(" — evidence: {}", evidence.trim()));
    }
    write_atomic(&path, &append_audit(&out, &audit_line(actor, &action)))?;
    Ok(find(project, id)?.1)
}

// ------------------------------------------------------------------------------------------- Git

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

pub(crate) fn git_summary(project: &Path) -> Result<GitSummary, String> {
    let status = git(project, &["status", "--porcelain=v1", "--branch", "-uall"])?;
    let mut summary = GitSummary {
        branch: None,
        upstream: None,
        ahead: 0,
        behind: 0,
        changes: Vec::new(),
        recent: Vec::new(),
    };
    for line in status.lines() {
        if let Some(head) = line.strip_prefix("## ") {
            let (names, counts) = head.split_once(" [").unwrap_or((head, ""));
            let (branch, upstream) = names.split_once("...").unwrap_or((names, ""));
            summary.branch = Some(branch.trim_start_matches("No commits yet on ").to_owned());
            summary.upstream = Some(upstream.to_owned()).filter(|u| !u.is_empty());
            for part in counts.trim_end_matches(']').split(", ") {
                if let Some(n) = part.strip_prefix("ahead ") {
                    summary.ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = part.strip_prefix("behind ") {
                    summary.behind = n.parse().unwrap_or(0);
                }
            }
        } else if line.len() > 3 {
            summary.changes.push(Change {
                code: line[..2].trim().to_owned(),
                path: line[3..].to_owned(),
                added: None,
                removed: None,
            });
        }
    }
    if !summary.changes.is_empty() {
        let counts = crate::diff::numstat(project, &summary.changes);
        for change in &mut summary.changes {
            if let Some(&(added, removed)) = counts.get(&change.path) {
                change.added = Some(added);
                change.removed = Some(removed);
            }
        }
    }
    if let Ok(log) = git(project, &["log", "-8", "--format=%h%x1f%s%x1f%an%x1f%cI"]) {
        summary.recent = log
            .lines()
            .filter_map(|line| {
                let mut f = line.split('\u{1f}');
                Some(Commit {
                    sha: f.next()?.to_owned(),
                    subject: f.next()?.to_owned(),
                    author: f.next()?.to_owned(),
                    when: f.next()?.to_owned(),
                })
            })
            .collect();
    }
    Ok(summary)
}

/// Commits every change in the working tree. When `spec_id` is given, the commit is recorded in
/// that spec's audit trail first, so the record and the code land in the same commit.
pub(crate) fn commit(
    project: &Path,
    message: &str,
    spec_id: Option<&str>,
    actor: &str,
) -> Result<String, String> {
    let message = message.trim();
    if message.is_empty() {
        return Err("describe what this commit changes".to_owned());
    }
    if git(project, &["status", "--porcelain"])?.trim().is_empty() {
        return Err("nothing to commit: there are no changes".to_owned());
    }
    let subject = message.lines().next().unwrap_or(message);
    if let Some(id) = spec_id.filter(|id| !id.is_empty()) {
        let (path, _) = find(project, id)?;
        let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        write_atomic(
            &path,
            &append_audit(&raw, &audit_line(actor, &format!("committed: {subject}"))),
        )?;
    }
    git(project, &["add", "--all"])?;
    git(project, &["commit", "--quiet", "-m", message])?;
    Ok(git(project, &["rev-parse", "--short", "HEAD"])?
        .trim()
        .to_owned())
}

/// Switches to the spec's branch, creating it from the current commit if needed. Refuses when
/// there are uncommitted changes, so no one's work moves branches by surprise.
pub(crate) fn switch_to_branch(project: &Path, id: &str, actor: &str) -> Result<String, String> {
    let (path, spec) = find(project, id)?;
    if spec.branch.is_empty() {
        return Err("this spec has no branch name".to_owned());
    }
    if !git(project, &["status", "--porcelain"])?.trim().is_empty() {
        return Err("commit or set aside your changes before switching branches".to_owned());
    }
    let exists = git(
        project,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{}", spec.branch),
        ],
    )
    .is_ok();
    if exists {
        git(project, &["switch", "--quiet", &spec.branch])?;
    } else {
        git(project, &["switch", "--quiet", "-c", &spec.branch])?;
    }
    // The spec file came along with the branch (it is committed), so record the switch there.
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let verb = if exists {
        "switched to"
    } else {
        "created and switched to"
    };
    write_atomic(
        &path,
        &append_audit(
            &raw,
            &audit_line(actor, &format!("{verb} branch {}", spec.branch)),
        ),
    )?;
    Ok(spec.branch)
}

/// Appends one audit line to a spec file, for events that happen outside this module.
pub(crate) fn record(path: &Path, actor: &str, action: &str) -> Result<(), String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    write_atomic(path, &append_audit(&raw, &audit_line(actor, action)))
}

/// The prompt given to an agent started on a spec: read it, work criterion by criterion, and stop
/// at the acceptance criteria rather than inventing scope.
pub(crate) fn agent_brief(spec: &Spec) -> String {
    let handoff = if spec.body.contains(HANDOFF_HEADING) {
        " Another agent worked on this before you: read the newest entry in its 'Handoff notes' \
         section first, and continue from there rather than starting over."
    } else {
        ""
    };
    // Without this an agent spends its first minutes in `git branch` and `git reflog` working out
    // where the work belongs (seen in a user test with Antigravity).
    let branch = Some(spec.branch.as_str())
        .filter(|b| !b.is_empty())
        .map(|b| {
            format!(
                " The work belongs on branch `{b}`: if you are not on it, switch first (`git switch {b}`, \
                 or `git switch -c {b}` if it does not exist yet)."
            )
        })
        .unwrap_or_default();
    format!(
        "Work on spec {id}, \"{title}\", described in {file}. Read that file first. Stay inside this \
         project folder: everything you need is here.{branch}{handoff} \
         Implement it so each acceptance criterion is met, one at a time, and stay within its \
         'Out of scope' section. Commit with messages that start with \"spec:{id}\". \
         Do not edit the spec's Audit trail or Handoff notes sections; Verb maintains them. When a \
         criterion is met, say which one and what proves it.",
        id = spec.id,
        title = spec.title,
        file = spec.file
    )
}

const HANDOFF_HEADING: &str = "## Handoff notes";

/// The agent that most recently started on a spec, from its audit trail.
pub(crate) fn last_agent(spec: &Spec) -> Option<String> {
    spec.audit.iter().rev().find_map(|a| {
        a.action
            .strip_prefix("started ")
            .and_then(|rest| rest.split(" on this spec").next())
            .map(str::to_owned)
    })
}

/// Writes a handoff note into the spec (newest first, under `## Handoff notes`) and records the
/// handoff in the audit trail. Returns the note. Starting the next agent is the caller's job, and
/// the previous agent is never stopped here: that stays the person's decision.
pub(crate) fn handoff(
    project: &Path,
    id: &str,
    to: &str,
    note: &str,
    actor: &str,
) -> Result<String, String> {
    let (path, spec) = find(project, id)?;
    let from = last_agent(&spec).unwrap_or_else(|| "nobody yet".to_owned());
    let at = crate::iso8601(crate::now_millis());
    let one_line = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut lines = vec![format!(
        "### {} · {from} → {to}",
        &at[..16].replace('T', " ")
    )];
    let open: Vec<_> = spec.criteria.iter().filter(|c| !c.done).collect();
    lines.push(if open.is_empty() {
        "- All acceptance criteria are proven.".to_owned()
    } else {
        format!(
            "- Still to prove: {}",
            open.iter()
                .map(|c| format!("{}. {}", c.index + 1, c.text))
                .collect::<Vec<_>>()
                .join("; ")
        )
    });
    let grep = format!("spec:{id}");
    if let Ok(log) = git(
        project,
        &[
            "log",
            "-5",
            "--fixed-strings",
            "--grep",
            &grep,
            "--format=%h %s",
        ],
    ) {
        let commits: Vec<&str> = log.lines().filter(|l| !l.trim().is_empty()).collect();
        if !commits.is_empty() {
            lines.push(format!(
                "- Recent commits for this spec: {}",
                commits.join("; ")
            ));
        }
    }
    if let Ok(summary) = git_summary(project) {
        let files: Vec<&str> = summary
            .changes
            .iter()
            .map(|c| c.path.as_str())
            .filter(|p| *p != spec.file)
            .take(12)
            .collect();
        if !files.is_empty() {
            lines.push(format!("- Uncommitted changes: {}", files.join(", ")));
        }
        if let Some(branch) = &summary.branch {
            lines.push(format!("- Branch: {branch}"));
        }
    }
    let note = one_line(note);
    if !note.is_empty() {
        lines.push(format!(
            "- Note from {}: {note}",
            actor.split(" via ").next().unwrap_or(actor)
        ));
    }
    let entry = lines.join("\n");

    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let next = if let Some(at_heading) = raw.find(HANDOFF_HEADING) {
        let insert = at_heading + HANDOFF_HEADING.len();
        format!("{}\n\n{entry}{}", &raw[..insert], &raw[insert..])
    } else if let Some(at_audit) = raw.find(AUDIT_HEADING) {
        format!(
            "{}{HANDOFF_HEADING}\n\n<!-- Written by Verb at each handoff, newest first. -->\n\n{entry}\n\n{}",
            &raw[..at_audit],
            &raw[at_audit..]
        )
    } else {
        format!("{}\n\n{HANDOFF_HEADING}\n\n{entry}\n", raw.trim_end())
    };
    let mut action = format!("handed off from {from} to {to}");
    if !note.is_empty() {
        action.push_str(&format!(" — {note}"));
    }
    write_atomic(&path, &append_audit(&next, &audit_line(actor, &action)))?;
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_repo() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "verb-specs-{}-{}-{n}",
            std::process::id(),
            crate::now_millis()
        ));
        fs::create_dir_all(&dir).unwrap();
        for args in [
            vec!["init", "--quiet", "-b", "main"],
            vec!["config", "user.name", "Test Person"],
            vec!["config", "user.email", "test@example.com"],
            vec!["commit", "--quiet", "--allow-empty", "-m", "init"],
        ] {
            git(&dir, &args).unwrap();
        }
        dir
    }

    fn new_spec(project: &Path) -> Spec {
        create(
            project,
            NewSpec {
                title: "Login with email!".to_owned(),
                problem: "People cannot sign in.".to_owned(),
                users: String::new(),
                criteria: vec![
                    "Email field shows".to_owned(),
                    "Bad email rejected".to_owned(),
                ],
                out_of_scope: String::new(),
            },
            "Test Person via test",
        )
        .unwrap()
    }

    #[test]
    fn create_writes_a_readable_spec_with_its_first_audit_line() {
        let project = temp_repo();
        let spec = new_spec(&project);
        assert_eq!(spec.id, "001");
        assert_eq!(spec.stage, "spec");
        assert_eq!(spec.branch, "spec/001-login-with-email");
        assert_eq!(spec.file, "specs/001-login-with-email.md");
        assert_eq!(spec.criteria.len(), 2);
        assert_eq!(spec.audit.len(), 1);
        assert_eq!(spec.audit[0].actor, "Test Person via test");
        assert!(spec.audit[0].action.starts_with("created spec"));
        assert_eq!(new_spec(&project).id, "002", "ids increase");
    }

    #[test]
    fn stage_changes_never_block_but_record_skips_and_unproven_criteria() {
        let project = temp_repo();
        new_spec(&project);
        let (spec, warnings) = set_stage(&project, "001", "review", "", "A via test").unwrap();
        assert_eq!(spec.stage, "review");
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        let last = spec.audit.last().unwrap();
        assert!(last.action.contains("spec → review"));
        assert!(last.action.contains("skipped plan, build, verify"));
        assert!(last
            .action
            .contains("2 of 2 acceptance criteria not yet proven"));
    }

    #[test]
    fn proving_a_criterion_ticks_it_and_keeps_the_evidence() {
        let project = temp_repo();
        new_spec(&project);
        let spec = set_criterion(
            &project,
            "001",
            1,
            true,
            "unit test rejects a@",
            "A via test",
        )
        .unwrap();
        assert!(!spec.criteria[0].done);
        assert!(spec.criteria[1].done);
        assert_eq!(spec.criteria_done, 1);
        let last = spec.audit.last().unwrap();
        assert_eq!(
            last.action,
            "criterion 2 proven: Bad email rejected — evidence: unit test rejects a@"
        );
        let raw = fs::read_to_string(project.join(&spec.file)).unwrap();
        assert!(raw.contains("- [x] Bad email rejected"));
        let spec = set_criterion(&project, "001", 1, false, "", "A via test").unwrap();
        assert!(!spec.criteria[1].done);
        assert_eq!(spec.audit.len(), 3, "audit lines are only ever appended");
    }

    #[test]
    fn audit_content_cannot_forge_extra_fields_or_lines() {
        let project = temp_repo();
        new_spec(&project);
        let spec = set_stage(
            &project,
            "001",
            "plan",
            "x · forged · y\n- fake line",
            "A via test",
        )
        .unwrap()
        .0;
        assert_eq!(spec.audit.len(), 2);
        assert_eq!(spec.audit[1].actor, "A via test");
    }

    #[test]
    fn commit_records_itself_in_the_spec_and_includes_that_record() {
        let project = temp_repo();
        new_spec(&project);
        let sha = commit(
            &project,
            "spec:001 add login spec",
            Some("001"),
            "A via test",
        )
        .unwrap();
        assert!(!sha.is_empty());
        let summary = git_summary(&project).unwrap();
        assert!(
            summary.changes.is_empty(),
            "the audit line was committed too: {:?}",
            summary.changes
        );
        assert_eq!(summary.recent[0].subject, "spec:001 add login spec");
        let spec = find(&project, "001").unwrap().1;
        assert!(spec
            .audit
            .last()
            .unwrap()
            .action
            .contains("committed: spec:001 add login spec"));
        assert!(
            commit(&project, "again", None, "A").is_err(),
            "nothing to commit"
        );
    }

    #[test]
    fn switching_branch_refuses_with_uncommitted_changes() {
        let project = temp_repo();
        new_spec(&project);
        assert!(switch_to_branch(&project, "001", "A").is_err());
        commit(&project, "spec:001 add", Some("001"), "A").unwrap();
        assert_eq!(
            switch_to_branch(&project, "001", "A").unwrap(),
            "spec/001-login-with-email"
        );
        assert_eq!(
            git_summary(&project).unwrap().branch.as_deref(),
            Some("spec/001-login-with-email")
        );
    }

    #[test]
    fn handoff_writes_a_note_newest_first_and_audits_it() {
        let project = temp_repo();
        new_spec(&project);
        let (path, _) = find(&project, "001").unwrap();
        record(
            &path,
            "A via test",
            "started claude on this spec (session aaaa1111)",
        )
        .unwrap();
        set_criterion(&project, "001", 0, true, "seen", "A").unwrap();
        commit(&project, "spec:001 first slice", None, "A").unwrap();
        fs::write(project.join("wip.txt"), "x").unwrap();

        let note = handoff(
            &project,
            "001",
            "codex",
            "tests  are\nflaky",
            "Test Person via test",
        )
        .unwrap();
        assert!(note.contains("claude → codex"), "{note}");
        assert!(note.contains("- Still to prove: 2. Bad email rejected"));
        assert!(note.contains("spec:001 first slice"));
        assert!(note.contains("- Uncommitted changes: wip.txt"));
        assert!(note.contains("- Note from Test Person: tests are flaky"));

        let spec = find(&project, "001").unwrap().1;
        assert_eq!(
            spec.audit.last().unwrap().action,
            "handed off from claude to codex — tests are flaky"
        );
        assert_eq!(
            spec.criteria.len(),
            2,
            "criteria still parse with a handoff section present"
        );
        assert!(agent_brief(&spec).contains("read the newest entry in its 'Handoff notes'"));

        record(&path, "A", "started codex on this spec (session bbbb2222)").unwrap();
        handoff(&project, "001", "claude", "", "A").unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        let first = raw.find("codex → claude").unwrap();
        let second = raw.find("claude → codex").unwrap();
        assert!(first < second, "newest note first");
        assert_eq!(raw.matches(HANDOFF_HEADING).count(), 1);
        assert!(raw.find(HANDOFF_HEADING).unwrap() < raw.find(AUDIT_HEADING).unwrap());
    }

    #[test]
    fn ids_must_be_digits() {
        let project = temp_repo();
        assert!(find(&project, "../etc").is_err());
    }
}
