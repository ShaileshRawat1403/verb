//! Work item documents, validation and owner-approved publication.

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
    brd_revision: Option<String>,
    prd_revision: Option<String>,
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

fn document_body(raw: &str) -> &str {
    raw.split_once("\n## Audit trail\n")
        .map_or(raw, |(body, _)| body.trim_end())
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

pub(crate) fn traceability_number(line: &str) -> Option<usize> {
    match fields(line).as_slice() {
        [number, _, _] => number.parse().ok(),
        _ => None,
    }
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
        Self::validate(id, brd, prd, warnings)
    }

    fn validate(
        id: String,
        brd: Option<String>,
        prd: Option<String>,
        mut warnings: Vec<String>,
    ) -> Self {
        let mut size = None;
        let mut goals = BTreeMap::new();
        let mut requirements = BTreeMap::new();
        if let Some(brd) = &brd {
            let brd = document_body(brd);
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
            let prd = document_body(prd);
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
            brd_revision: brd.as_deref().map(revision),
            prd_revision: prd.as_deref().map(revision),
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

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Publish {
    pub id: Option<String>,
    pub title: Option<String>,
    pub document: Document,
    pub content: String,
    pub expected_revision: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Document {
    Brd,
    Prd,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Link {
    pub work_item: String,
    pub spec_id: String,
    pub expected_revision: String,
    #[serde(default)]
    pub criteria: BTreeMap<usize, Vec<String>>,
    #[serde(default)]
    pub remove_criteria: Vec<usize>,
}

fn revision(raw: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(raw.as_bytes()))
}

fn require_dir(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if metadata.is_dir() {
        Ok(())
    } else {
        Err("Expected a real work item directory".into())
    }
}

fn allocate(root: &Path) -> Result<u32, String> {
    let mut highest = 0;
    for entry in fs::read_dir(root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if let Some(number) = entry.file_name().to_str().and_then(work_number) {
            highest = highest.max(number);
        }
    }
    let counter = root.join(".next-id");
    let mut warnings = Vec::new();
    let saved = read_document(root, ".next-id", &mut warnings);
    if !warnings.is_empty() {
        return Err("Cannot read work/.next-id".into());
    }
    let next = match saved {
        Some(raw) => raw
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|number| *number > 0)
            .ok_or("Invalid work/.next-id")?,
        None => 1,
    };
    let number = next.max(
        highest
            .checked_add(1)
            .ok_or("Work item numbers exhausted")?,
    );
    let following = number.checked_add(1).ok_or("Work item numbers exhausted")?;
    crate::specs::write_atomic(&counter, &format!("{following}\n"))?;
    Ok(number)
}

pub(crate) fn publish(
    project: &Path,
    input: Publish,
    actor: &str,
) -> Result<serde_json::Value, String> {
    if input.content.trim().is_empty()
        || input.content.contains('\0')
        || input.content.len() as u64 > MAX_DOCUMENT_BYTES
        || input
            .content
            .lines()
            .any(|line| line.trim_end() == "## Audit trail")
    {
        return Err(
            "Supply nonempty Markdown within 200 KiB, without the managed Audit trail section"
                .into(),
        );
    }
    let identity = crate::project::identity(project)?;
    let _lock = crate::workbench::lock_project(&identity.store)?;
    let root = project.join("work");
    match fs::create_dir(&root) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.to_string()),
    }
    require_dir(&root)?;
    let creating = input.id.is_none();
    let id = match input.id {
        Some(id) if work_number(&id).is_some() => id,
        Some(_) => return Err("Invalid work item ID".into()),
        None => {
            let title = input
                .title
                .as_deref()
                .map(str::trim)
                .filter(|title| !title.is_empty())
                .ok_or("A title is required to create a work item")?;
            if !matches!(input.document, Document::Brd) || input.expected_revision.is_some() {
                return Err("Start a new work item by publishing its BRD".into());
            }
            format!("{:03}-{}", allocate(&root)?, crate::specs::slug(title))
        }
    };
    let dir = root.join(&id);
    let name = match input.document {
        Document::Brd => "brd.md",
        Document::Prd => "prd.md",
    };
    if !creating {
        require_dir(&dir)?;
    }
    let mut item = WorkItem::read(id.clone(), &dir);
    if matches!(input.document, Document::Prd) && item.brd.is_none() {
        return Err("Publish a readable BRD before the PRD".into());
    }
    let mut warnings = Vec::new();
    let previous = read_document(&dir, name, &mut warnings);
    if !warnings.is_empty() {
        return Err(format!("Cannot safely replace {name}"));
    }
    if previous.as_deref().map(revision) != input.expected_revision {
        return Err(format!("{name} changed. Refresh before publishing."));
    }
    let audit = previous.as_deref().map_or("", |raw| {
        raw.match_indices("## Audit trail")
            .find(|(offset, _)| {
                (*offset == 0 || raw.as_bytes()[offset - 1] == b'\n')
                    && raw[*offset..].lines().next() == Some("## Audit trail")
            })
            .map_or("", |(offset, _)| &raw[offset..])
    });
    let mut raw = input.content.trim_end().to_owned();
    if !audit.is_empty() {
        raw.push_str("\n\n");
        raw.push_str(audit);
    }
    // Validate the candidate in memory before publishing or appending its warning record.
    match input.document {
        Document::Brd => item.brd = Some(input.content.clone()),
        Document::Prd => item.prd = Some(input.content.clone()),
    }
    item = WorkItem::validate(id.clone(), item.brd, item.prd, Vec::new());
    let action = if item.warnings.is_empty() {
        format!("published {name}")
    } else {
        format!("published {name} (warning: {})", item.warnings.join(" | "))
    };
    let raw = crate::specs::append_audit(&raw, &crate::specs::audit_line(actor, &action));
    if raw.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err("Document and audit trail exceed 200 KiB".into());
    }
    if creating {
        let staging = root.join(format!(".publish-{}", crate::new_id()));
        fs::create_dir(&staging).map_err(|error| error.to_string())?;
        let result = crate::specs::write_atomic(&staging.join(name), &raw)
            .and_then(|()| fs::rename(&staging, &dir).map_err(|error| error.to_string()));
        if result.is_err() {
            let _ = fs::remove_dir_all(&staging);
        }
        result?;
        #[cfg(unix)]
        fs::File::open(&root)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| {
                format!("Could not sync the work directory after publishing: {error}")
            })?;
    } else {
        crate::specs::write_atomic(&dir.join(name), &raw)?;
    }
    Ok(serde_json::json!({"id": id, "revision": revision(&raw), "warnings": item.warnings}))
}

pub(crate) fn link(project: &Path, input: Link, actor: &str) -> Result<(), String> {
    if work_number(&input.work_item).is_none() {
        return Err("Invalid work item ID".into());
    }
    let identity = crate::project::identity(project)?;
    let _lock = crate::workbench::lock_project(&identity.store)?;
    let root = project.join("work");
    require_dir(&root)?;
    let dir = root.join(&input.work_item);
    require_dir(&dir)?;
    let item = WorkItem::read(input.work_item.clone(), &dir);
    if item.brd.is_none() || item.prd.is_none() {
        return Err("Publish the BRD and PRD before linking a spec".into());
    }
    let (_, spec) = crate::specs::find(project, &input.spec_id)?;
    if spec.revision != input.expected_revision {
        return Err("The spec changed. Refresh before linking it.".into());
    }
    if input.criteria.is_empty() && input.remove_criteria.is_empty() {
        return Err("Choose at least one criterion to link or remove".into());
    }
    if spec.work_item.is_none() && input.criteria.is_empty() {
        return Err("Link this spec before removing links".into());
    }
    let mut rows = BTreeMap::new();
    for number in input.remove_criteria {
        if number == 0 || input.criteria.contains_key(&number) {
            return Err("Choose a valid criterion to remove without also linking it".into());
        }
        rows.insert(number, None);
    }
    let mut warnings = Vec::new();
    for (number, requirements) in input.criteria {
        let index = number.checked_sub(1).ok_or("Invalid criterion number")?;
        let identity = crate::sdlc::criterion_key(&spec, index).ok_or("Unknown criterion")?;
        if requirements.is_empty() {
            return Err("Choose at least one requirement per mapped criterion".into());
        }
        for requirement in &requirements {
            if !valid_id(requirement, 'R') {
                return Err("Invalid requirement ID".into());
            }
            match item.requirements.get(requirement) {
                Some(record) if record.retired => {
                    warnings.push(format!("criterion {number} links to retired {requirement}"))
                }
                None => warnings.push(format!("criterion {number} links to missing {requirement}")),
                _ => {}
            }
        }
        rows.insert(
            number,
            Some(format!(
                "- {number} · {identity} · {}",
                requirements.join(", ")
            )),
        );
    }
    crate::specs::link_work_item(
        project,
        &input.spec_id,
        &input.expected_revision,
        &input.work_item,
        &rows,
        actor,
        &warnings,
    )
}

pub(crate) fn valid_work_item_id(id: &str) -> bool {
    work_number(id).is_some()
}

/// Selects only current requirement links and their goals for the spec context.
pub(crate) fn linked_context(spec: &Spec, brd: Option<String>, prd: Option<String>) -> Vec<String> {
    let Some(id) = &spec.work_item else {
        return Vec::new();
    };
    let (Some(brd), Some(prd)) = (brd, prd) else {
        return vec![format!(
            "Work item {id} has no visible BRD and PRD in this checkout."
        )];
    };
    let mut item = WorkItem::validate(id.clone(), Some(brd.clone()), Some(prd.clone()), Vec::new());
    item.link_spec(spec);
    let references: BTreeSet<_> = item.specs[0]
        .criteria
        .iter()
        .filter(|link| !link.stale)
        .flat_map(|link| &link.requirements)
        .map(String::as_str)
        .collect();
    let goals: BTreeSet<_> = references
        .iter()
        .filter_map(|id| item.requirements.get(*id))
        .filter(|record| !record.retired)
        .map(|record| record.goal.as_str())
        .collect();
    let mut context = vec![format!("Work item {id}")];
    for (id, retired, fields) in records(document_body(&brd), 'G') {
        if !retired && goals.contains(id) && item.goals.get(id) == Some(&false) {
            context.push(format!("BRD goal {id} · {}", fields.join(" · ")));
        }
    }
    for (id, retired, fields) in records(document_body(&prd), 'R') {
        if !retired
            && references.contains(id)
            && item
                .requirements
                .get(id)
                .is_some_and(|record| !record.retired)
        {
            context.push(format!("PRD requirement {id} · {}", fields.join(" · ")));
        }
    }
    context
}
