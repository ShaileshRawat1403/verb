//! The agent stream: what an agent session said and did, read from the agent's own log, **only when
//! the person has turned it on for this project**.
//!
//! This is the one place Verb reads an agent record's *content*. `observe` and `meter` read structure
//! and numbers only, by construction, and they stay that way: everything else in Verb keeps working
//! without this module ever running. The stream exists because a person reviewing work wants the
//! conversation itself -- their request, the agent's answer, the files it read and the commands it
//! ran -- and the agent already writes all of it down on this machine.
//!
//! The rules, each enforced here rather than left to the caller:
//!
//! - **Off by default, per project.** `load(store).enabled` is false until the person turns it on, and
//!   `for_session` is only called after checking it.
//! - **Read on demand, never stored.** Each request reads the tail of the log and returns; nothing is
//!   cached, written to disk, logged, or added to an audit trail.
//! - **Local only.** The result goes to the browser that asked, over Verb's own authenticated API.
//!   It is never handed to a model or sent anywhere else.
//! - **Redacted.** Keys, tokens, private keys and `password=`-style values are replaced before the
//!   text leaves this module.
//! - **Bounded.** At most the last `TAIL_BYTES` of a log and the last `MAX_ITEMS` items; long text is
//!   cut with a marker.

use crate::observe::{Record, RecordTail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const TAIL_BYTES: u64 = 2 * 1024 * 1024;
const MAX_ITEMS: usize = 300;
const MAX_TEXT: usize = 6_000;
const MAX_TARGET: usize = 200;

#[derive(Serialize, Deserialize, Default, Debug)]
pub(crate) struct Settings {
    pub enabled: bool,
}

pub(crate) fn load(store: &Path) -> Settings {
    fs::read_to_string(store.join("stream.json"))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub(crate) fn save(store: &Path, settings: &Settings) -> Result<(), String> {
    fs::create_dir_all(store).map_err(|e| e.to_string())?;
    let raw = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(store.join("stream.json"), raw).map_err(|e| e.to_string())
}

/// One thing in the stream, in the order it happened.
#[derive(Serialize, Debug, PartialEq, Default)]
pub(crate) struct Item {
    /// `you`, `agent` or `step`.
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// For a step: the tool, in plain words (`Read`, `Edit`, `Bash`, ...).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// For a step: the file, command or pattern it acted on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub failed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
}

#[derive(Serialize, Debug, Default)]
pub(crate) struct Stream {
    pub items: Vec<Item>,
    pub note: Option<&'static str>,
}

// ------------------------------------------------------------------------------------- redaction

const SECRET_NAMES: [&str; 9] = [
    "token",
    "secret",
    "password",
    "passwd",
    "api_key",
    "apikey",
    "access_key",
    "private_key",
    "auth",
];

/// A credential prefix, the shortest run after it that counts, and the characters of that run.
type Pattern = (&'static str, usize, fn(char) -> bool);

/// Replaces anything that looks like a credential with `[redacted]`.
pub(crate) fn redact(text: &str) -> String {
    let mut out = redact_private_keys(text);
    let token = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    let upper = |c: char| c.is_ascii_uppercase() || c.is_ascii_digit();
    let prefixed: [Pattern; 13] = [
        ("github_pat_", 20, token),
        ("ghp_", 20, token),
        ("gho_", 20, token),
        ("ghs_", 20, token),
        ("ghu_", 20, token),
        ("sk-ant-", 20, token),
        ("sk-proj-", 20, token),
        ("xoxb-", 20, token),
        ("xoxp-", 20, token),
        ("glpat-", 20, token),
        ("AIza", 30, token),
        ("AKIA", 16, upper),
        ("sk-", 32, token),
    ];
    for (prefix, min, ok) in prefixed {
        out = replace_runs(&out, prefix, min, ok);
    }
    redact_assignments(&out)
}

fn redact_private_keys(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("-----BEGIN") {
        let tail = &rest[start..];
        let Some(end_marker) = tail.find("-----END") else {
            break;
        };
        let after_end = &tail[end_marker + 8..];
        let close = after_end
            .find("-----")
            .map(|i| i + 5)
            .unwrap_or(after_end.len());
        let block = &tail[..end_marker + 8 + close];
        out.push_str(&rest[..start]);
        if block.contains("PRIVATE KEY") {
            out.push_str("[private key redacted]");
        } else {
            out.push_str(block);
        }
        rest = &tail[end_marker + 8 + close..];
    }
    out.push_str(rest);
    out
}

fn replace_runs(text: &str, prefix: &str, min: usize, ok: fn(char) -> bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find(prefix) {
        let after = &rest[i + prefix.len()..];
        let run: usize = after
            .chars()
            .take_while(|c| ok(*c))
            .map(char::len_utf8)
            .sum();
        let starts_word = rest[..i]
            .chars()
            .last()
            .is_none_or(|c| !c.is_ascii_alphanumeric());
        if run >= min && starts_word {
            out.push_str(&rest[..i]);
            out.push_str("[redacted]");
            rest = &after[run..];
        } else {
            out.push_str(&rest[..i + prefix.len()]);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// `PASSWORD=hunter22`, `"api_key": "abc..."`, `--token xyz`: the value goes, the name stays.
fn redact_assignments(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'=' || c == b':' {
            // The name before the separator (skipping a closing quote).
            let mut j = i;
            if j > 0 && (bytes[j - 1] == b'"' || bytes[j - 1] == b'\'') {
                j -= 1;
            }
            let mut start = j;
            while start > 0
                && (bytes[start - 1].is_ascii_alphanumeric()
                    || bytes[start - 1] == b'_'
                    || bytes[start - 1] == b'-')
            {
                start -= 1;
            }
            let name = text[start..j].to_ascii_lowercase();
            let secretish = !name.is_empty() && SECRET_NAMES.iter().any(|s| name.contains(s));
            if secretish {
                // The value after the separator (skipping spaces and an opening quote).
                let mut v = i + 1;
                while v < bytes.len() && bytes[v] == b' ' {
                    v += 1;
                }
                if v < bytes.len() && (bytes[v] == b'"' || bytes[v] == b'\'') {
                    v += 1;
                }
                let mut end = v;
                while end < bytes.len()
                    && !matches!(
                        bytes[end],
                        b' ' | b'"' | b'\'' | b'\n' | b',' | b'}' | b';' | b'&'
                    )
                {
                    end += 1;
                }
                if end - v >= 6 && text.is_char_boundary(v) && text.is_char_boundary(end) {
                    out.push_str(&text[copied..v]);
                    out.push_str("[redacted]");
                    copied = end;
                    i = end;
                    continue;
                }
            }
        }
        i += 1;
    }
    out.push_str(&text[copied..]);
    out
}

// ---------------------------------------------------------------------------------- helpers

fn cut(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut s: String = text.chars().take(max).collect();
    s.push_str(" …");
    s
}

fn clean(text: &str, max: usize) -> String {
    redact(&cut(text, max))
}

/// A path inside the project is shown relative to it.
fn relative(path: &str, project: &Path) -> String {
    let root = project.to_string_lossy();
    path.strip_prefix(&format!("{}/", root.trim_end_matches('/')))
        .unwrap_or(path)
        .to_owned()
}

fn first_line(text: &str) -> &str {
    text.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
}

/// Text Verb never shows as something the person typed: harness wrappers and injected context.
fn is_wrapper(text: &str) -> bool {
    let t = text.trim_start();
    t.is_empty()
        || t.starts_with('<')
        || t.starts_with("# AGENTS.md instructions")
        || t.starts_with("[Request interrupted")
        || t.starts_with("Caveat: The messages below")
}

// ------------------------------------------------------------------------------------- Claude

/// Claude Code's `~/.claude/projects/<project>/<session>.jsonl`.
pub(crate) fn claude_items(lines: &[&str], project: &Path) -> Vec<Item> {
    let mut items = Vec::new();
    let mut steps: HashMap<String, usize> = HashMap::new();
    for line in lines {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v["isSidechain"].as_bool() == Some(true) || v["isMeta"].as_bool() == Some(true) {
            continue;
        }
        let at = v["timestamp"].as_str().map(str::to_owned);
        let content = &v["message"]["content"];
        match v["type"].as_str() {
            Some("user") => {
                if let Some(text) = content.as_str() {
                    if !is_wrapper(text) {
                        items.push(Item {
                            kind: "you",
                            text: Some(clean(text, MAX_TEXT)),
                            at,
                            ..Item::default()
                        });
                    }
                    continue;
                }
                let mut said = Vec::new();
                for block in content.as_array().into_iter().flatten() {
                    match block["type"].as_str() {
                        Some("text") => {
                            if let Some(t) = block["text"].as_str().filter(|t| !is_wrapper(t)) {
                                said.push(t.to_owned());
                            }
                        }
                        Some("tool_result") if block["is_error"].as_bool() == Some(true) => {
                            if let Some(&i) =
                                block["tool_use_id"].as_str().and_then(|id| steps.get(id))
                            {
                                items[i].failed = true;
                            }
                        }
                        _ => {}
                    }
                }
                if !said.is_empty() {
                    items.push(Item {
                        kind: "you",
                        text: Some(clean(&said.join("\n\n"), MAX_TEXT)),
                        at,
                        ..Item::default()
                    });
                }
            }
            Some("assistant") => {
                for block in content.as_array().into_iter().flatten() {
                    match block["type"].as_str() {
                        Some("text") => {
                            if let Some(t) = block["text"].as_str().filter(|t| !t.trim().is_empty())
                            {
                                items.push(Item {
                                    kind: "agent",
                                    text: Some(clean(t, MAX_TEXT)),
                                    at: at.clone(),
                                    ..Item::default()
                                });
                            }
                        }
                        Some("tool_use") => {
                            let name = block["name"].as_str().unwrap_or("Tool");
                            let input = &block["input"];
                            let target = claude_target(name, input, project);
                            if let Some(id) = block["id"].as_str() {
                                steps.insert(id.to_owned(), items.len());
                            }
                            items.push(Item {
                                kind: "step",
                                tool: Some(name.to_owned()),
                                target: target.map(|t| clean(&t, MAX_TARGET)),
                                at: at.clone(),
                                ..Item::default()
                            });
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    items
}

fn claude_target(name: &str, input: &Value, project: &Path) -> Option<String> {
    let s = |key: &str| input[key].as_str().map(str::to_owned);
    match name {
        "Read" | "Write" | "Edit" | "MultiEdit" => s("file_path").map(|p| relative(&p, project)),
        "NotebookEdit" => s("notebook_path").map(|p| relative(&p, project)),
        "Bash" => s("command").map(|c| first_line(&c).to_owned()),
        "Grep" | "Glob" => s("pattern"),
        "WebFetch" => s("url"),
        "WebSearch" => s("query"),
        "Task" | "Agent" => s("description"),
        _ => None,
    }
}

// -------------------------------------------------------------------------------------- Codex

/// Codex's `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`.
pub(crate) fn codex_items(lines: &[&str], project: &Path) -> Vec<Item> {
    let mut items = Vec::new();
    let mut steps: HashMap<String, usize> = HashMap::new();
    for line in lines {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v["type"].as_str() != Some("response_item") {
            continue;
        }
        let at = v["timestamp"].as_str().map(str::to_owned);
        let p = &v["payload"];
        match p["type"].as_str() {
            Some("message") => {
                let role = p["role"].as_str();
                let kind = match role {
                    Some("user") => "you",
                    Some("assistant") => "agent",
                    _ => continue,
                };
                let text: Vec<&str> = p["content"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|c| c["text"].as_str())
                    .filter(|t| !is_wrapper(t))
                    .collect();
                if !text.is_empty() {
                    items.push(Item {
                        kind,
                        text: Some(clean(&text.join("\n\n"), MAX_TEXT)),
                        at,
                        ..Item::default()
                    });
                }
            }
            Some("function_call") | Some("custom_tool_call") => {
                let name = p["name"].as_str().unwrap_or("tool");
                let (tool, target) = codex_step(name, p, project);
                if let Some(id) = p["call_id"].as_str() {
                    steps.insert(id.to_owned(), items.len());
                }
                items.push(Item {
                    kind: "step",
                    tool: Some(tool),
                    target: target.map(|t| clean(&t, MAX_TARGET)),
                    at,
                    ..Item::default()
                });
            }
            Some("function_call_output") | Some("custom_tool_call_output") => {
                if let (Some(&i), Some(code)) = (
                    p["call_id"].as_str().and_then(|id| steps.get(id)),
                    exit_code(&p["output"]),
                ) {
                    items[i].failed = code != 0;
                }
            }
            _ => {}
        }
    }
    items
}

fn codex_step(name: &str, p: &Value, project: &Path) -> (String, Option<String>) {
    match name {
        "shell" | "exec_command" | "local_shell" | "shell_command" => {
            let args: Value = p["arguments"]
                .as_str()
                .and_then(|a| serde_json::from_str(a).ok())
                .unwrap_or(Value::Null);
            let command = match &args["command"] {
                Value::Array(parts) => {
                    let parts: Vec<&str> = parts.iter().filter_map(Value::as_str).collect();
                    // ["bash", "-lc", "the command"] is the command, not bash.
                    match parts.as_slice() {
                        [_, flag, cmd] if flag.starts_with('-') => Some(cmd.to_string()),
                        _ => Some(parts.join(" ")),
                    }
                }
                Value::String(s) => Some(s.clone()),
                _ => args["cmd"].as_str().map(str::to_owned),
            };
            (
                "Bash".to_owned(),
                command.map(|c| first_line(&c).to_owned()),
            )
        }
        "apply_patch" => {
            let input = p["input"].as_str().unwrap_or("");
            let files: Vec<String> = input
                .lines()
                .filter_map(|l| {
                    ["*** Update File: ", "*** Add File: ", "*** Delete File: "]
                        .iter()
                        .find_map(|m| l.strip_prefix(m))
                })
                .map(|f| relative(f.trim(), project))
                .collect();
            (
                "Edit".to_owned(),
                Some(files.join(", ")).filter(|f| !f.is_empty()),
            )
        }
        // Newer Codex runs its tools from a small script: `text(await tools.exec_command({cmd: "…"}))`.
        "exec" => {
            let script = p["input"].as_str().unwrap_or("");
            match script_command(script) {
                Some(cmd) => ("Bash".to_owned(), Some(first_line(&cmd).to_owned())),
                None => ("Script".to_owned(), None),
            }
        }
        other => (other.to_owned(), None),
    }
}

/// The first `cmd:` string literal in a Codex exec script, unescaped.
fn script_command(script: &str) -> Option<String> {
    let at = script.find("exec_command(")?;
    let rest = &script[at..];
    let after = &rest[rest.find("cmd")? + 3..];
    let after = after.trim_start().strip_prefix(':')?.trim_start();
    let quote = after
        .chars()
        .next()
        .filter(|c| matches!(c, '"' | '\'' | '`'))?;
    let mut out = String::new();
    let mut chars = after[1..].chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                other => out.push(other),
            },
            c if c == quote => return Some(out),
            c => out.push(c),
        }
    }
    None
}

/// The exit code a Codex tool output reports, in any of the shapes it uses.
fn exit_code(output: &Value) -> Option<i64> {
    let text = match output {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    if let Ok(parsed) = serde_json::from_str::<Value>(&text) {
        if let Some(code) = parsed["metadata"]["exit_code"].as_i64() {
            return Some(code);
        }
    }
    for marker in ["Exit code: ", "exit_code\":", "Process exited with code "] {
        if let Some(i) = text.find(marker) {
            let digits: String = text[i + marker.len()..]
                .trim_start()
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '-')
                .collect();
            if let Ok(code) = digits.parse() {
                return Some(code);
            }
        }
    }
    None
}

// ------------------------------------------------------------------------------------ reading

fn tail_lines(path: &Path) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let start = len.saturating_sub(TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    // Starting mid-file means the first line is a fragment.
    Some(if start > 0 {
        text.split_once('\n')
            .map(|(_, rest)| rest.to_owned())
            .unwrap_or_default()
    } else {
        text
    })
}

/// The stream of one agent session. Call only after checking `load(store).enabled`.
pub(crate) fn for_session(
    record: Record,
    home: &Path,
    project: &Path,
    created_at_ms: u128,
    conversation_id: Option<&str>,
) -> Stream {
    let found = match conversation_id {
        Some(id) => RecordTail::find_existing(record, home, project, id),
        None => {
            let since = UNIX_EPOCH
                + Duration::from_millis(created_at_ms as u64)
                    .saturating_sub(Duration::from_secs(5));
            RecordTail::find(record, home, project, since.min(SystemTime::now()))
        }
    };
    let Some(tail) = found else {
        return Stream {
            items: Vec::new(),
            note: Some("The agent has not started its session log yet."),
        };
    };
    let Some(text) = tail_lines(tail.path()) else {
        return Stream {
            items: Vec::new(),
            note: Some("The agent's session log could not be read."),
        };
    };
    let lines: Vec<&str> = text.lines().collect();
    let mut items = match record {
        Record::Claude => claude_items(&lines, project),
        Record::Codex => codex_items(&lines, project),
    };
    if items.len() > MAX_ITEMS {
        items.drain(..items.len() - MAX_ITEMS);
    }
    Stream { items, note: None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_are_redacted_and_ordinary_text_is_not() {
        let fake_gh = format!("ghp_{}", "a".repeat(36));
        let text = format!(
            "use {fake_gh} then PASSWORD=hunter2222 and \"api_key\": \"abcdef123456\" but keep token count and auth: ok"
        );
        let out = redact(&text);
        assert!(!out.contains(&fake_gh));
        assert!(!out.contains("hunter2222"));
        assert!(!out.contains("abcdef123456"));
        assert!(out.contains("PASSWORD=[redacted]"));
        assert!(
            out.contains("token count"),
            "the word alone is not a secret"
        );
        assert!(out.contains("auth: ok"), "short values are left alone");
        let key = "-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n-----END OPENSSH PRIVATE KEY-----";
        assert_eq!(redact(key), "[private key redacted]");
        assert_eq!(redact("sk-short is fine"), "sk-short is fine");
    }

    #[test]
    fn claude_records_become_requests_replies_and_steps() {
        let p = Path::new("/w/shop");
        let lines = [
            r#"{"type":"user","message":{"role":"user","content":"Make the expired-link message clear"},"timestamp":"t1"}"#,
            r#"{"type":"user","isMeta":true,"message":{"role":"user","content":"injected"}}"#,
            r#"{"type":"user","message":{"role":"user","content":"<command-name>/clear</command-name>"}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"hmm"},{"type":"tool_use","id":"a","name":"Read","input":{"file_path":"/w/shop/src/auth/link.ts"}}]},"timestamp":"t2"}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"b","name":"Bash","input":{"command":"npm test\n# again"}}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"b","is_error":true,"content":"FAIL"}]}}"#,
            r#"{"type":"assistant","isSidechain":true,"message":{"content":[{"type":"text","text":"subagent"}]}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Fixed it."}]}}"#,
        ];
        let items = claude_items(&lines, p);
        let summary: Vec<_> = items
            .iter()
            .map(|i| {
                (
                    i.kind,
                    i.text.as_deref().or(i.target.as_deref()).unwrap_or(""),
                    i.failed,
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                ("you", "Make the expired-link message clear", false),
                ("step", "src/auth/link.ts", false),
                ("step", "npm test", true),
                ("agent", "Fixed it.", false),
            ]
        );
    }

    #[test]
    fn codex_records_become_requests_replies_and_steps() {
        let p = Path::new("/w/shop");
        let lines = [
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<environment_context>x</environment_context>"}]}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"run the tests"}]}}"#,
            r#"{"type":"response_item","payload":{"type":"function_call","name":"shell","call_id":"c1","arguments":"{\"command\":[\"bash\",\"-lc\",\"cargo test\"]}"}}"#,
            r#"{"type":"response_item","payload":{"type":"function_call_output","call_id":"c1","output":"{\"output\":\"x\",\"metadata\":{\"exit_code\":101}}"}}"#,
            r#"{"type":"response_item","payload":{"type":"custom_tool_call","name":"apply_patch","call_id":"c2","input":"*** Begin Patch\n*** Update File: /w/shop/src/lib.rs\n@@\n*** End Patch"}}"#,
            r#"{"type":"response_item","payload":{"type":"custom_tool_call_output","call_id":"c2","output":"Exit code: 0\nok"}}"#,
            r#"{"type":"response_item","payload":{"type":"reasoning","summary":[]}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Tests fixed."}]}}"#,
        ];
        let items = codex_items(&lines, p);
        let summary: Vec<_> = items
            .iter()
            .map(|i| {
                (
                    i.kind,
                    i.tool.as_deref().unwrap_or(""),
                    i.text.as_deref().or(i.target.as_deref()).unwrap_or(""),
                    i.failed,
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                ("you", "", "run the tests", false),
                ("step", "Bash", "cargo test", true),
                ("step", "Edit", "src/lib.rs", false),
                ("agent", "", "Tests fixed.", false),
            ]
        );
    }

    #[test]
    fn codex_exec_scripts_show_their_command() {
        assert_eq!(
            script_command(
                r#"text(await tools.exec_command({cmd: "git status --short", workdir: "/w"}))"#
            )
            .as_deref(),
            Some("git status --short")
        );
        assert_eq!(
            script_command(r#"await tools.exec_command({ cmd: 'echo \'hi\'\nls' })"#).as_deref(),
            Some("echo 'hi'\nls")
        );
        assert_eq!(script_command("text(await tools.web__run({}))"), None);
    }

    #[test]
    fn settings_default_to_off() {
        let dir = std::env::temp_dir().join(format!("verb-stream-{}", std::process::id()));
        assert!(!load(&dir).enabled);
        save(&dir, &Settings { enabled: true }).unwrap();
        assert!(load(&dir).enabled);
        let _ = fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod real_logs_probe {
    #[test]
    #[ignore]
    fn probe() {
        let home = std::path::PathBuf::from(std::env::var("HOME").unwrap());
        for (label, path, record) in [
            (
                "claude",
                std::env::var("PROBE_CLAUDE"),
                crate::observe::Record::Claude,
            ),
            (
                "codex",
                std::env::var("PROBE_CODEX"),
                crate::observe::Record::Codex,
            ),
        ] {
            // Run by hand against real logs: PROBE_CLAUDE=<file> PROBE_CODEX=<file>. Prints counts only.
            let Ok(path) = path else { continue };
            let text = super::tail_lines(std::path::Path::new(&path)).unwrap();
            let lines: Vec<&str> = text.lines().collect();
            let items = match record {
                crate::observe::Record::Claude => super::claude_items(&lines, &home),
                crate::observe::Record::Codex => super::codex_items(&lines, &home),
            };
            let count = |k| items.iter().filter(|i| i.kind == k).count();
            let failed = items.iter().filter(|i| i.failed).count();
            let mut tools: Vec<_> = items.iter().filter_map(|i| i.tool.clone()).collect();
            tools.sort();
            tools.dedup();
            println!(
                "{label}: you={} agent={} step={} failed={} tools={:?}",
                count("you"),
                count("agent"),
                count("step"),
                failed,
                tools
            );
        }
    }
}
