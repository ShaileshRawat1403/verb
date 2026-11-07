//! Agent chat adapters for live mobile continuation.
//!
//! Provides structured conversation history and safe input gating for supported CLI agents,
//! starting with Claude Code. Excludes private reasoning (thinking blocks), hooks, and internal metadata.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatMessageDto {
    pub id: String,
    pub sender: String, // "user" | "agent"
    pub text: String,
    pub timestamp: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatStateUpdate {
    pub agent: String,
    pub agent_state: String, // "waiting" | "working" | "awaiting_approval" | "uncertain"
    pub can_send_prompt: bool,
    pub messages: Vec<ChatMessageDto>,
}

pub(crate) trait AgentChatAdapter: Send {
    fn poll_chat(&mut self, terminal_screen: &str) -> ChatStateUpdate;
}

/// Creates a chat adapter for the session if a supported agent is specified.
pub(crate) fn create_adapter(
    agent_label: Option<&str>,
    project: &Path,
    hosted_pid: i32,
    resume_identity: Option<&str>,
) -> Option<Box<dyn AgentChatAdapter>> {
    if agent_label != Some("claude") {
        return None;
    }
    let store = crate::project::identity(project).ok()?.store;
    match agent_label {
        Some("claude") => Some(Box::new(ClaudeChatAdapter::new(
            project.to_path_buf(),
            hosted_pid,
            resume_identity.map(str::to_owned),
            store,
        ))),
        _ => None,
    }
}

pub(crate) struct ClaudeChatAdapter {
    project: PathBuf,
    hosted_pid: i32,
    resume_identity: Option<String>,
    known_session_id: Option<String>,
    transcript_path: Option<PathBuf>,
    store: PathBuf,
}

impl ClaudeChatAdapter {
    pub(crate) fn new(
        project: PathBuf,
        hosted_pid: i32,
        resume_identity: Option<String>,
        store: PathBuf,
    ) -> Self {
        Self {
            project,
            hosted_pid,
            resume_identity,
            known_session_id: None,
            transcript_path: None,
            store,
        }
    }

    fn resolve_transcript(&mut self) -> Option<PathBuf> {
        if let Some(path) = &self.transcript_path {
            if path.is_file() {
                return Some(path.clone());
            }
        }

        let home = crate::isolation::agent_home()?;
        let project_slug = crate::agents::claude_project_dir(&self.project);
        let projects_dir = home.join(".claude").join("projects").join(&project_slug);

        // 1. If an explicit resume identity was given, verify that exact transcript exists
        if let Some(id) = &self.resume_identity {
            crate::valid_resume_identity(id)?;
            let candidate = projects_dir.join(format!("{id}.jsonl"));
            if candidate.is_file() {
                self.known_session_id = Some(id.clone());
                self.transcript_path = Some(candidate.clone());
                return Some(candidate);
            }
            return None;
        }

        // 2. Discover via ~/.claude/sessions/<pid>.json
        let sessions_dir = home.join(".claude").join("sessions");
        if let Ok(entries) = fs::read_dir(sessions_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                    continue;
                }
                let Ok(contents) = fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(meta) = serde_json::from_str::<serde_json::Value>(&contents) else {
                    continue;
                };
                let pid = meta.get("pid").and_then(|v| v.as_i64()).map(|v| v as i32);
                let session_id = meta.get("sessionId").and_then(|v| v.as_str());
                let cwd = meta.get("cwd").and_then(|v| v.as_str());

                if let (Some(meta_pid), Some(sess_id), Some(meta_cwd)) = (pid, session_id, cwd) {
                    if !crate::agents::same_directory(meta_cwd, &self.project) {
                        continue;
                    }

                    // Check process relationship: direct PID match or same session leader (setsid)
                    let matches_process = meta_pid == self.hosted_pid || {
                        #[cfg(unix)]
                        {
                            unsafe extern "C" {
                                fn getsid(pid: i32) -> i32;
                            }
                            let meta_sid = unsafe { getsid(meta_pid) };
                            meta_sid == self.hosted_pid
                        }
                        #[cfg(not(unix))]
                        {
                            false
                        }
                    };

                    if matches_process && crate::valid_resume_identity(sess_id).is_some() {
                        let candidate = projects_dir.join(format!("{sess_id}.jsonl"));
                        if candidate.is_file() {
                            self.known_session_id = Some(sess_id.to_owned());
                            self.transcript_path = Some(candidate.clone());
                            return Some(candidate);
                        }
                    }
                }
            }
        }

        None
    }

    fn check_session_status(&self) -> Option<String> {
        let home = crate::isolation::agent_home()?;
        let sessions_dir = home.join(".claude").join("sessions");
        let Ok(entries) = fs::read_dir(sessions_dir) else {
            return None;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Ok(contents) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(meta) = serde_json::from_str::<serde_json::Value>(&contents) else {
                continue;
            };
            let session_id = meta.get("sessionId").and_then(|v| v.as_str());
            if session_id == self.known_session_id.as_deref() {
                return meta
                    .get("status")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned);
            }
        }
        None
    }
}

impl AgentChatAdapter for ClaudeChatAdapter {
    fn poll_chat(&mut self, terminal_screen: &str) -> ChatStateUpdate {
        // Recheck on every poll: revoking opt-in clears the next mobile chat update.
        if !crate::transcript::load(&self.store).enabled {
            return ChatStateUpdate {
                agent: "claude".to_owned(),
                agent_state: "uncertain".to_owned(),
                can_send_prompt: false,
                messages: Vec::new(),
            };
        }
        let messages = self
            .resolve_transcript()
            .map(|path| crate::transcript::claude_chat(&self.store, &path, &self.project))
            .unwrap_or_default();
        // Screen hints can restrict input, but never establish that it is safe to send a prompt.
        let approval_hint = [
            "[y/N]",
            "[Y/n]",
            "Do you want to run",
            "Allow this tool?",
            "Press Enter to approve",
            "Esc to cancel",
        ]
        .iter()
        .any(|hint| terminal_screen.contains(hint));
        let (agent_state, can_send_prompt) = if approval_hint {
            ("uncertain", false)
        } else if self.known_session_id.is_some() {
            match self.check_session_status().as_deref() {
                Some("idle") => ("waiting", true),
                Some("running" | "working") => ("working", false),
                _ => ("uncertain", false),
            }
        } else {
            ("uncertain", false)
        };
        ChatStateUpdate {
            agent: "claude".to_owned(),
            agent_state: agent_state.to_owned(),
            can_send_prompt,
            messages,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mobile_content_requires_opt_in_and_revocation_clears_it() {
        let root = std::env::temp_dir().join(format!("verb-chat-{}", crate::new_id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("transcript.jsonl");
        let secret = format!("ghp_{}", "a".repeat(36));
        fs::write(&path, format!("{{\"type\":\"assistant\",\"uuid\":\"a\",\"timestamp\":\"2026-08-22T08:00:00.123Z\",\"message\":{{\"content\":[{{\"type\":\"thinking\",\"thinking\":\"private\"}},{{\"type\":\"text\",\"text\":\"{secret}\"}}]}}}}\n")).unwrap();
        let mut adapter = ClaudeChatAdapter::new(root.clone(), 12345, None, root.clone());
        adapter.transcript_path = Some(path);
        assert!(adapter.poll_chat("").messages.is_empty());
        crate::transcript::save(&root, &crate::transcript::Settings { enabled: true }).unwrap();
        let update = adapter.poll_chat("");
        assert_eq!(update.messages.len(), 1);
        assert_eq!(update.messages[0].text, "[redacted]");
        assert_eq!(update.messages[0].timestamp, 1787385600123);
        assert!(
            !update.can_send_prompt,
            "an assistant reply alone is not proof of idle"
        );
        crate::transcript::save(&root, &crate::transcript::Settings { enabled: false }).unwrap();
        assert!(adapter.poll_chat("").messages.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
