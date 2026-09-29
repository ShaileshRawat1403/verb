//! Agent chat adapters for live mobile continuation.
//!
//! Provides structured conversation history and safe input gating for supported CLI agents,
//! starting with Claude Code. Excludes private reasoning (thinking blocks), hooks, and internal metadata.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
    match agent_label {
        Some("claude") => Some(Box::new(ClaudeChatAdapter::new(
            project.to_path_buf(),
            hosted_pid,
            resume_identity.map(str::to_owned),
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
    last_offset: u64,
    cached_messages: Vec<ChatMessageDto>,
    last_status: String,
}

impl ClaudeChatAdapter {
    pub(crate) fn new(
        project: PathBuf,
        hosted_pid: i32,
        resume_identity: Option<String>,
    ) -> Self {
        Self {
            project,
            hosted_pid,
            resume_identity,
            known_session_id: None,
            transcript_path: None,
            last_offset: 0,
            cached_messages: Vec::new(),
            last_status: "uncertain".to_owned(),
        }
    }

    fn resolve_transcript(&mut self) -> Option<PathBuf> {
        if let Some(path) = &self.transcript_path {
            if path.is_file() {
                return Some(path.clone());
            }
        }

        let home = std::env::var_os("HOME").map(PathBuf::from)?;
        let project_slug = crate::agents::claude_project_dir(&self.project);
        let projects_dir = home.join(".claude").join("projects").join(&project_slug);

        // 1. If an explicit resume identity was given, verify that exact transcript exists
        if let Some(id) = &self.resume_identity {
            let candidate = projects_dir.join(format!("{id}.jsonl"));
            if candidate.is_file() {
                self.known_session_id = Some(id.clone());
                self.transcript_path = Some(candidate.clone());
                return Some(candidate);
            }
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

                    if matches_process {
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
        let home = std::env::var_os("HOME").map(PathBuf::from)?;
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
                return meta.get("status").and_then(|v| v.as_str()).map(str::to_owned);
            }
        }
        None
    }
}

impl AgentChatAdapter for ClaudeChatAdapter {
    fn poll_chat(&mut self, terminal_screen: &str) -> ChatStateUpdate {
        let transcript = self.resolve_transcript();

        if let Some(path) = transcript {
            if let Ok(mut file) = fs::File::open(&path) {
                let metadata = file.metadata().ok();
                let file_len = metadata.map(|m| m.len()).unwrap_or(0);

                if file_len < self.last_offset {
                    // Truncation or rotation: reset
                    self.last_offset = 0;
                    self.cached_messages.clear();
                }

                if file_len > self.last_offset {
                    if self.last_offset == 0 {
                        self.cached_messages.clear();
                    }
                    if file.seek(SeekFrom::Start(self.last_offset)).is_ok() {
                        let reader = BufReader::new(file);
                        let mut bytes_read = 0u64;

                        for line_result in reader.lines() {
                            let Ok(line) = line_result else { break };
                            bytes_read += line.len() as u64 + 1;

                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                                let is_meta = val.get("isMeta").and_then(|v| v.as_bool()).unwrap_or(false);
                                if is_meta {
                                    continue;
                                }

                                let entry_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
                                let uuid = val.get("uuid").and_then(|v| v.as_str()).unwrap_or("").to_owned();
                                let ts = parse_timestamp(&val);

                                if entry_type == "user" {
                                    if let Some(msg) = val.get("message") {
                                        let text = extract_clean_user_text(msg.get("content"));
                                        if is_real_user_message(&text) {
                                            self.cached_messages.push(ChatMessageDto {
                                                id: if uuid.is_empty() {
                                                    format!("user-{}", self.cached_messages.len())
                                                } else {
                                                    uuid
                                                },
                                                sender: "user".to_owned(),
                                                text,
                                                timestamp: ts,
                                            });
                                        }
                                    }
                                } else if entry_type == "assistant" {
                                    if let Some(msg) = val.get("message") {
                                        let text = extract_clean_assistant_text(msg.get("content"));
                                        if !text.is_empty() {
                                            self.cached_messages.push(ChatMessageDto {
                                                id: if uuid.is_empty() {
                                                    format!("agent-{}", self.cached_messages.len())
                                                } else {
                                                    uuid
                                                },
                                                sender: "agent".to_owned(),
                                                text,
                                                timestamp: ts,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                        self.last_offset += bytes_read;
                    }
                }
            }
        }

        // Determine agent state and safe input gating
        // 1. Is the terminal awaiting an approval or interactive menu?
        let is_awaiting_approval = terminal_screen.contains("[y/N]")
            || terminal_screen.contains("[Y/n]")
            || terminal_screen.contains("Do you want to run")
            || terminal_screen.contains("Allow this tool?")
            || terminal_screen.contains("Press Enter to approve")
            || terminal_screen.contains("Esc to cancel");

        let (agent_state, can_send_prompt) = if is_awaiting_approval {
            ("awaiting_approval".to_owned(), false)
        } else if self.known_session_id.is_some() {
            // Check status reported in session metadata
            let status = self.check_session_status();
            match status.as_deref() {
                Some("idle") => ("waiting".to_owned(), true),
                Some("running") | Some("working") => ("working".to_owned(), false),
                _ => {
                    // Fallback to checking the last recorded message
                    if let Some(last) = self.cached_messages.last() {
                        if last.sender == "user" {
                            ("working".to_owned(), false)
                        } else {
                            ("waiting".to_owned(), true)
                        }
                    } else {
                        ("waiting".to_owned(), true)
                    }
                }
            }
        } else {
            ("uncertain".to_owned(), false)
        };

        self.last_status = agent_state.clone();

        ChatStateUpdate {
            agent: "claude".to_owned(),
            agent_state,
            can_send_prompt,
            messages: self.cached_messages.clone(),
        }
    }
}

fn parse_timestamp(value: &serde_json::Value) -> u64 {
    if let Some(ms) = value.get("timestampMs").and_then(|v| v.as_u64()) {
        return ms;
    }
    if let Some(_iso) = value.get("timestamp").and_then(|v| v.as_str()) {
        // Simple approximate fallback if RFC3339 string without external crate
        return SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
    }
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn extract_clean_user_text(content: Option<&serde_json::Value>) -> String {
    let mut parts = Vec::new();
    if let Some(serde_json::Value::Array(items)) = content {
        for item in items {
            let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if item_type == "text" {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        parts.push(trimmed.to_owned());
                    }
                }
            }
        }
    } else if let Some(serde_json::Value::String(s)) = content {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed.to_owned());
        }
    }
    parts.join("\n\n")
}

fn extract_clean_assistant_text(content: Option<&serde_json::Value>) -> String {
    let mut parts = Vec::new();
    if let Some(serde_json::Value::Array(items)) = content {
        for item in items {
            let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
            // Strictly exclude private thinking / reasoning blocks per spec
            if item_type == "thinking" {
                continue;
            }
            if item_type == "text" {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        parts.push(trimmed.to_owned());
                    }
                }
            }
        }
    } else if let Some(serde_json::Value::String(s)) = content {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed.to_owned());
        }
    }
    parts.join("\n\n")
}

fn is_real_user_message(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.starts_with("<command-")
        || trimmed.starts_with("<local-command-")
        || trimmed.starts_with("<system-reminder>")
        || trimmed.starts_with("<user-instructions>")
        || trimmed.starts_with("<environment_context>")
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_clean_text_and_excludes_thinking() {
        let assistant_json = serde_json::json!({
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [
                    { "type": "thinking", "thinking": "Internal secret thought" },
                    { "type": "text", "text": "Here is the solution to your issue." }
                ]
            }
        });
        let text = extract_clean_assistant_text(assistant_json.pointer("/message/content"));
        assert_eq!(text, "Here is the solution to your issue.");
        assert!(!text.contains("Internal secret thought"));
    }

    #[test]
    fn filters_out_internal_system_tags() {
        assert!(!is_real_user_message("<command-name>/model</command-name>"));
        assert!(!is_real_user_message("<local-command-stdout>Set model</local-command-stdout>"));
        assert!(!is_real_user_message("<system-reminder>SessionStart</system-reminder>"));
        assert!(is_real_user_message("Please explain how the database works."));
    }

    #[test]
    fn detects_awaiting_approval_in_terminal() {
        let mut adapter = ClaudeChatAdapter::new(PathBuf::from("/tmp"), 12345, None);
        let screen = "Do you want to run `npm test`? [y/N]";
        let update = adapter.poll_chat(screen);
        assert_eq!(update.agent_state, "awaiting_approval");
        assert!(!update.can_send_prompt);
    }
}
