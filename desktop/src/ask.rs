//! Ask Verb v1: answers about the project from evidence Verb already holds, and cites it.
//!
//! No model is involved. A question is matched to one of a few intents, and the answer is assembled
//! from specs and their audit trails, the project brief, Git, and the live sessions. Every fact in
//! an answer carries a source the UI can open, and an unrecognised question gets an honest "here is
//! what I can answer from evidence" instead of a guess.

use crate::{hub, specs};
use serde::Serialize;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Serialize, Debug, Clone, PartialEq)]
pub(crate) struct Source {
    /// "spec", "file", "commit", "session" or "audit".
    pub kind: &'static str,
    pub label: String,
    /// What the UI opens: a spec id, a file path, a commit sha or a session id.
    pub target: String,
}

#[derive(Serialize, Debug, PartialEq)]
pub(crate) struct Answer {
    pub intent: &'static str,
    pub summary: String,
    pub points: Vec<String>,
    pub sources: Vec<Source>,
    pub suggestions: Vec<&'static str>,
}

pub(crate) struct LiveSession {
    pub id: String,
    pub agent: String,
}

#[derive(Debug, PartialEq)]
enum Intent {
    Remaining(Option<String>),
    Changes,
    Status,
    History(Option<String>),
    Project,
    Who,
    Unknown,
}

const SUGGESTIONS: [&str; 6] = [
    "Where are we?",
    "What's left?",
    "What changed today?",
    "Who is working on what?",
    "What is this project?",
    "History of spec 1",
];

fn spec_id(q: &str) -> Option<String> {
    // "spec 12", "spec:012", "#12", "012"
    let words: Vec<&str> = q
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let number = words
        .windows(2)
        .find(|w| w[0] == "spec" && w[1].chars().all(|c| c.is_ascii_digit()))
        .map(|w| w[1])
        .or_else(|| {
            words
                .iter()
                .copied()
                .find(|w| w.len() <= 4 && w.chars().all(|c| c.is_ascii_digit()))
        })?;
    number.parse::<u32>().ok().map(|n| format!("{n:03}"))
}

fn classify(question: &str) -> Intent {
    let q = question.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| q.contains(w));
    let id = spec_id(&q);
    if has(&[
        "why",
        "history",
        "audit",
        "skip",
        "who moved",
        "what happened",
    ]) && id.is_some()
    {
        return Intent::History(id);
    }
    if has(&[
        "left",
        "remaining",
        "todo",
        "to do",
        "unproven",
        "open criteria",
        "not done",
        "still need",
    ]) {
        return Intent::Remaining(id);
    }
    if has(&[
        "changed",
        "commit",
        "today",
        "yesterday",
        "recent",
        "diff",
        "unsaved",
    ]) {
        return Intent::Changes;
    }
    if has(&["who", "working on", "agent", "session", "running"]) {
        return Intent::Who;
    }
    if has(&[
        "what is this",
        "about",
        "project",
        "brief",
        "purpose",
        "scope",
    ]) && !has(&["status"])
    {
        return Intent::Project;
    }
    if has(&[
        "where are we",
        "status",
        "progress",
        "overview",
        "summary",
        "how are we",
        "state",
    ]) {
        return Intent::Status;
    }
    if let Some(id) = id {
        return Intent::Remaining(Some(id));
    }
    Intent::Unknown
}

fn spec_source(spec: &specs::Spec) -> Source {
    Source {
        kind: "spec",
        label: format!("Spec {} · {}", spec.id, spec.title),
        target: spec.id.clone(),
    }
}

fn git(project: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(project)
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn remaining(project: &Path, id: Option<String>) -> Answer {
    let all = specs::list(project);
    let chosen: Vec<&specs::Spec> = match &id {
        Some(id) => all.iter().filter(|s| &s.id == id).collect(),
        None => all.iter().filter(|s| s.stage != "ship").collect(),
    };
    if chosen.is_empty() {
        return Answer {
            intent: "remaining",
            summary: match id {
                Some(id) => format!("There is no spec {id}."),
                None => "There are no open specs.".to_owned(),
            },
            points: Vec::new(),
            sources: Vec::new(),
            suggestions: vec!["Where are we?"],
        };
    }
    let mut points = Vec::new();
    let mut sources = Vec::new();
    let mut open_total = 0;
    for spec in &chosen {
        let open: Vec<_> = spec.criteria.iter().filter(|c| !c.done).collect();
        open_total += open.len();
        if open.is_empty() {
            points.push(format!(
                "Spec {} · {}: all {} criteria proven (stage: {}).",
                spec.id,
                spec.title,
                spec.criteria.len(),
                spec.stage
            ));
        } else {
            for c in open {
                points.push(format!(
                    "Spec {} · criterion {}: {}",
                    spec.id,
                    c.index + 1,
                    c.text
                ));
            }
        }
        sources.push(spec_source(spec));
    }
    let summary = if open_total == 0 {
        "Nothing is left to prove on the open specs.".to_owned()
    } else {
        format!(
            "{open_total} acceptance {} still to prove across {} spec{}.",
            if open_total == 1 {
                "criterion"
            } else {
                "criteria"
            },
            chosen.len(),
            if chosen.len() == 1 { "" } else { "s" }
        )
    };
    Answer {
        intent: "remaining",
        summary,
        points,
        sources,
        suggestions: vec!["What changed today?"],
    }
}

fn changes(project: &Path) -> Answer {
    let log = git(
        project,
        &["log", "--since=midnight", "--format=%h%x1f%s%x1f%an"],
    )
    .unwrap_or_default();
    let (log, window) = if log.trim().is_empty() {
        (
            git(project, &["log", "-5", "--format=%h%x1f%s%x1f%an"]).unwrap_or_default(),
            "recent",
        )
    } else {
        (log, "today")
    };
    let mut points = Vec::new();
    let mut sources = Vec::new();
    for line in log.lines() {
        let mut f = line.split('\u{1f}');
        if let (Some(sha), Some(subject), Some(author)) = (f.next(), f.next(), f.next()) {
            points.push(format!("{sha} {subject} ({author})"));
            sources.push(Source {
                kind: "commit",
                label: format!("Commit {sha}"),
                target: sha.to_owned(),
            });
        }
    }
    let unsaved = specs::git_summary(project)
        .map(|g| g.changes)
        .unwrap_or_default();
    for change in unsaved.iter().take(10) {
        sources.push(Source {
            kind: "file",
            label: change.path.clone(),
            target: change.path.clone(),
        });
    }
    let commits = points.len();
    if !unsaved.is_empty() {
        points.push(format!(
            "{} unsaved change{} not yet committed: {}{}",
            unsaved.len(),
            if unsaved.len() == 1 { "" } else { "s" },
            unsaved
                .iter()
                .take(5)
                .map(|c| c.path.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            if unsaved.len() > 5 { ", …" } else { "" }
        ));
    }
    let summary = match (window, commits, unsaved.len()) {
        (_, 0, 0) => "No commits yet and no unsaved changes.".to_owned(),
        ("today", n, u) => format!(
            "{n} commit{} today{}.",
            if n == 1 { "" } else { "s" },
            unsaved_tail(u)
        ),
        (_, n, u) => format!(
            "Nothing committed today. Here {} the last {n} commit{}{}.",
            if n == 1 { "is" } else { "are" },
            if n == 1 { "" } else { "s" },
            unsaved_tail(u)
        ),
    };
    Answer {
        intent: "changes",
        summary,
        points,
        sources,
        suggestions: vec!["What's left?", "Where are we?"],
    }
}

fn unsaved_tail(n: usize) -> String {
    match n {
        0 => String::new(),
        1 => ", plus 1 unsaved change".to_owned(),
        n => format!(", plus {n} unsaved changes"),
    }
}

fn status(project: &Path) -> Answer {
    let all = specs::list(project);
    let mut points = Vec::new();
    let mut sources = Vec::new();
    for stage in specs::STAGES {
        let here: Vec<_> = all.iter().filter(|s| s.stage == stage).collect();
        if !here.is_empty() {
            points.push(format!(
                "{}: {}",
                stage[..1].to_uppercase() + &stage[1..],
                here.iter()
                    .map(|s| format!("{} {}", s.id, s.title))
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
            sources.extend(here.iter().map(|s| spec_source(s)));
        }
    }
    let open: usize = all
        .iter()
        .filter(|s| s.stage != "ship")
        .map(|s| s.criteria.len() - s.criteria_done)
        .sum();
    let git = specs::git_summary(project).ok();
    if let Some(g) = &git {
        points.push(format!(
            "Git: on {}, {} unsaved change{}{}.",
            g.branch.as_deref().unwrap_or("a detached commit"),
            g.changes.len(),
            if g.changes.len() == 1 { "" } else { "s" },
            if g.ahead > 0 {
                format!(", {} commit(s) to push", g.ahead)
            } else {
                String::new()
            }
        ));
    }
    let summary = if all.is_empty() {
        "No specs yet. Start one to give the work a shape.".to_owned()
    } else {
        format!(
            "{} spec{}, {} acceptance criteria still to prove.",
            all.len(),
            if all.len() == 1 { "" } else { "s" },
            open
        )
    };
    Answer {
        intent: "status",
        summary,
        points,
        sources,
        suggestions: vec!["What's left?", "What changed today?"],
    }
}

fn history(project: &Path, id: Option<String>) -> Answer {
    let Some(id) = id else {
        return Answer {
            intent: "history",
            summary: "Which spec? For example: \"history of spec 1\".".to_owned(),
            points: Vec::new(),
            sources: Vec::new(),
            suggestions: vec!["Where are we?"],
        };
    };
    match specs::find(project, &id) {
        Err(_) => Answer {
            intent: "history",
            summary: format!("There is no spec {id}."),
            points: Vec::new(),
            sources: Vec::new(),
            suggestions: vec!["Where are we?"],
        },
        Ok((_, spec)) => {
            let warnings: Vec<_> = spec
                .audit
                .iter()
                .filter(|a| a.action.contains("warning"))
                .collect();
            let points = spec
                .audit
                .iter()
                .map(|a| {
                    format!(
                        "{} · {} · {}",
                        a.at.replace('T', " ").trim_end_matches('Z'),
                        a.actor,
                        a.action
                    )
                })
                .collect();
            let summary = if warnings.is_empty() {
                format!(
                    "Spec {} has {} recorded step{}, none with warnings. It is at {}.",
                    spec.id,
                    spec.audit.len(),
                    if spec.audit.len() == 1 { "" } else { "s" },
                    spec.stage
                )
            } else {
                format!(
                    "Spec {} moved on with warnings {} time{}: {}.",
                    spec.id,
                    warnings.len(),
                    if warnings.len() == 1 { "" } else { "s" },
                    warnings
                        .iter()
                        .map(|w| w.action.as_str())
                        .collect::<Vec<_>>()
                        .join("; ")
                )
            };
            Answer {
                intent: "history",
                summary,
                points,
                sources: vec![
                    spec_source(&spec),
                    Source {
                        kind: "file",
                        label: spec.file.clone(),
                        target: spec.file.clone(),
                    },
                ],
                suggestions: vec!["What's left?"],
            }
        }
    }
}

fn project_answer(project: &Path) -> Answer {
    match fs::read_to_string(project.join(hub::BRIEF_PATH)) {
        Err(_) => Answer {
            intent: "project",
            summary: "There is no project brief yet, so Verb has nothing to quote. Write one in the Project view; it takes about two minutes.".to_owned(),
            points: Vec::new(),
            sources: Vec::new(),
            suggestions: vec!["Where are we?"],
        },
        Ok(raw) => {
            let points: Vec<String> = ["Problem", "Who it is for", "Scope", "Constraints"]
                .iter()
                .filter_map(|h| {
                    let text = hub::brief_section(&raw, h);
                    hub::brief_section_filled(&text).then(|| format!("{h}: {}", text.split_whitespace().collect::<Vec<_>>().join(" ")))
                })
                .collect();
            Answer {
                intent: "project",
                summary: if points.is_empty() {
                    "The project brief exists but its sections are still empty.".to_owned()
                } else {
                    "From the project brief:".to_owned()
                },
                points,
                sources: vec![Source { kind: "file", label: "Project brief".to_owned(), target: hub::BRIEF_PATH.to_owned() }],
                suggestions: vec!["Where are we?", "What's left?"],
            }
        }
    }
}

fn who(project: &Path, live: &[LiveSession]) -> Answer {
    if live.is_empty() {
        return Answer {
            intent: "who",
            summary: "Nothing is running right now.".to_owned(),
            points: Vec::new(),
            sources: Vec::new(),
            suggestions: vec!["Where are we?"],
        };
    }
    let all = specs::list(project);
    let mut points = Vec::new();
    let mut sources = Vec::new();
    for session in live {
        let short = &session.id[..session.id.len().min(8)];
        // Specs record agents started on them, with the session id, in their audit trail.
        let spec = all.iter().find(|s| {
            s.audit
                .iter()
                .any(|a| a.action.contains(&format!("(session {short})")))
        });
        points.push(match spec {
            Some(spec) => format!(
                "{} ({short}) is working on spec {} · {}",
                session.agent, spec.id, spec.title
            ),
            None => format!("{} ({short}) is not linked to a spec", session.agent),
        });
        sources.push(Source {
            kind: "session",
            label: format!("{} {short}", session.agent),
            target: session.id.clone(),
        });
        if let Some(spec) = spec {
            sources.push(spec_source(spec));
        }
    }
    Answer {
        intent: "who",
        summary: format!(
            "{} session{} running.",
            live.len(),
            if live.len() == 1 { "" } else { "s" }
        ),
        points,
        sources,
        suggestions: vec!["What's left?"],
    }
}

pub(crate) fn answer(project: &Path, question: &str, live: &[LiveSession]) -> Answer {
    match classify(question) {
        Intent::Remaining(id) => remaining(project, id),
        Intent::Changes => changes(project),
        Intent::Status => status(project),
        Intent::History(id) => history(project, id),
        Intent::Project => project_answer(project),
        Intent::Who => who(project, live),
        Intent::Unknown => Answer {
            intent: "unknown",
            summary: "I only answer from evidence in this project, and I can't match that question to any. Try one of these:".to_owned(),
            points: Vec::new(),
            sources: Vec::new(),
            suggestions: SUGGESTIONS.to_vec(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn repo() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "verb-ask-{}-{}-{n}",
            std::process::id(),
            crate::now_millis()
        ));
        fs::create_dir_all(&dir).unwrap();
        for args in [
            vec!["init", "--quiet", "-b", "main"],
            vec!["config", "user.name", "Asker"],
            vec!["config", "user.email", "ask@example.com"],
            vec!["commit", "--quiet", "--allow-empty", "-m", "initial"],
        ] {
            Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(&args)
                .output()
                .unwrap();
        }
        specs::create(
            &dir,
            specs::NewSpec {
                title: "Login".into(),
                problem: String::new(),
                users: String::new(),
                criteria: vec!["Link arrives".into(), "Bad email rejected".into()],
                out_of_scope: String::new(),
            },
            "Asker via test",
        )
        .unwrap();
        dir
    }

    #[test]
    fn questions_map_to_intents() {
        assert_eq!(
            classify("What's left on spec 12?"),
            Intent::Remaining(Some("012".into()))
        );
        assert_eq!(classify("what's left"), Intent::Remaining(None));
        assert_eq!(classify("What changed today?"), Intent::Changes);
        assert_eq!(classify("where are we"), Intent::Status);
        assert_eq!(
            classify("why did spec:3 skip review"),
            Intent::History(Some("003".into()))
        );
        assert_eq!(classify("What is this project?"), Intent::Project);
        assert_eq!(classify("who is working on what"), Intent::Who);
        assert_eq!(classify("compose me a sonnet"), Intent::Unknown);
    }

    #[test]
    fn remaining_lists_open_criteria_and_cites_the_spec() {
        let p = repo();
        specs::set_criterion(&p, "001", 0, true, "seen", "A").unwrap();
        let a = answer(&p, "what's left on spec 1", &[]);
        assert_eq!(
            a.summary,
            "1 acceptance criterion still to prove across 1 spec."
        );
        assert_eq!(a.points, vec!["Spec 001 · criterion 2: Bad email rejected"]);
        assert_eq!(
            a.sources[0],
            Source {
                kind: "spec",
                label: "Spec 001 · Login".into(),
                target: "001".into()
            }
        );
        assert_eq!(
            answer(&p, "what's left on spec 9", &[]).summary,
            "There is no spec 009."
        );
    }

    #[test]
    fn changes_cite_commits_and_unsaved_files() {
        let p = repo();
        let a = answer(&p, "what changed today?", &[]);
        assert_eq!(a.summary, "1 commit today, plus 1 unsaved change.");
        assert!(a.sources.iter().any(|s| s.kind == "commit"));
        assert!(a
            .sources
            .iter()
            .any(|s| s.kind == "file" && s.target.starts_with("specs/")));
    }

    #[test]
    fn history_surfaces_recorded_warnings() {
        let p = repo();
        specs::set_stage(&p, "001", "build", "", "A via test").unwrap();
        let a = answer(&p, "why did spec 1 skip plan", &[]);
        assert!(
            a.summary.contains("moved on with warnings 1 time"),
            "{}",
            a.summary
        );
        assert!(a.summary.contains("skipped plan"));
        assert_eq!(a.points.len(), 2);
    }

    #[test]
    fn who_links_sessions_to_specs_through_the_audit_trail() {
        let p = repo();
        let (path, _) = specs::find(&p, "001").unwrap();
        specs::record(&path, "A", "started claude on this spec (session abcd1234)").unwrap();
        let live = [
            LiveSession {
                id: "abcd1234ffff".into(),
                agent: "Claude Code".into(),
            },
            LiveSession {
                id: "99999999".into(),
                agent: "Shell".into(),
            },
        ];
        let a = answer(&p, "who is working on what", &live);
        assert_eq!(
            a.points[0],
            "Claude Code (abcd1234) is working on spec 001 · Login"
        );
        assert_eq!(a.points[1], "Shell (99999999) is not linked to a spec");
    }

    #[test]
    fn unknown_questions_say_so_and_suggest() {
        let p = repo();
        let a = answer(&p, "compose me a sonnet", &[]);
        assert_eq!(a.intent, "unknown");
        assert!(a.sources.is_empty());
        assert_eq!(a.suggestions.len(), 6);
        assert!(answer(&p, "what is this project", &[])
            .summary
            .starts_with("There is no project brief yet"));
    }
}
