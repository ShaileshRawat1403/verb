//! Read-only projections of existing facts, shared by all Spec presentations.
use crate::specs::Spec;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

pub(crate) fn resolve(spec: &Spec, workspace: Option<&Value>, checkout: &Path) -> Value {
    let sessions = workspace.and_then(|w| w["sessions"].as_array());
    let mut associated = Vec::new();
    let mut seen = HashSet::new();
    let mut ambiguous = false;
    for entry in spec.audit.iter().rev() {
        let Some((_, rest)) = entry
            .action
            .strip_prefix("started ")
            .and_then(|s| s.split_once(" on this spec (session "))
        else {
            continue;
        };
        let Some(short) = rest.strip_suffix(')') else {
            continue;
        };
        if short.len() < 8
            || !short
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        {
            continue;
        }
        let matches: Vec<_> = sessions
            .into_iter()
            .flatten()
            .filter(|s| s["id"].as_str().is_some_and(|id| id.starts_with(short)))
            .collect();
        if matches.len() > 1 {
            ambiguous = true;
            continue;
        }
        if let Some(session) = matches.first() {
            let id = session["id"].as_str().unwrap_or_default();
            if !seen.insert(id.to_owned()) {
                continue;
            }
            associated.push(json!({"id": id, "agent": session["agent"], "state": session["state"],
                "connected": session["terminal"]["running"] == true,
                "sameCheckout": session["workspace"].as_str().is_some_and(|p| Path::new(p) == checkout),
                "helpRequested": session["attention"].as_u64().unwrap_or(0) > 0 }));
        }
    }
    let connected = associated.iter().filter(|s| s["connected"] == true).count();
    let live = associated.iter().any(|s| s["state"] == "live");
    let execution = if ambiguous {
        "Session association is ambiguous".to_owned()
    } else if sessions.is_none() {
        "Execution unavailable".into()
    } else if connected > 0 {
        format!(
            "{connected} connected session{}",
            if connected == 1 { "" } else { "s" }
        )
    } else if live {
        "Live session reported; not attached here".into()
    } else {
        "No associated session confirmed live".into()
    };
    let mut attention = Vec::new();
    for session in &associated {
        if session["helpRequested"] == true {
            attention.push(json!({"reason": "An associated session has a recorded help request", "sessionId": session["id"], "source": "task ledger"}));
        }
    }
    if spec.stage == "review" {
        attention.push(json!({"reason": "Review requested by the chosen stage", "sessionId": null, "source": "spec stage"}));
    }
    let project = workspace.map(|w| &w["project"]);
    let checkout_changes = project
        .filter(|p| {
            p["gitStatus"] == "branch" && p["branch"].as_str() == Some(spec.branch.as_str())
        })
        .and_then(|p| p["changedFiles"].as_u64());
    json!({"revision": spec.revision, "stage": spec.stage,
        "observedAt": workspace.and_then(|w| w["observedAt"].as_u64()),
        "execution": {"label": execution, "sessions": associated, "ambiguous": ambiguous},
        "verification": {"recorded": spec.criteria_done, "total": spec.criteria.len(),
            "result": "unknown", "freshness": "unknown",
            "label": format!("{}/{} criteria recorded complete", spec.criteria_done, spec.criteria.len()),
            "detail": "Checkmarks and audit notes record assertions. Independent test results and evidence freshness have not been established."},
        "delivery": {"label": "Release not observed", "checkoutChangedFiles": checkout_changes,
            "scope": "checkout", "detail": "A stage or commit does not prove a deployment. Use the Git review for this checkout's local and remote state."},
        "attention": attention })
}

pub(crate) fn resolve_all(
    specs: &[Spec],
    workspace: Option<&Value>,
    checkout: &Path,
) -> BTreeMap<String, Value> {
    specs
        .iter()
        .map(|s| (s.id.clone(), resolve(s, workspace, checkout)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn spec() -> Spec {
        Spec {
            id: "024".into(),
            revision: "revision".into(),
            title: "Recovery".into(),
            stage: "build".into(),
            branch: "spec/024".into(),
            created: String::new(),
            file: String::new(),
            body: String::new(),
            criteria: vec![],
            criteria_done: 0,
            audit: vec![crate::specs::AuditEntry {
                at: "now".into(),
                actor: "fixture".into(),
                action: "started codex on this spec (session abcdef12)".into(),
            }],
        }
    }
    #[test]
    fn stage_presence_assertions_and_release_remain_independent() {
        let spec = spec();
        let workspace = json!({"observedAt": 123, "project": {"branch": "spec/024", "gitStatus": "branch", "changedFiles": 3}, "sessions": [{"id": "abcdef12-full", "agent": "codex", "state": "live", "terminal": {"running": true}, "workspace": "/other", "attention": 1}]});
        let summary = resolve(&spec, Some(&workspace), Path::new("/project"));
        assert_eq!(summary["stage"], "build");
        assert_eq!(summary["execution"]["sessions"][0]["sameCheckout"], false);
        assert_eq!(summary["verification"]["result"], "unknown");
        assert_eq!(summary["verification"]["freshness"], "unknown");
        assert_eq!(summary["delivery"]["label"], "Release not observed");
        assert_eq!(summary["delivery"]["scope"], "checkout");
        assert_eq!(summary["attention"].as_array().unwrap().len(), 1);
    }
    #[test]
    fn ambiguous_associations_and_missing_git_do_not_invent_progress() {
        let workspace = json!({"sessions": [{"id": "abcdef12-one"}, {"id": "abcdef12-two"}], "project": {"gitStatus": "unavailable", "changedFiles": 0}});
        let summary = resolve(&spec(), Some(&workspace), Path::new("/project"));
        assert_eq!(summary["execution"]["ambiguous"], true);
        assert_eq!(summary["execution"]["sessions"], json!([]));
        assert!(summary["delivery"]["checkoutChangedFiles"].is_null());
        assert_eq!(summary["attention"], json!([]));
        let missing = resolve(&spec(), None, Path::new("/project"));
        assert_eq!(missing["execution"]["label"], "Execution unavailable");
        assert!(missing["observedAt"].is_null());
    }
}
