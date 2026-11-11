//! Read-only work item documents and links. Warnings describe the current read.

use crate::specs::Spec;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read};
use std::path::Path;

const MAX_DOCUMENT_BYTES: u64 = 200 * 1024;
const BRD_SECTIONS: [&str; 5] = ["Problem", "Audience", "Goals", "Success measures", "Scope"];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkItems {
    items: Vec<WorkItem>,
    unlinked_specs: Vec<String>,
    warnings: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkItem {
    id: String,
    size: Option<&'static str>,
    brd: Option<String>,
    prd: Option<String>,
    goals: BTreeMap<String, bool>,
    requirements: BTreeMap<String, Requirement>,
    specs: Vec<SpecLinks>,
    warnings: Vec<String>,
}

#[derive(Serialize)]
struct Requirement {
    goal: String,
    retired: bool,
}

#[derive(Serialize)]
struct SpecLinks {
    id: String,
    criteria: Vec<CriterionLink>,
}

#[derive(Serialize)]
struct CriterionLink {
    number: usize,
    requirements: Vec<String>,
    stale: bool,
}

fn work_number(id: &str) -> Option<u32> {
    let (number, slug) = id.split_once('-')?;
    let value = number.parse::<u32>().ok()?;
    (value > 0
        && number == format!("{value:03}")
        && slug.len() <= 48
        && slug.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        }))
    .then_some(value)
}

fn read_document(dir: &Path, name: &str, warnings: &mut Vec<String>) -> Option<String> {
    let path = dir.join(name);
    let result = (|| -> io::Result<Option<String>> {
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let file = match options.open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.len() > MAX_DOCUMENT_BYTES {
            return Err(io::Error::other(
                "expected a regular Markdown file up to 200 KiB",
            ));
        }
        let mut raw = String::new();
        file.take(MAX_DOCUMENT_BYTES + 1).read_to_string(&mut raw)?;
        if raw.len() as u64 > MAX_DOCUMENT_BYTES || raw.contains('\0') {
            return Err(io::Error::other("expected UTF-8 Markdown up to 200 KiB"));
        }
        Ok(Some(raw))
    })();
    match result {
        Ok(raw) => raw,
        Err(error) => {
            warnings.push(format!("{name}: {error}"));
            None
        }
    }
}

fn valid_id(id: &str, prefix: char) -> bool {
    id.strip_prefix(prefix).is_some_and(|number| {
        !number.is_empty() && !number.starts_with('0') && number.bytes().all(|b| b.is_ascii_digit())
    })
}

fn fields(line: &str) -> Vec<&str> {
    let line = line.trim();
    if let Some(row) = line.strip_prefix('|').and_then(|row| row.strip_suffix('|')) {
        return row.split('|').map(str::trim).collect();
    }
    line.strip_prefix("- ")
        .unwrap_or(line)
        .split(" · ")
        .map(str::trim)
        .collect()
}

fn label(value: &str) -> &str {
    value.trim_matches('`')
}

fn retired(value: &str) -> (&str, bool) {
    let value = label(value);
    value
        .strip_suffix(" (retired)")
        .map_or((value, false), |id| (id, true))
}

fn records(raw: &str, prefix: char) -> impl Iterator<Item = (&str, bool, Vec<&str>)> {
    raw.lines().filter_map(move |line| {
        let mut fields = fields(line).into_iter();
        let first = fields.next()?;
        let first = first.strip_prefix("Goal: ").unwrap_or(first);
        let (id, retired) = retired(first);
        let id = label(id);
        valid_id(id, prefix).then(|| (id, retired, fields.collect()))
    })
}

impl WorkItem {
    fn read(id: String, dir: &Path) -> Self {
        let mut warnings = Vec::new();
        let brd = read_document(dir, "brd.md", &mut warnings);
        let prd = read_document(dir, "prd.md", &mut warnings);
        let mut size = None;
        let mut goals = BTreeMap::new();
        let mut requirements = BTreeMap::new();
        if let Some(brd) = &brd {
            let header = brd.lines().take_while(|line| !line.starts_with("## "));
            let sizes: Vec<_> = header
                .filter_map(|line| line.strip_prefix("Size: "))
                .collect();
            size = match sizes.as_slice() {
                ["small"] => Some("small"),
                ["standard"] => Some("standard"),
                _ => {
                    warnings.push(
                        "BRD needs exactly one Size: small or Size: standard line near the top"
                            .into(),
                    );
                    None
                }
            };
            if size == Some("standard") {
                for heading in BRD_SECTIONS {
                    if !crate::specs::section(brd, &format!("## {heading}"))
                        .iter()
                        .any(|line| !line.trim().is_empty())
                    {
                        warnings.push(format!("BRD is missing content in {heading}"));
                    }
                }
            } else if size == Some("small")
                && brd
                    .lines()
                    .filter(|line| {
                        let line = line.trim();
                        !line.is_empty() && !line.starts_with("# ") && !line.starts_with("Size: ")
                    })
                    .count()
                    != 3
            {
                warnings.push("Small BRD needs three content lines".into());
            }
            for (id, retired, _) in records(brd, 'G') {
                if goals.insert(id.to_owned(), retired).is_some() {
                    warnings.push(format!("Duplicate BRD goal {id}"));
                }
            }
            if goals.is_empty() {
                warnings.push("BRD has no numbered goals".into());
            }
        } else {
            warnings.push("Missing readable BRD".into());
        }
        if let Some(prd) = &prd {
            let mut count = 0;
            for (id, retired, fields) in records(prd, 'R') {
                count += 1;
                let goal = fields.first().map_or("", |goal| label(goal));
                if !goals.contains_key(goal) {
                    warnings.push(format!("Requirement {id} references missing goal {goal:?}"));
                } else if goals[goal] && !retired {
                    warnings.push(format!("Requirement {id} references retired goal {goal}"));
                }
                if !retired && fields.get(1).is_none_or(|text| text.is_empty()) {
                    warnings.push(format!("Requirement {id} is missing text"));
                }
                if !retired
                    && fields
                        .get(2)
                        .is_none_or(|verification| verification.is_empty())
                {
                    warnings.push(format!("Requirement {id} is missing verification"));
                }
                if requirements
                    .insert(
                        id.to_owned(),
                        Requirement {
                            goal: goal.to_owned(),
                            retired,
                        },
                    )
                    .is_some()
                {
                    warnings.push(format!("Duplicate PRD requirement {id}"));
                }
            }
            if count == 0 {
                warnings.push("PRD has no numbered requirements".into());
            }
            if size == Some("small") && (count > 5 || prd.lines().count() > 60) {
                warnings.push("Small PRD exceeds five requirements or 60 lines".into());
            }
        }
        Self {
            id,
            size,
            brd,
            prd,
            goals,
            requirements,
            specs: Vec::new(),
            warnings,
        }
    }

    fn link_spec(&mut self, spec: &Spec) {
        if self.prd.is_none() {
            self.warnings
                .push(format!("Spec {} is linked before the PRD exists", spec.id));
        }
        let mut legacy = BTreeMap::new();
        for line in crate::specs::section(&spec.body, "## Traceability") {
            if line.trim().is_empty() {
                continue;
            }
            let fields = fields(line);
            let [number, identity, requirements] = fields.as_slice() else {
                self.warnings
                    .push(format!("Spec {} has an invalid Traceability row", spec.id));
                continue;
            };
            let Some(number) = number.parse::<usize>().ok().filter(|n| *n > 0) else {
                self.warnings
                    .push(format!("Spec {} has an invalid criterion number", spec.id));
                continue;
            };
            if legacy.insert(number, (*identity, *requirements)).is_some() {
                self.warnings.push(format!(
                    "Spec {} has duplicate Traceability rows for criterion {number}",
                    spec.id
                ));
            }
        }

        let mut criteria = Vec::new();
        for criterion in &spec.criteria {
            let number = criterion.index + 1;
            let (references, stale) = match legacy.remove(&number) {
                Some((identity, refs)) => (
                    Some(refs),
                    crate::sdlc::criterion_key(spec, criterion.index).as_deref() != Some(identity),
                ),
                None => (
                    criterion
                        .text
                        .rsplit_once(" (")
                        .and_then(|(_, suffix)| suffix.strip_suffix(')'))
                        .filter(|refs| refs.starts_with('R')),
                    false,
                ),
            };
            let requirements: Vec<_> = references
                .into_iter()
                .flat_map(|refs| refs.split(','))
                .map(|id| id.trim().to_owned())
                .collect();
            let context = format!("Spec {} criterion {number}", spec.id);
            if stale {
                self.warnings
                    .push(format!("{context} has a stale Traceability link"));
            }
            if requirements.is_empty() {
                self.warnings
                    .push(format!("{context} has no requirement link"));
            }
            for id in &requirements {
                match self.requirements.get(id) {
                    None => self
                        .warnings
                        .push(format!("{context} references missing requirement {id:?}")),
                    Some(requirement) if requirement.retired => {
                        self.warnings
                            .push(format!("{context} references retired requirement {id}"));
                    }
                    Some(_) => {}
                }
            }
            criteria.push(CriterionLink {
                number,
                requirements,
                stale,
            });
        }
        for number in legacy.keys() {
            self.warnings.push(format!(
                "Spec {} has a stale Traceability link for missing criterion {number}",
                spec.id
            ));
        }
        self.specs.push(SpecLinks {
            id: spec.id.clone(),
            criteria,
        });
    }
}

pub(crate) fn read(project: &Path) -> Result<WorkItems, String> {
    let root = project.join("work");
    let mut items = Vec::new();
    let mut warnings = Vec::new();
    let mut numbers = BTreeSet::new();
    match fs::symlink_metadata(&root) {
        Ok(metadata) if metadata.is_dir() => {
            let entries =
                fs::read_dir(root).map_err(|error| format!("Cannot read work/: {error}"))?;
            let mut dirs = Vec::new();
            for entry in entries {
                let entry = entry.map_err(|error| format!("Cannot read work/ entry: {error}"))?;
                let Ok(id) = entry.file_name().into_string() else {
                    continue;
                };
                let Some(number) = work_number(&id) else {
                    continue;
                };
                if entry
                    .file_type()
                    .map_err(|error| error.to_string())?
                    .is_dir()
                {
                    dirs.push((number, id, entry.path()));
                }
            }
            dirs.sort();
            for (number, id, dir) in dirs {
                if !numbers.insert(number) {
                    warnings.push(format!("Duplicate work item number {number:03}"));
                }
                items.push(WorkItem::read(id, &dir));
            }
        }
        Ok(_) => warnings.push("work/ must be a directory".into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("Cannot read work/: {error}")),
    }

    let mut unlinked_specs = Vec::new();
    let mut seen_specs = BTreeSet::new();
    for spec in crate::specs::list_checked(project)? {
        if !seen_specs.insert(spec.id.clone()) {
            warnings.push(format!("Duplicate spec ID {}", spec.id));
        }
        if let Some(item) = items
            .iter_mut()
            .find(|item| spec.work_item.as_deref() == Some(&item.id))
        {
            item.link_spec(&spec);
        } else {
            if let Some(id) = &spec.work_item {
                warnings.push(format!(
                    "Spec {} references missing work item {id}",
                    spec.id
                ));
            }
            unlinked_specs.push(spec.id);
        }
    }
    Ok(WorkItems {
        items,
        unlinked_specs,
        warnings,
    })
}
