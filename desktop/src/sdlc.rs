//! Private, bounded SDLC evidence. Repository text never creates an observed check result.
use crate::specs::Spec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MAX_BYTES: u64 = 2 * 1024 * 1024;
static ACTION: Mutex<()> = Mutex::new(());
fn digest(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Outcome {
    Passed,
    Failed,
    Inconclusive,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Goal {
    Code,
    Artifact,
    Service,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Kind {
    Plan,
    Evidence,
    Acceptance,
    Review,
    Delivery,
    Remote,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Source {
    Manual,
    Agent,
    Observed,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Binding {
    pub tree: String,
    pub intent: String,
    pub commit: String,
    pub branch: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Record {
    pub id: String,
    pub at: u128,
    pub actor: String,
    pub kind: Kind,
    pub source: Source,
    pub result: Option<Outcome>,
    pub binding: Option<Binding>,
    pub criterion: Option<String>,
    pub evidence_id: Option<String>,
    pub label: String,
    pub note: String,
    pub reference: Option<String>,
    pub proof_version: Option<String>,
    pub target: String,
    pub goal: Option<Goal>,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u128>,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ledger {
    version: u8,
    records: Vec<Record>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Snapshot {
    tree: String,
    commit: String,
    branch: String,
    clean: bool,
    integrated: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Request {
    action: String,
    expected_revision: String,
    expected_version: String,
    expected_context: String,
    #[serde(default)]
    criterion: Option<usize>,
    #[serde(default)]
    text: String,
    #[serde(default)]
    target: String,
    #[serde(default)]
    source: Option<Source>,
    #[serde(default)]
    result: Option<Outcome>,
    #[serde(default)]
    evidence_id: Option<String>,
    #[serde(default)]
    check: Option<String>,
    #[serde(default)]
    decision: Option<String>,
    #[serde(default)]
    goal: Option<Goal>,
    #[serde(default)]
    milestone: Option<String>,
}

fn read_git(
    project: &Path,
    args: &[&str],
    deadline: Instant,
) -> Result<std::process::Output, String> {
    if Instant::now() >= deadline {
        return Err("Source observation timed out. Refresh before continuing.".into());
    }
    let mut cmd = crate::exec::git(project).ok_or("Could not prepare source observation.")?;
    crate::exec::read_output_before(cmd.args(args), deadline)
        .map_err(|_| "Source observation unavailable. Refresh before continuing.".into())
}
fn text_git(project: &Path, args: &[&str], deadline: Instant) -> Result<String, String> {
    let output = read_git(project, args, deadline)?;
    if !output.status.success() {
        return Err(
            "Could not read the source revision. Save a first commit and check Git access.".into(),
        );
    }
    String::from_utf8(output.stdout)
        .map(|s| s.trim_end().into())
        .map_err(|_| "Source metadata is unreadable.".into())
}
fn spec_metadata(path: &str) -> bool {
    path.starts_with("specs/") && path.ends_with(".md")
}
pub(crate) fn snapshot(project: &Path) -> Result<Snapshot, String> {
    let deadline = Instant::now() + Duration::from_secs(30);
    let commit = text_git(
        project,
        &["rev-parse", "--verify", "HEAD^{commit}"],
        deadline,
    )?;
    let branch = text_git(
        project,
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
        deadline,
    )?;
    let tree = text_git(
        project,
        &["ls-tree", "-r", "-z", "--full-tree", "HEAD"],
        deadline,
    )?;
    let tree = tree
        .split('\0')
        .filter(|row| {
            row.split_once('\t')
                .is_some_and(|(_, path)| !spec_metadata(path))
        })
        .collect::<Vec<_>>()
        .join("\0");
    let status = text_git(
        project,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignore-submodules=none",
        ],
        deadline,
    )?;
    let mut paths = status.split('\0').filter(|p| !p.is_empty());
    let mut clean = true;
    while let Some(row) = paths.next() {
        if row.len() < 4 || !row.is_char_boundary(3) {
            return Err("Source status is unreadable.".into());
        }
        let renamed = row.as_bytes()[..2].iter().any(|c| matches!(c, b'R' | b'C'));
        let old = if renamed {
            Some(paths.next().ok_or("Source rename status is incomplete.")?)
        } else {
            None
        };
        if !spec_metadata(&row[3..]) || old.is_some_and(|p| !spec_metadata(p)) {
            clean = false;
        }
    }
    let mut integrated = None;
    for base in ["refs/heads/main", "refs/heads/master"] {
        let exists = read_git(
            project,
            &["rev-parse", "--verify", "--quiet", base],
            deadline,
        )?;
        if !exists.status.success() {
            continue;
        }
        let ancestor = read_git(
            project,
            &["merge-base", "--is-ancestor", &commit, base],
            deadline,
        )?;
        integrated = match ancestor.status.code() {
            Some(0) => Some(true),
            Some(1) => Some(false),
            _ => None,
        };
        break;
    }
    Ok(Snapshot {
        tree: digest(tree),
        commit,
        branch,
        clean,
        integrated,
    })
}

fn store_dir(root: &Path, project_id: &str) -> PathBuf {
    root.join("sdlc/v1").join(digest(project_id))
}
fn load(root: &Path, project_id: &str, id: &str) -> Result<Ledger, String> {
    if id.is_empty() || id.len() > 32 || !id.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Invalid evidence spec identity.".into());
    }
    let path = store_dir(root, project_id).join(format!("{id}.json"));
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Ledger {
                version: 1,
                records: vec![],
            })
        }
        Err(_) => {
            return Err("Evidence records are unreadable. Restore access before continuing.".into())
        }
    };
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read evidence records.")?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(
            "Evidence records exceed the supported size. Export and archive before continuing."
                .into(),
        );
    }
    let ledger: Ledger = serde_json::from_slice(&bytes)
        .map_err(|_| "Evidence records are damaged or unsupported; they have not been replaced.")?;
    if ledger.version != 1 || ledger.records.len() > 1000 {
        return Err("Evidence record version or size is unsupported.".into());
    }
    Ok(ledger)
}
fn version(ledger: &Ledger) -> String {
    digest(serde_json::to_vec(ledger).unwrap_or_default())
}
fn criterion_key(spec: &Spec, index: usize) -> Option<String> {
    spec.criteria
        .get(index)
        .map(|c| digest(format!("{index}\0{}", c.text)))
}
fn intent(spec: &Spec, ledger: &Ledger) -> String {
    let mut text = spec.title.clone();
    for heading in ["## Problem", "## Who it is for", "## Out of scope"] {
        if let Some((_, section)) = spec.body.split_once(heading) {
            text.push_str(section.split("\n## ").next().unwrap_or_default());
        }
    }
    for criterion in &spec.criteria {
        text.push_str(&criterion.text);
        text.push('\0');
    }
    if let Some(plan) = ledger.records.iter().rev().find(|r| r.kind == Kind::Plan) {
        text.push_str(&plan.label);
        text.push_str(&serde_json::to_string(&plan.goal).unwrap_or_default());
        text.push_str(&plan.target);
    }
    digest(text)
}
fn fresh(record: &Record, source: Option<&Snapshot>, intent: &str) -> &'static str {
    match (record.binding.as_ref(), source) {
        (Some(binding), Some(now))
            if now.clean && binding.tree == now.tree && binding.intent == intent =>
        {
            "current"
        }
        (Some(_), Some(_)) => "stale",
        _ => "unknown",
    }
}
fn assemble(spec: &Spec, ledger: &Ledger, source: Option<&Snapshot>) -> Value {
    let intent = intent(spec, ledger);
    let criteria: Vec<_> = spec.criteria.iter().map(|c| {
        let key = criterion_key(spec, c.index).unwrap_or_default();
        let latest = ledger.records.iter().rev().find(|r| r.kind == Kind::Evidence && r.criterion.as_deref() == Some(&key));
        let freshness = latest.map(|r| fresh(r, source, &intent)).unwrap_or("unknown");
        let accepted = latest.is_some_and(|e| e.result == Some(Outcome::Passed) && freshness == "current" && ledger.records.iter().any(|a| a.kind == Kind::Acceptance && a.evidence_id.as_deref() == Some(&e.id)));
        json!({"index": c.index, "text": c.text, "claimed": c.done, "accepted": accepted, "freshness": freshness,
            "result": latest.and_then(|r| r.result.as_ref()), "evidence": latest})
    }).collect();
    let accepted = criteria.iter().filter(|c| c["accepted"] == true).count();
    let all_accepted = !criteria.is_empty() && accepted == criteria.len();
    let review = ledger.records.iter().rev().find(|r| r.kind == Kind::Review);
    let proof_version = digest(
        serde_json::to_vec(
            &criteria
                .iter()
                .filter(|c| c["accepted"] == true)
                .map(|c| c["evidence"]["id"].clone())
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default(),
    );
    let review_current = review.is_some_and(|r| {
        fresh(r, source, &intent) == "current"
            && r.label == "approved"
            && all_accepted
            && r.proof_version.as_deref() == Some(proof_version.as_str())
    });
    let plan = ledger.records.iter().rev().find(|r| r.kind == Kind::Plan);
    let goal = plan.and_then(|p| p.goal.as_ref());
    let remote = ledger.records.iter().rev().find(|r| r.kind == Kind::Remote);
    let uploaded = remote.is_some_and(|r| {
        r.result == Some(Outcome::Passed) && fresh(r, source, &intent) == "current"
    });
    let latest_delivery = |milestone: &str| {
        ledger.records.iter().rev().find(|r| {
            r.kind == Kind::Delivery
                && r.label == milestone
                && plan.is_some_and(|p| r.target == p.target)
        })
    };
    let reported = |milestone: &str| {
        latest_delivery(milestone).is_some_and(|r| {
            r.result == Some(Outcome::Passed)
                && fresh(r, source, &intent) == "current"
                && (milestone != "health"
                    || latest_delivery("deployment")
                        .is_some_and(|d| r.evidence_id.as_deref() == Some(d.id.as_str())))
        })
    };
    let delivery_failed = ["artifact", "deployment", "health"]
        .iter()
        .any(|milestone| {
            let required = matches!(
                (goal, *milestone),
                (Some(Goal::Artifact), "artifact") | (Some(Goal::Service), "deployment" | "health")
            );
            required
                && latest_delivery(milestone).is_some_and(|r| {
                    r.result == Some(Outcome::Failed) && fresh(r, source, &intent) == "current"
                })
        });
    let ready = review_current
        && uploaded
        && match goal {
            Some(Goal::Code) => source.is_some_and(|s| s.integrated == Some(true)),
            Some(Goal::Artifact) => reported("artifact"),
            Some(Goal::Service) => reported("deployment") && reported("health"),
            None => false,
        };
    let failed = criteria
        .iter()
        .any(|c| c["result"] == "failed" && c["freshness"] == "current");
    let stale = criteria.iter().any(|c| c["freshness"] == "stale");
    let verification = if failed {
        "failed"
    } else if all_accepted {
        "accepted"
    } else if stale {
        "stale"
    } else {
        "unproven"
    };
    let review_state = review
        .map(|r| {
            if fresh(r, source, &intent) != "current" {
                "stale"
            } else if r.label == "changes_requested" {
                "changes_requested"
            } else if review_current {
                "approved"
            } else {
                "evidence_changed"
            }
        })
        .unwrap_or("not_reviewed");
    let context = digest(format!(
        "{}:{}:{}:{}",
        spec.revision,
        version(ledger),
        intent,
        serde_json::to_string(&source).unwrap_or_default()
    ));
    json!({"version": version(ledger), "context": context, "criteria": criteria, "accepted": accepted, "total": spec.criteria.len(),
        "verification": verification, "reviewState": review_state, "review": review, "plan": plan,
        "goal": goal, "ready": ready, "uploaded": uploaded, "source": source, "proofVersion": proof_version,
        "deliveryFailed": delivery_failed,
        "deliveryLabel": if delivery_failed { "Delivery failure reported · needs attention" } else if ready { "Delivery conditions recorded" } else if reported("deployment") { "Deployment reported · fresh health report required" } else if uploaded { "Implementation observed remotely" } else { "Release not observed" },
        "records": ledger.records.iter().rev().take(50).collect::<Vec<_>>()})
}
pub(crate) fn view(project: &Path, project_id: &str, spec: &Spec) -> Result<Value, String> {
    let ledger = load(&crate::state_root()?, project_id, &spec.id)?;
    Ok(assemble(spec, &ledger, snapshot(project).ok().as_ref()))
}

pub(crate) fn export(project_id: &str, spec: &Spec) -> Result<Value, String> {
    let ledger = load(&crate::state_root()?, project_id, &spec.id)?;
    Ok(
        json!({"schemaVersion": 1, "specId": spec.id, "records": ledger.records,
        "notice": "Host evidence metadata. Import elsewhere does not independently establish execution there."}),
    )
}
pub(crate) fn enrich_all(
    project: &Path,
    project_id: &str,
    specs: &[Spec],
    summaries: &mut std::collections::BTreeMap<String, Value>,
) {
    if specs.is_empty() {
        return;
    }
    let source = snapshot(project).ok();
    for spec in specs {
        let result = crate::state_root().and_then(|root| load(&root, project_id, &spec.id));
        if let Some(summary) = summaries.get_mut(&spec.id) {
            match result {
                Ok(ledger) => {
                    let flow = assemble(spec, &ledger, source.as_ref());
                    summary["verification"]["label"] = json!(format!(
                        "{}/{} criteria recorded complete · {}/{} accepted · {}",
                        spec.criteria_done,
                        spec.criteria.len(),
                        flow["accepted"],
                        flow["total"],
                        flow["verification"].as_str().unwrap_or("unknown")
                    ));
                    summary["verification"]["detail"] = json!(if ledger
                        .records
                        .iter()
                        .any(|r| r.kind == Kind::Evidence)
                    {
                        "Accepted evidence is bound to source and intent. Manual and agent reports remain distinct from Verb-observed checks."
                    } else {
                        "Checkmarks and audit notes record assertions. Independent test results and evidence freshness have not been established."
                    });
                    summary["verification"]["result"] = flow["verification"].clone();
                    summary["delivery"]["label"] = flow["deliveryLabel"].clone();
                    if flow["deliveryFailed"] == true
                        || matches!(flow["verification"].as_str(), Some("failed" | "stale"))
                        || matches!(
                            flow["reviewState"].as_str(),
                            Some("stale" | "changes_requested" | "evidence_changed")
                        )
                    {
                        summary["attention"].as_array_mut().unwrap().push(json!({"reason": "Evidence, review or delivery needs attention", "sessionId": null, "source": "evidence ledger"}));
                    }
                    summary["sdlc"] = json!({"context": flow["context"], "verification": flow["verification"], "reviewState": flow["reviewState"], "accepted": flow["accepted"], "total": flow["total"], "ready": flow["ready"], "goal": flow["goal"]});
                }
                Err(error) => {
                    summary["sdlc"] = json!({"error": error});
                    summary["verification"]["label"] = json!("Evidence records unavailable");
                }
            }
        }
    }
}

pub(crate) fn implementation_diff(project: &Path) -> Result<Value, String> {
    let deadline = Instant::now() + Duration::from_secs(30);
    let branch = text_git(
        project,
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
        deadline,
    )?;
    let mut base = None;
    for candidate in ["main", "master"] {
        if branch != candidate
            && read_git(
                project,
                &[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    &format!("refs/heads/{candidate}"),
                ],
                deadline,
            )?
            .status
            .success()
        {
            base = Some(format!("refs/heads/{candidate}"));
            break;
        }
    }
    let excludes = [
        "--",
        ".",
        ":(exclude)specs/*.md",
        ":(exclude).env*",
        ":(glob,exclude)**/.env*",
    ];
    let (output, scope) = if let Some(base) = base {
        let mut args = vec![
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--no-renames",
            &base,
            "HEAD",
        ];
        args.extend(excludes);
        (text_git(project, &args, deadline)?, format!("Committed implementation compared with local {base}. Spec Markdown and environment files are omitted."))
    } else {
        let mut args = vec![
            "show",
            "--format=",
            "--no-show-signature",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--no-renames",
            "HEAD",
        ];
        args.extend(excludes);
        (text_git(project, &args, deadline)?, "Latest commit only. Use a terminal for a wider comparison. Spec Markdown and environment files are omitted.".into())
    };
    let truncated = output.len() > 200_000;
    let mut limit = output.len().min(200_000);
    while !output.is_char_boundary(limit) {
        limit -= 1;
    }
    Ok(json!({"scope": scope, "diff": &output[..limit], "truncated": truncated}))
}

pub(crate) fn warnings(flow: &Value, stage: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    if matches!(stage, "build" | "verify" | "review" | "ship") && flow["plan"].is_null() {
        warnings.push("No implementation plan or completion target has been recorded.".into());
    }
    if matches!(stage, "review" | "ship")
        && (flow["accepted"] != flow["total"] || flow["total"] == 0)
    {
        warnings.push("Not every criterion has accepted, current evidence.".into());
    }
    if stage == "ship" && flow["ready"] != true {
        warnings.push(
            "Delivery conditions are incomplete. Moving to Ship does not confirm a release.".into(),
        );
    }
    warnings
}

pub(crate) fn act(
    project: &Path,
    project_id: &str,
    id: &str,
    input: Request,
    actor: &str,
) -> Result<Value, String> {
    let _active = ACTION
        .try_lock()
        .map_err(|_| "Another evidence action is running. Wait for it to finish.")?;
    let root = crate::state_root()?;
    let root_location = root.canonicalize().unwrap_or_else(|_| {
        if root.is_absolute() {
            root.clone()
        } else {
            std::env::current_dir().unwrap_or_default().join(&root)
        }
    });
    if root_location.starts_with(
        project
            .canonicalize()
            .map_err(|_| "Project location is unavailable.")?,
    ) {
        return Err("Evidence state must be outside the source checkout. Configure a private VERB_STATE_DIR outside this project.".into());
    }
    let dir = store_dir(&root, project_id);
    fs::create_dir_all(&dir).map_err(|_| "Could not prepare private evidence storage.")?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join(".lock"))
        .map_err(|_| "Could not open evidence lock.")?;
    lock.try_lock()
        .map_err(|_| "Another process is updating evidence. Retry after it finishes.")?;
    let spec = crate::specs::find(project, id)?.1;
    let mut ledger = load(&root, project_id, id)?;
    if spec.revision != input.expected_revision || version(&ledger) != input.expected_version {
        return Err("The spec or evidence changed. Refresh and review this action again.".into());
    }
    if input.text.len() > 8192
        || input.target.len() > 200
        || input.target.chars().any(char::is_control)
    {
        return Err("Keep the note under 8 KiB and use a short, single-line target.".into());
    }
    if ledger.records.len() >= 1000 {
        return Err("Evidence history is full. Export it before adding more records.".into());
    }
    let source = snapshot(project).ok();
    let before = assemble(&spec, &ledger, source.as_ref());
    if before["context"].as_str() != Some(input.expected_context.as_str()) {
        return Err("Source or evidence changed while this action was open. Refresh observations and review it again.".into());
    }
    let mut record = Record {
        id: crate::new_id(),
        at: crate::now_millis(),
        actor: actor.into(),
        kind: Kind::Plan,
        source: Source::Manual,
        result: None,
        binding: None,
        criterion: None,
        evidence_id: None,
        label: input.text.trim().into(),
        note: String::new(),
        reference: None,
        proof_version: None,
        target: input.target.trim().into(),
        goal: None,
        exit_code: None,
        duration_ms: None,
    };
    if input.action != "plan" {
        let source = source.as_ref().filter(|s| s.clean).ok_or("Save code changes to Git history before recording evidence, review or delivery. Spec Markdown changes can stay uncommitted.")?;
        record.binding = Some(Binding {
            tree: source.tree.clone(),
            intent: intent(&spec, &ledger),
            commit: source.commit.clone(),
            branch: source.branch.clone(),
        });
    }
    match input.action.as_str() {
        "plan" => {
            if record.label.is_empty() {
                return Err("Describe the approach, risks and verification strategy.".into());
            }
            record.goal = Some(
                input
                    .goal
                    .ok_or("Choose what completion means for this spec.")?,
            );
            if record.goal != Some(Goal::Code) && record.target.is_empty() {
                return Err(
                    "Name the release channel or service environment this spec must reach.".into(),
                );
            }
        }
        "report" | "run" => {
            let index = input
                .criterion
                .ok_or("Choose the criterion this evidence supports.")?;
            record.criterion =
                Some(criterion_key(&spec, index).ok_or("That criterion no longer exists.")?);
            record.kind = Kind::Evidence;
            if input.action == "report" {
                record.source = match input.source {
                    Some(Source::Manual) => Source::Manual,
                    Some(Source::Agent) => Source::Agent,
                    _ => {
                        return Err("A report must identify a person or agent as its source.".into())
                    }
                };
                if record.label.is_empty() {
                    return Err("Describe what was checked, with a test/device/reference rather than terminal output.".into());
                }
                record.result = Some(input.result.ok_or("Choose the reported result.")?);
            } else {
                let (name, script) = match input.check.as_deref() {
                    Some("rust") => ("Rust test task", "command -v cargo >/dev/null 2>&1 || exit 127; exec cargo test --offline"),
                    Some("node") => ("Node test task", "command -v npm >/dev/null 2>&1 || exit 127; exec npm --offline test"),
                    Some("python") => ("Python unit tests", "command -v python3 >/dev/null 2>&1 || exit 127; exec python3 -c 'import sys, unittest; suite = unittest.defaultTestLoader.discover(\".\"); sys.exit(125 if suite.countTestCases() == 0 else (0 if unittest.TextTestRunner().run(suite).wasSuccessful() else 1))'"),
                    _ => return Err("Choose a supported check. Arbitrary commands are not accepted.".into()),
                };
                let bash = crate::exec::trusted_program("bash", project)
                    .ok_or("Bash is unavailable for this check.")?;
                let mut command = crate::isolation::command(&bash, project, "")?;
                command
                    .current_dir(project)
                    .env_remove("BASH_ENV")
                    .env_remove("ENV")
                    .env("GIT_TERMINAL_PROMPT", "0")
                    .args(["--noprofile", "--norc", "-c", script]);
                for (key, _) in std::env::vars_os()
                    .filter(|(key, _)| key.to_string_lossy().starts_with("VERB_"))
                {
                    command.env_remove(key);
                }
                if !crate::isolation::external() {
                    let paths =
                        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                            .filter(|p| {
                                p.is_absolute()
                                    && !p
                                        .canonicalize()
                                        .unwrap_or_else(|_| p.clone())
                                        .starts_with(project)
                            })
                            .collect::<Vec<_>>();
                    command.env(
                        "PATH",
                        std::env::join_paths(paths)
                            .map_err(|_| "Could not prepare the check environment.")?,
                    );
                }
                let start = Instant::now();
                let output = crate::exec::check_output(&mut command);
                record.duration_ms = Some(start.elapsed().as_millis());
                record.source = Source::Observed;
                record.label = format!(
                    "{name} · {}/{}",
                    std::env::consts::OS,
                    std::env::consts::ARCH
                );
                match output {
                    Ok(output) => {
                        if crate::isolation::refused(output.status.code(), &output.stderr) {
                            return Err("The execution boundary did not admit this check. No failed test result was recorded; retry when capacity is available.".into());
                        }
                        record.exit_code = output.status.code();
                        record.result = Some(match output.status.code() {
                            Some(0) => Outcome::Passed,
                            Some(125 | 127) | None => Outcome::Inconclusive,
                            _ => Outcome::Failed,
                        });
                    }
                    Err(_) => {
                        record.result = Some(Outcome::Inconclusive);
                        record.label.push_str(" · timed out, exceeded limits or could not start; cleanup/outcome unconfirmed");
                    }
                }
            }
        }
        "accept" => {
            let evidence = input
                .evidence_id
                .as_ref()
                .and_then(|id| {
                    ledger
                        .records
                        .iter()
                        .find(|r| &r.id == id && r.kind == Kind::Evidence)
                })
                .ok_or("Choose an evidence record to accept.")?;
            if evidence.result != Some(Outcome::Passed)
                || fresh(evidence, source.as_ref(), &intent(&spec, &ledger)) != "current"
            {
                return Err("Only passing, current evidence can be accepted. Re-run or update the check first.".into());
            }
            let row = before["criteria"].as_array().unwrap().iter().find(|c| c["evidence"]["id"] == evidence.id).ok_or("This evidence was superseded or its criterion changed. Review the latest record.")?;
            if row["accepted"] == true {
                return Err("This evidence is already accepted.".into());
            }
            if record.label.is_empty() {
                return Err("Explain why this evidence is sufficient for the criterion.".into());
            }
            record.kind = Kind::Acceptance;
            record.criterion = evidence.criterion.clone();
            record.evidence_id = input.evidence_id;
        }
        "review" => {
            record.kind = Kind::Review;
            record.proof_version = before["proofVersion"].as_str().map(str::to_owned);
            match input.decision.as_deref() {
                Some("approved") if before["accepted"] == before["total"] && before["total"] != 0 => {},
                Some("approved") => return Err("Accept current evidence for every criterion before approving this implementation.".into()),
                Some("changes_requested") => {},
                _ => return Err("Choose an approval or a request for changes.".into()),
            }
            if input.text.trim().is_empty() {
                return Err("Record what you reviewed and any remaining risks.".into());
            }
            record.note = input.text.trim().into();
            record.label = input.decision.unwrap();
        }
        "remote" => {
            let review = crate::git_sync::review(project)?;
            if Some(review.local_oid.as_str()) != source.as_ref().map(|s| s.commit.as_str()) {
                return Err(
                    "The checkout changed before the remote check. Refresh and try again.".into(),
                );
            }
            let review = crate::git_sync::transfer(
                project,
                crate::git_sync::Request {
                    action: "check".into(),
                    review,
                },
            )?;
            record.kind = Kind::Remote;
            record.source = Source::Observed;
            record.label = format!("{}/{}", review.remote, review.remote_ref);
            record.target = review.remote_oid;
            record.result = Some(if review.ahead == 0 {
                Outcome::Passed
            } else {
                Outcome::Inconclusive
            });
        }
        "delivery" => {
            if before["reviewState"] != "approved" {
                return Err("Approve a current implementation review before recording release or deployment results.".into());
            }
            let milestone = input
                .milestone
                .as_deref()
                .filter(|s| matches!(*s, "artifact" | "deployment" | "health"))
                .ok_or("Choose an artifact, deployment or health report.")?;
            if record.label.is_empty() || record.target.is_empty() {
                return Err("Give the target and release/deployment/check reference.".into());
            }
            let deployment = ledger
                .records
                .iter()
                .rev()
                .find(|r| {
                    r.kind == Kind::Delivery && r.label == "deployment" && r.target == record.target
                })
                .filter(|r| {
                    r.result == Some(Outcome::Passed)
                        && fresh(r, source.as_ref(), &intent(&spec, &ledger)) == "current"
                });
            if milestone == "health" && deployment.is_none() {
                return Err("Record a successful deployment to this same target before reporting its health.".into());
            }
            if milestone == "health" {
                record.evidence_id = deployment.map(|r| r.id.clone());
            }
            record.kind = Kind::Delivery;
            record.reference = Some(record.label.clone());
            record.label = milestone.into();
            record.result = Some(input.result.ok_or("Choose the reported delivery result.")?);
        }
        _ => return Err("Unknown SDLC action.".into()),
    }
    if crate::specs::find(project, id)?.1.revision != spec.revision {
        return Err("The spec changed during this action. The result was not attached; refresh before retrying.".into());
    }
    ledger.records.push(record);
    let bytes = serde_json::to_vec(&ledger).map_err(|_| "Could not encode evidence record.")?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Evidence storage limit reached. Export the history before continuing.".into());
    }
    crate::fsutil::atomic_write(&dir.join(format!("{id}.json")), &bytes)?;
    Ok(assemble(&spec, &ledger, snapshot(project).ok().as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Spec, Ledger, Snapshot) {
        let spec = Spec {
            id: "1".into(),
            revision: "revision".into(),
            title: "Reliable answer".into(),
            stage: "build".into(),
            branch: "main".into(),
            created: String::new(),
            file: "specs/1.md".into(),
            criteria: vec![crate::specs::Criterion {
                index: 0,
                text: "The answer is correct".into(),
                done: true,
            }],
            criteria_done: 1,
            audit: vec![],
            body: String::new(),
        };
        let ledger = Ledger {
            version: 1,
            records: vec![],
        };
        let source = Snapshot {
            tree: "tree".into(),
            commit: "commit".into(),
            branch: "main".into(),
            clean: true,
            integrated: Some(true),
        };
        (spec, ledger, source)
    }
    fn record(spec: &Spec, ledger: &Ledger, source: &Snapshot, id: &str, kind: Kind) -> Record {
        Record {
            id: id.into(),
            at: 1,
            actor: "owner".into(),
            kind,
            source: Source::Manual,
            result: Some(Outcome::Passed),
            binding: Some(Binding {
                tree: source.tree.clone(),
                intent: intent(spec, ledger),
                commit: source.commit.clone(),
                branch: source.branch.clone(),
            }),
            criterion: criterion_key(spec, 0),
            evidence_id: None,
            label: String::new(),
            note: String::new(),
            reference: None,
            proof_version: None,
            target: "phone".into(),
            goal: None,
            exit_code: None,
            duration_ms: None,
        }
    }
    fn accepted_review() -> (Spec, Ledger, Snapshot) {
        let (spec, mut ledger, source) = fixture();
        let mut plan = record(&spec, &ledger, &source, "plan", Kind::Plan);
        plan.label = "Run and review the test".into();
        plan.goal = Some(Goal::Service);
        ledger.records.push(plan);
        ledger
            .records
            .push(record(&spec, &ledger, &source, "test", Kind::Evidence));
        let mut acceptance = record(&spec, &ledger, &source, "accept", Kind::Acceptance);
        acceptance.evidence_id = Some("test".into());
        ledger.records.push(acceptance);
        let mut review = record(&spec, &ledger, &source, "review", Kind::Review);
        review.label = "approved".into();
        review.proof_version = assemble(&spec, &ledger, Some(&source))["proofVersion"]
            .as_str()
            .map(str::to_owned);
        ledger.records.push(review);
        ledger
            .records
            .push(record(&spec, &ledger, &source, "remote", Kind::Remote));
        (spec, ledger, source)
    }

    #[test]
    fn assertions_do_not_become_evidence_and_unknown_source_never_accepts() {
        let (spec, ledger, source) = fixture();
        assert_eq!(assemble(&spec, &ledger, Some(&source))["accepted"], 0);
        let (spec, ledger, _) = accepted_review();
        let flow = assemble(&spec, &ledger, None);
        assert_eq!(flow["accepted"], 0);
        assert_eq!(flow["criteria"][0]["freshness"], "unknown");
        assert_eq!(flow["ready"], false);
    }

    #[test]
    fn source_and_intent_changes_invalidate_acceptance_without_erasing_failed_results() {
        let (mut spec, mut ledger, mut source) = accepted_review();
        assert_eq!(
            assemble(&spec, &ledger, Some(&source))["reviewState"],
            "approved"
        );
        spec.stage = "review".into();
        spec.criteria[0].done = false;
        spec.revision = "stage-change".into();
        assert_eq!(assemble(&spec, &ledger, Some(&source))["accepted"], 1);
        source.clean = false;
        assert_eq!(
            assemble(&spec, &ledger, Some(&source))["reviewState"],
            "stale"
        );
        source.clean = true;
        spec.criteria[0].text = "A different promise".into();
        assert_eq!(assemble(&spec, &ledger, Some(&source))["accepted"], 0);
        let mut failed = record(&spec, &ledger, &source, "failed", Kind::Evidence);
        failed.result = Some(Outcome::Failed);
        ledger.records.push(failed);
        assert_eq!(
            assemble(&spec, &ledger, Some(&source))["verification"],
            "failed"
        );
        source.tree = "new-code".into();
        let flow = assemble(&spec, &ledger, Some(&source));
        assert_eq!(flow["criteria"][0]["result"], "failed");
        assert_eq!(flow["criteria"][0]["freshness"], "stale");
    }

    #[test]
    fn new_evidence_requires_acceptance_and_a_new_review() {
        let (spec, mut ledger, source) = accepted_review();
        ledger
            .records
            .push(record(&spec, &ledger, &source, "new-test", Kind::Evidence));
        assert_eq!(assemble(&spec, &ledger, Some(&source))["accepted"], 0);
        let mut acceptance = record(&spec, &ledger, &source, "new-acceptance", Kind::Acceptance);
        acceptance.evidence_id = Some("new-test".into());
        ledger.records.push(acceptance);
        assert_eq!(
            assemble(&spec, &ledger, Some(&source))["reviewState"],
            "evidence_changed"
        );
    }

    #[test]
    fn service_health_is_bound_to_the_latest_deployment_and_target() {
        let (spec, mut ledger, source) = accepted_review();
        let mut deploy = record(&spec, &ledger, &source, "deploy-1", Kind::Delivery);
        deploy.label = "deployment".into();
        ledger.records.push(deploy.clone());
        let mut health = record(&spec, &ledger, &source, "health-1", Kind::Delivery);
        health.label = "health".into();
        health.evidence_id = Some("deploy-1".into());
        ledger.records.push(health.clone());
        assert_eq!(assemble(&spec, &ledger, Some(&source))["ready"], true);
        deploy.id = "deploy-2".into();
        ledger.records.push(deploy);
        assert_eq!(assemble(&spec, &ledger, Some(&source))["ready"], false);
        health.id = "health-2".into();
        health.evidence_id = Some("deploy-2".into());
        health.result = Some(Outcome::Failed);
        ledger.records.push(health.clone());
        assert_eq!(
            assemble(&spec, &ledger, Some(&source))["deliveryFailed"],
            true
        );
        health.id = "health-3".into();
        health.result = Some(Outcome::Passed);
        health.target = "another-phone".into();
        ledger.records.push(health);
        assert_eq!(assemble(&spec, &ledger, Some(&source))["ready"], false);
    }

    #[test]
    fn private_ledger_rejects_traversal_corruption_and_unsupported_versions() {
        let root = std::env::temp_dir().join(format!("verb-sdlc-{}", crate::new_id()));
        assert!(load(&root, "project", "../other").is_err());
        let directory = store_dir(&root, "project");
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("1.json");
        for bytes in [
            "damaged",
            "{\"version\":2,\"records\":[]}",
            "{\"version\":1,\"records\":[],\"forged\":true}",
        ] {
            fs::write(&path, bytes).unwrap();
            assert!(load(&root, "project", "1").is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), bytes);
        }
        fs::remove_dir_all(root).unwrap();
    }
}
