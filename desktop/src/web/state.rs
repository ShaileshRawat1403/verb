//! Durable workspace snapshots run off the PTY pump. Runtime facts come from the host,
//! never from recovery metadata: an exited replay buffer is not a running process.
use crate::{project, workbench, SessionState};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

/// `Some(false)` retains replay only; `None` means this host owns no terminal buffer.
pub(super) fn terminal_status(hosted: Option<bool>) -> Value {
    json!({"running": hosted == Some(true), "replayAvailable": hosted.is_some()})
}

pub(super) struct StateCache {
    pub(super) body: Vec<u8>,
    pub(super) cached_at: Instant,
    pub(super) generation: u64,
}

pub(super) struct StateJob {
    pub(super) project: PathBuf,
    pub(super) identity: project::ProjectIdentity,
    pub(super) deployment: Value,
    pub(super) hosted: HashMap<String, bool>,
    pub(super) cache: Arc<std::sync::Mutex<Option<StateCache>>>,
    pub(super) generation: u64,
    pub(super) current_generation: Arc<AtomicU64>,
}

impl StateJob {
    pub(super) fn run(&self) -> Result<Vec<u8>, String> {
        // Concurrent state requests share work. The PTY thread only uses try_lock.
        let mut cache = self.cache.lock().map_err(|_| "state cache unavailable")?;
        if let Some(cached) = cache.as_ref().filter(|cached| {
            cached.generation == self.generation
                && cached.cached_at.elapsed() < Duration::from_millis(1500)
        }) {
            return Ok(cached.body.clone());
        }
        let body = self.state()?.to_string().into_bytes();
        if self.current_generation.load(Ordering::SeqCst) == self.generation {
            *cache = Some(StateCache {
                body: body.clone(),
                cached_at: Instant::now(),
                generation: self.generation,
            });
        }
        Ok(body)
    }

    fn state(&self) -> Result<Value, String> {
        let identity = &self.identity;
        let git = crate::git_snapshot(&self.project);
        let hosted_ids: Vec<_> = self.hosted.keys().map(String::as_str).collect();
        let sessions = crate::read_sessions_except(&hosted_ids)?;
        let tasks = workbench::snapshots(&self.project)?
            .into_iter()
            .map(|task| {
                json!({
                    "id": task.id, "title": task.title, "brief": task.brief, "status": task.status,
                    "owner": task.owner, "needsHelp": task.needs_help, "createdAt": task.created_at,
                    "events": task.events.into_iter().map(|event| json!({
                        "at": event.at, "kind": event.kind.label(), "sessionId": event.session_id,
                        "note": event.note,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>();
        let mut session_rows = Vec::new();
        for session in sessions {
            if !crate::session_in_project_with_id(&session, &self.project, &identity.id) {
                continue;
            }
            let attention = if session.agent.is_some() {
                tasks
                    .iter()
                    .filter(|task| {
                        task.get("owner").and_then(|o| o.as_str()) == Some(&session.id)
                            && task
                                .get("needsHelp")
                                .and_then(|h| h.as_bool())
                                .unwrap_or(false)
                    })
                    .count()
            } else {
                0
            };
            session_rows.push(json!({
                "id": session.id, "agent": session.display_agent(), "state": session.state.as_str(),
                "workspace": session.project_id, "isolated": session.project_id != identity.anchor,
                "lastSeenAt": session.last_seen_at, "hostedHere": self.hosted.get(&session.id).copied().unwrap_or(false),
                "hasTerminal": self.hosted.contains_key(&session.id),
                "terminal": terminal_status(self.hosted.get(&session.id).copied()),
                "canResume": session.state == SessionState::Recoverable && session.project_id.is_dir(),
                "attention": attention, "contextState": "not applicable",
            }));
        }
        Ok(json!({
            "project": {"id": identity.id, "name": identity.anchor.file_name()
                .map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
                "anchor": identity.anchor, "workspace": self.project,
                "branch": git.branch, "changedFiles": git.changed_files},
            "deployment": self.deployment,
            "memory": workbench::shared_memory(&self.project)?,
            "tasks": tasks, "sessions": session_rows,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replay_does_not_claim_a_running_process() {
        assert_eq!(
            terminal_status(Some(false)),
            json!({"running": false, "replayAvailable": true})
        );
        assert_eq!(
            terminal_status(Some(true)),
            json!({"running": true, "replayAvailable": true})
        );
        assert_eq!(
            terminal_status(None),
            json!({"running": false, "replayAvailable": false})
        );
    }
}
