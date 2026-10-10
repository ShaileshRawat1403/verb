//! `verb check`: every observed reason to be careful right now, in one place.
//!
//! It gathers the three newer observations -- repository state (C5), runtime versions against what
//! the project declares (C4) and distance from the last-known-good mark (C3) -- and prints them as
//! facts with the safe next step beside each. It interprets nothing and runs nothing.
//!
//! Output carries counts and version numbers only: no file names, branch names or paths. That keeps
//! `verb check --json` safe to hand to an assistant under the same rule as `verb context`.

use crate::gitstate::{self, Level, Warning};
use crate::good::{self, Distance, Mark};
use crate::runtime::{self, Fact, Verdict};
use std::path::Path;

pub(crate) struct Report {
    pub assembled_at: u128,
    /// `None` when there are no repository warnings to give: see `repository_status` for why.
    pub repository: Option<Vec<Warning>>,
    pub repository_status: &'static str,
    pub runtimes: Vec<Fact>,
    pub good: Option<(Mark, Distance)>,
}

pub(crate) fn assemble(project: &Path) -> Result<Report, String> {
    let (repository, repository_status) = match gitstate::observe(project) {
        gitstate::Reading::State(state) => (Some(state.warnings()), "read"),
        gitstate::Reading::NotRepository => (None, "notRepository"),
        gitstate::Reading::Unavailable => (None, "unavailable"),
    };
    let good = match good::load(project)? {
        Some(mark) => {
            let distance = good::distance(project, &mark, true);
            Some((mark, distance))
        }
        None => None,
    };
    Ok(Report {
        assembled_at: crate::now_millis(),
        repository,
        repository_status,
        runtimes: runtime::observe(project),
        good,
    })
}

impl Report {
    /// Runtime facts worth a warning: a declared runtime that is absent or the wrong version.
    pub(crate) fn runtime_problems(&self) -> impl Iterator<Item = &Fact> {
        self.runtimes
            .iter()
            .filter(|fact| matches!(fact.verdict, Verdict::Mismatch | Verdict::Missing))
    }

    /// Nothing observed that calls for care. Unknowns do not count against this: they are reported,
    /// but "Verb could not tell" is not "something is wrong".
    pub(crate) fn clear(&self) -> bool {
        self.repository.as_ref().is_none_or(Vec::is_empty) && self.runtime_problems().count() == 0
    }

    pub(crate) fn to_json(&self) -> String {
        let repository = match &self.repository {
            Some(warnings) => format!(
                "[{}]",
                warnings
                    .iter()
                    .map(Warning::to_json)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            None => "null".to_owned(),
        };
        let runtimes: Vec<String> = self.runtimes.iter().map(Fact::to_json).collect();
        let good = match &self.good {
            Some((mark, distance)) => format!(
                "{{\"mark\":{},\"distance\":{}}}",
                mark.to_json(),
                distance.to_json()
            ),
            None => "null".to_owned(),
        };
        format!(
            "{{\"schemaVersion\":1,\"assembledAt\":\"{}\",\"clear\":{},\"repositoryStatus\":\"{}\",\"repository\":{},\"runtimes\":[{}],\"lastKnownGood\":{}}}",
            crate::iso8601(self.assembled_at),
            self.clear(),
            self.repository_status,
            repository,
            runtimes.join(","),
            good
        )
    }

    pub(crate) fn to_text(&self) -> String {
        let mut lines = vec![format!(
            "Observed now ({})",
            crate::iso8601(self.assembled_at)
        )];

        lines.push(String::new());
        match &self.repository {
            None if self.repository_status == "unavailable" => lines.push(
                "Repository: Git could not read it here (not installed, or it refused this checkout)"
                    .to_owned(),
            ),
            None => lines.push("Repository: not a Git repository".to_owned()),
            Some(warnings) if warnings.is_empty() => {
                lines.push("Repository: nothing unfinished or diverged".to_owned())
            }
            Some(warnings) => {
                lines.push("Repository".to_owned());
                for warning in warnings {
                    let mark = match warning.level {
                        Level::Risk => "!",
                        Level::Caution => "·",
                    };
                    lines.push(format!("  {mark} {}", warning.fact));
                    lines.push(format!("    safe next: {}", warning.safe_next));
                }
            }
        }

        lines.push(String::new());
        if self.runtimes.is_empty() {
            lines.push("Runtimes: the project declares none Verb reads".to_owned());
        } else {
            lines.push(
                "Runtimes (declared by the project, compared with what runs in Verb's environment)"
                    .to_owned(),
            );
            for fact in &self.runtimes {
                lines.push(format!("  {}", fact.to_text()));
            }
        }

        lines.push(String::new());
        match &self.good {
            None => lines.push("Last known good: not marked (verb good mark)".to_owned()),
            Some((mark, distance)) => lines.push(format!(
                "Last known good: {} · {} · {}",
                mark.short_head().unwrap_or("before the first commit"),
                crate::iso8601(mark.marked_at),
                distance.summary()
            )),
        }

        lines.push(String::new());
        lines.push(if self.clear() {
            "Nothing observed calls for care.".to_owned()
        } else {
            "Verb ran nothing; each safe next step is yours to choose.".to_owned()
        });
        lines.join("\n")
    }
}

pub(crate) fn command(project: &Path, json: bool) -> Result<(), String> {
    let report = assemble(project)?;
    if json {
        println!("{}", report.to_json());
    } else {
        println!("{}", report.to_text());
    }
    Ok(())
}

/// `verb runtime [--json]`: the runtime half of `verb check`, on its own.
pub(crate) fn runtime_command(project: &Path, json: bool) -> Result<(), String> {
    let facts = runtime::observe(project);
    if json {
        let items: Vec<String> = facts.iter().map(Fact::to_json).collect();
        println!("{{\"runtimes\":[{}]}}", items.join(","));
    } else if facts.is_empty() {
        println!("This project declares no runtime version Verb reads.");
        println!(
            "Read: .nvmrc, .node-version, package.json engines, .python-version, pyproject.toml,"
        );
        println!("rust-toolchain(.toml), Cargo.toml rust-version, go.mod, .ruby-version, .tool-versions.");
    } else {
        for fact in facts {
            println!("{}", fact.to_text());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gitstate::{Operation, RepoState, Upstream};

    #[test]
    fn unknowns_are_reported_but_do_not_make_the_report_unclear() {
        let report = Report {
            assembled_at: 0,
            repository: Some(Vec::new()),
            repository_status: "read",
            runtimes: Vec::new(),
            good: None,
        };
        assert!(report.clear());
        assert!(report
            .to_text()
            .contains("Nothing observed calls for care."));
        assert!(report.to_json().contains("\"clear\":true"));
    }

    #[test]
    fn a_repository_warning_makes_the_report_unclear_and_names_the_safe_step() {
        let warnings = RepoState {
            operation: Some(Operation::Merge),
            unmerged: 0,
            detached: false,
            upstream: Upstream::None,
        }
        .warnings();
        let report = Report {
            assembled_at: 0,
            repository: Some(warnings),
            repository_status: "read",
            runtimes: Vec::new(),
            good: None,
        };
        assert!(!report.clear());
        let text = report.to_text();
        assert!(text.contains("A merge is in progress."));
        assert!(text.contains("git merge --abort"));
        assert!(report
            .to_json()
            .contains("\"code\":\"operation-in-progress\""));
    }
}
