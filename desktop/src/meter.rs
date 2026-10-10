//! The context meter: how full an agent's context is, and how much of its rate limit is used.
//!
//! Read from the agent's own session log, by the same rule as `observe`: only named numbers, never
//! content. Claude Code records each reply's token usage but not its context window, so for Claude
//! the meter reports tokens and says the window is unknown rather than guessing a percentage. Codex
//! records its window and its rate limits, so those are reported as recorded.

use crate::json::json_number;
use crate::observe::{Record, RecordTail};
use serde::Serialize;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// How far back from the end of a log to look for the latest usage line.
const TAIL_BYTES: u64 = 512 * 1024;

#[derive(Serialize, Debug, Default, PartialEq)]
pub(crate) struct RateLimit {
    pub label: &'static str,
    pub used_percent: u8,
    pub resets_at: Option<u64>,
}

#[derive(Serialize, Debug, Default, PartialEq)]
pub(crate) struct Meter {
    /// Tokens in the context as of the agent's latest reply.
    pub tokens: Option<u64>,
    /// The context window, when the agent records it.
    pub window: Option<u64>,
    pub percent: Option<u8>,
    pub rate_limits: Vec<RateLimit>,
    /// Why something is missing, in plain words.
    pub note: Option<&'static str>,
}

/// Claude Code: the context of a reply is its prompt plus everything read from or written to cache.
pub(crate) fn claude_line(line: &str) -> Option<u64> {
    if !line.contains("\"usage\"") || !line.contains("\"cache_read_input_tokens\"") {
        return None;
    }
    let usage = &line[line.find("\"usage\"")?..];
    let n = |key| json_number(usage, key).unwrap_or(0) as u64;
    Some(n("input_tokens") + n("cache_read_input_tokens") + n("cache_creation_input_tokens"))
}

/// Codex: `token_count` events carry the last turn's usage, the window and the rate limits.
pub(crate) fn codex_line(line: &str) -> Option<Meter> {
    if !line.contains("\"token_count\"") {
        return None;
    }
    // Scope each read to its own object: "input_tokens" also appears in the running total.
    let last = &line[line.find("\"last_token_usage\"")?..];
    let tokens = json_number(last, "input_tokens")? as u64;
    let window = json_number(line, "model_context_window").map(|w| w as u64);
    let mut rate_limits = Vec::new();
    for (key, label) in [
        ("\"primary\"", "5-hour limit"),
        ("\"secondary\"", "weekly limit"),
    ] {
        if let Some(at) = line.find(key) {
            let scoped = &line[at..];
            if let Some(used) = json_number(scoped, "used_percent") {
                rate_limits.push(RateLimit {
                    label,
                    used_percent: used.min(100) as u8,
                    resets_at: json_number(scoped, "resets_at").map(|r| r as u64),
                });
            }
        }
    }
    Some(Meter {
        tokens: Some(tokens),
        percent: window
            .filter(|w| *w > 0)
            .map(|w| ((tokens * 100) / w).min(100) as u8),
        window,
        rate_limits,
        note: None,
    })
}

fn tail(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(TAIL_BYTES)))
        .ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Reads the latest usage from a log file.
pub(crate) fn from_log(record: Record, path: &Path) -> Meter {
    let Some(text) = tail(path) else {
        return Meter {
            note: Some("The agent's session log could not be read."),
            ..Meter::default()
        };
    };
    for line in text.lines().rev() {
        match record {
            Record::Claude => {
                if let Some(tokens) = claude_line(line) {
                    return Meter {
                        tokens: Some(tokens),
                        note: Some("Claude Code does not record its context window size, so only the token count is shown."),
                        ..Meter::default()
                    };
                }
            }
            Record::Codex => {
                if let Some(meter) = codex_line(line) {
                    return meter;
                }
            }
        }
    }
    Meter {
        note: Some("No reply recorded yet."),
        ..Meter::default()
    }
}

/// The meter for a hosted session: its exact log when the conversation id is known, else the log
/// the agent began after the session started.
pub(crate) fn for_session(
    record: Record,
    home: &Path,
    project: &Path,
    created_at_ms: u128,
    conversation_id: Option<&str>,
) -> Meter {
    let found = match conversation_id {
        Some(id) => RecordTail::find_existing(record, home, project, id),
        None => {
            // A few seconds of slack: the agent may create its log just before Verb records the start.
            let since = UNIX_EPOCH
                + Duration::from_millis(created_at_ms as u64)
                    .saturating_sub(Duration::from_secs(5));
            RecordTail::find(record, home, project, since.min(SystemTime::now()))
        }
    };
    match found {
        Some(tail) => from_log(record, tail.path()),
        None => Meter {
            note: Some("The agent has not started its session log yet."),
            ..Meter::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_context_is_prompt_plus_cache() {
        let line = r#"{"type":"assistant","message":{"model":"claude-x","content":[{"type":"text","text":"secret words"}],"usage":{"input_tokens":2,"cache_creation_input_tokens":489,"cache_read_input_tokens":721704,"output_tokens":432}}}"#;
        assert_eq!(claude_line(line), Some(722195));
        assert_eq!(
            claude_line(r#"{"type":"user","message":{"content":"hi"}}"#),
            None
        );
    }

    #[test]
    fn codex_reads_last_turn_window_and_rate_limits() {
        let line = r#"{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":535360327,"cached_input_tokens":1},"last_token_usage":{"input_tokens":185167,"cached_input_tokens":184064},"model_context_window":760000},"rate_limits":{"primary":{"used_percent":18.0,"window_minutes":300,"resets_at":1791479620},"secondary":{"used_percent":49.0,"window_minutes":10080,"resets_at":1791969024}}}}"#;
        let m = codex_line(line).unwrap();
        assert_eq!(m.tokens, Some(185167), "last turn, not the running total");
        assert_eq!(m.window, Some(760000));
        assert_eq!(m.percent, Some(24));
        assert_eq!(
            m.rate_limits,
            vec![
                RateLimit {
                    label: "5-hour limit",
                    used_percent: 18,
                    resets_at: Some(1791479620)
                },
                RateLimit {
                    label: "weekly limit",
                    used_percent: 49,
                    resets_at: Some(1791969024)
                },
            ]
        );
    }

    #[test]
    fn from_log_takes_the_latest_usage_and_explains_gaps() {
        let dir = std::env::temp_dir().join(format!("verb-meter-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.jsonl");
        std::fs::write(
            &path,
            concat!(
                r#"{"message":{"usage":{"input_tokens":1,"cache_read_input_tokens":100,"cache_creation_input_tokens":0}}}"#, "\n",
                r#"{"message":{"usage":{"input_tokens":1,"cache_read_input_tokens":5000,"cache_creation_input_tokens":20}}}"#, "\n",
                r#"{"type":"user","message":{"content":"x"}}"#, "\n",
            ),
        )
        .unwrap();
        let m = from_log(Record::Claude, &path);
        assert_eq!(m.tokens, Some(5021));
        assert!(
            m.percent.is_none() && m.note.is_some(),
            "no guessed percentage for Claude"
        );
        std::fs::write(&path, "{\"type\":\"user\"}\n").unwrap();
        assert_eq!(
            from_log(Record::Claude, &path).note,
            Some("No reply recorded yet.")
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod real_logs {
    use super::*;

    /// Manual check against this machine's real agent logs: `cargo test meter::real_logs -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn reads_this_machines_latest_logs() {
        let home = std::path::PathBuf::from(std::env::var("HOME").unwrap());
        let newest = |dir: std::path::PathBuf| {
            walk(&dir)
                .into_iter()
                .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
        };
        if let Some(p) = newest(home.join(".claude/projects")) {
            println!("claude: {:?}", from_log(Record::Claude, &p));
        }
        if let Some(p) = newest(home.join(".codex/sessions")) {
            println!("codex: {:?}", from_log(Record::Codex, &p));
        }
    }

    fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(walk(&p));
            } else if p.extension().is_some_and(|x| x == "jsonl") {
                out.push(p);
            }
        }
        out
    }
}
