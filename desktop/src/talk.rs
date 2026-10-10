//! Talk: a conversation with Antigravity that Verb drives through agy's own JSON mode.
//!
//! agy's terminal UI keeps its conversation in a private binary store, so Verb cannot show it as a
//! stream. agy's print mode, though, speaks documented NDJSON (`--output-format stream-json`): an
//! `init` event with the conversation id, `step_update` events for the person's input, the agent's
//! reply (streamed as `text_delta`s) and each tool call, and a `result`. Verb runs one turn per
//! message and continues the same conversation with `--conversation <id>`.
//!
//! What headless agy cannot do is ask permission: anything that needs it is auto-denied (tested with
//! and without `--sandbox`). Verb never works around that -- no `--dangerously-skip-permissions`, no
//! allow-rules written into the person's settings. Instead Talk runs in agy's `plan` mode, frames each
//! turn to read with agy's own read tools inside the project, and turns every refusal into an item
//! that offers to continue the *same* conversation in agy's terminal, where the person approves each
//! action (`agy --conversation <id>`).
//!
//! Content handling follows the agent stream's rules: held in memory for this Verb process only,
//! never written to disk or the audit trail (the audit trail records only that a talk started),
//! redacted before it is shown, and bounded.

use crate::transcript::{redact, Item};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

const MAX_ITEMS: usize = 400;
const MAX_TEXT: usize = 12_000;
const MAX_MESSAGE: usize = 8_000;

/// The framing in front of every Talk turn. Plain and short: agy reads it as the person's words.
const FRAME: &str = "You are talking with the user through Verb, inside the project folder (your \
working directory). In this conversation, read with view_file, list_dir, grep_search and \
find_by_name, and stay inside the project folder. Do not use run_command or edit files here: if \
something needs a command or an edit, say exactly what and why, and the user will continue in the \
terminal to approve it.";

#[derive(Serialize, Debug, Default)]
pub(crate) struct TalkView {
    pub id: String,
    pub spec_id: Option<String>,
    pub conversation_id: Option<String>,
    pub working: bool,
    pub items: Vec<Item>,
    pub tokens: Option<u64>,
}

#[derive(Default)]
struct Talk {
    spec_id: Option<String>,
    conversation_id: Option<String>,
    working: bool,
    items: Vec<Item>,
    /// step_index of agy's current turn → index in `items`.
    steps: HashMap<i64, usize>,
    tokens: Option<u64>,
    child: Option<Arc<Mutex<Option<Child>>>>,
    /// Set when the person stopped the running turn.
    stopped: bool,
}

#[derive(Clone, Default)]
pub(crate) struct Talks(Arc<Mutex<HashMap<String, Talk>>>);

impl Talks {
    pub(crate) fn create(&self, id: String, spec_id: Option<String>) {
        self.0.lock().unwrap().insert(
            id,
            Talk {
                spec_id,
                ..Talk::default()
            },
        );
    }

    pub(crate) fn view(&self, id: &str) -> Option<TalkView> {
        let talks = self.0.lock().unwrap();
        let t = talks.get(id)?;
        Some(TalkView {
            id: id.to_owned(),
            spec_id: t.spec_id.clone(),
            conversation_id: t.conversation_id.clone(),
            working: t.working,
            items: t.items.iter().map(clone_item).collect(),
            tokens: t.tokens,
        })
    }

    pub(crate) fn list(&self) -> Vec<TalkView> {
        let ids: Vec<String> = self.0.lock().unwrap().keys().cloned().collect();
        ids.iter().filter_map(|id| self.view(id)).collect()
    }

    pub(crate) fn remove(&self, id: &str) {
        self.stop(id);
        self.0.lock().unwrap().remove(id);
    }

    /// Ends a running turn. agy keeps whatever it already recorded in the conversation.
    pub(crate) fn stop(&self, id: &str) {
        let child = {
            let mut talks = self.0.lock().unwrap();
            let t = talks.get_mut(id);
            t.and_then(|t| {
                t.stopped = t.working;
                t.child.clone()
            })
        };
        if let Some(child) = child {
            if let Some(c) = child.lock().unwrap().as_mut() {
                let _ = c.kill();
            }
        }
    }

    /// Starts one turn with `message`. `program` is `agy` (or a stand-in in tests).
    pub(crate) fn send(
        &self,
        id: &str,
        project: &Path,
        message: &str,
        program: &str,
    ) -> Result<(), String> {
        let message = message.trim();
        if message.is_empty() {
            return Err("write a message first".to_owned());
        }
        if message.len() > MAX_MESSAGE {
            return Err("keep a message under 8000 characters".to_owned());
        }
        let conversation = {
            let mut talks = self.0.lock().unwrap();
            let t = talks.get_mut(id).ok_or("no such talk")?;
            if t.working {
                return Err("Antigravity is still answering; wait or stop it first".to_owned());
            }
            t.working = true;
            t.steps.clear();
            push(
                t,
                Item {
                    kind: "you",
                    text: Some(cut(message, MAX_TEXT)),
                    ..Item::default()
                },
            );
            t.conversation_id.clone()
        };
        let mut command = Command::new(program);
        command
            .args(["--mode", "plan", "--output-format", "stream-json"])
            // Headless agy does not treat its working directory as its workspace: without this it
            // guessed paths (~/Desktop, /tmp/specs) and every read of the project was refused.
            .arg("--add-dir")
            .arg(project)
            .current_dir(project)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(c) = &conversation {
            command.args(["--conversation", c]);
        }
        command.arg(format!("-p={FRAME}\n\n{message}"));
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(e) => {
                let mut talks = self.0.lock().unwrap();
                if let Some(t) = talks.get_mut(id) {
                    t.working = false;
                    push(t, note(format!(
                        "Verb could not start Antigravity ({e}). Is the `agy` command installed on this machine?"
                    )));
                }
                return Err("could not start Antigravity".to_owned());
            }
        };
        let stdout = child.stdout.take().ok_or("no output from Antigravity")?;
        let stderr = child.stderr.take();
        let handle = Arc::new(Mutex::new(Some(child)));
        if let Some(t) = self.0.lock().unwrap().get_mut(id) {
            t.child = Some(Arc::clone(&handle));
        }
        let talks = self.clone();
        let id = id.to_owned();
        let project = project.to_path_buf();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                let Ok(event) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if let Some(t) = talks.0.lock().unwrap().get_mut(&id) {
                    apply(t, &event, &project);
                }
            }
            let mut err = String::new();
            if let Some(mut s) = stderr {
                let _ = s.read_to_string(&mut err);
            }
            let status = handle
                .lock()
                .unwrap()
                .take()
                .and_then(|mut c| c.wait().ok());
            if let Some(t) = talks.0.lock().unwrap().get_mut(&id) {
                t.working = false;
                t.child = None;
                if std::mem::take(&mut t.stopped) {
                    push(t, note("You stopped Antigravity.".to_owned()));
                    return;
                }
                let answered = t.items.last().is_some_and(|i| i.kind != "you");
                if !answered {
                    let why = first_line(&err);
                    push(
                        t,
                        note(match status {
                            Some(s) if s.success() && why.is_empty() => {
                                "Antigravity finished without a reply.".to_owned()
                            }
                            _ if why.is_empty() => "The turn was stopped.".to_owned(),
                            _ => format!("Antigravity stopped: {why}"),
                        }),
                    );
                }
            }
        });
        Ok(())
    }
}

fn clone_item(i: &Item) -> Item {
    Item {
        kind: i.kind,
        text: i.text.clone(),
        tool: i.tool.clone(),
        target: i.target.clone(),
        failed: i.failed,
        at: i.at.clone(),
    }
}

fn push(t: &mut Talk, item: Item) {
    t.items.push(item);
    if t.items.len() > MAX_ITEMS {
        let extra = t.items.len() - MAX_ITEMS;
        t.items.drain(..extra);
        t.steps.retain(|_, i| *i >= extra);
        for i in t.steps.values_mut() {
            *i -= extra;
        }
    }
}

fn note(text: String) -> Item {
    Item {
        kind: "note",
        text: Some(text),
        ..Item::default()
    }
}

fn cut(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut s: String = text.chars().take(max).collect();
    s.push_str(" …");
    s
}

fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .chars()
        .take(400)
        .collect()
}

/// agy's tool names in the words the stream already uses, and the parameter that says what it acted on.
fn tool_words(name: &str, params: &Value, project: &Path) -> (String, Option<String>) {
    let s = |k: &str| params[k].as_str().map(str::to_owned);
    let rel = |p: String| {
        let root = format!("{}/", project.to_string_lossy().trim_end_matches('/'));
        p.strip_prefix(&root).map(str::to_owned).unwrap_or(p)
    };
    let first = |text: String| text.lines().next().unwrap_or("").trim().to_owned();
    match name {
        "run_command" => ("Bash".to_owned(), s("CommandLine").map(first)),
        "view_file" => (
            "Read".to_owned(),
            s("AbsolutePath").or(s("FilePath")).map(rel),
        ),
        "list_dir" => (
            "Listed".to_owned(),
            s("DirectoryPath").or(s("AbsolutePath")).map(rel),
        ),
        "grep_search" => ("Grep".to_owned(), s("Query").or(s("Pattern"))),
        "find_by_name" => ("Glob".to_owned(), s("Pattern").or(s("SearchDirectory"))),
        "write_to_file" | "replace_file_content" | "multi_replace_file_content" | "sed_file" => (
            "Edit".to_owned(),
            s("TargetFile")
                .or(s("AbsolutePath"))
                .or(s("FilePath"))
                .map(rel),
        ),
        "search_web" => ("WebSearch".to_owned(), s("query").or(s("Query"))),
        "read_url_content" => ("WebFetch".to_owned(), s("Url").or(s("url"))),
        other => (other.to_owned(), None),
    }
}

/// Applies one agy stream-json event to a talk.
fn apply(t: &mut Talk, event: &Value, project: &Path) {
    match event["event"].as_str() {
        Some("init") => {
            if let Some(c) = event["conversation_id"].as_str() {
                t.conversation_id = Some(c.to_owned());
            }
        }
        Some("step_update") => {
            let su = &event["step_update"];
            let index = su["step_index"].as_i64().unwrap_or(-1);
            let state = su["state"].as_str().unwrap_or("");
            match su["step_type"].as_str() {
                Some("agent_response") => {
                    let delta = su["text_delta"].as_str().unwrap_or("");
                    match t.steps.get(&index).copied() {
                        Some(i) => {
                            let text = t.items[i].text.get_or_insert_with(String::new);
                            if text.chars().count() < MAX_TEXT {
                                text.push_str(&redact(delta));
                            }
                        }
                        None if !delta.is_empty() => {
                            t.steps.insert(index, t.items.len());
                            push(
                                t,
                                Item {
                                    kind: "agent",
                                    text: Some(redact(delta)),
                                    ..Item::default()
                                },
                            );
                        }
                        None => {}
                    }
                }
                Some("tool") => {
                    let info = &su["tool_info"];
                    let name = su["tool_name"]
                        .as_str()
                        .or(info["name"].as_str())
                        .unwrap_or("tool");
                    let (tool, target) = tool_words(name, &info["parameters"], project);
                    let i = match t.steps.get(&index).copied() {
                        Some(i) => i,
                        None => {
                            t.steps.insert(index, t.items.len());
                            push(
                                t,
                                Item {
                                    kind: "step",
                                    tool: Some(tool.clone()),
                                    target: target.as_deref().map(|x| redact(&cut(x, 200))),
                                    ..Item::default()
                                },
                            );
                            t.items.len() - 1
                        }
                    };
                    if state == "ERROR" {
                        t.items[i].failed = true;
                        let message = info["error"]["message"].as_str().unwrap_or("");
                        // A read outside the project is agy guessing or reaching for its own files;
                        // approving it in the terminal would not help, so it stays a failed step.
                        let reads = matches!(tool.as_str(), "Read" | "Listed" | "Glob" | "Grep");
                        let outside = target.as_deref().is_some_and(|p| p.starts_with('/'));
                        if message.contains("permission") && !(reads && outside) {
                            push(
                                t,
                                Item {
                                    kind: "permission",
                                    tool: Some(tool),
                                    target: target.as_deref().map(|x| redact(&cut(x, 200))),
                                    ..Item::default()
                                },
                            );
                        }
                    }
                }
                _ => {}
            }
        }
        Some("result") => {
            let r = &event["result"];
            if let Some(c) = r["conversation_id"].as_str() {
                t.conversation_id = Some(c.to_owned());
            }
            t.tokens = r["usage"]["total_tokens"].as_u64().or(t.tokens);
            // When nothing streamed, the final response is the reply.
            let streamed = t
                .items
                .iter()
                .rev()
                .take_while(|i| i.kind != "you")
                .any(|i| i.kind == "agent");
            if let Some(text) = r["response"]
                .as_str()
                .filter(|s| !s.trim().is_empty() && !streamed)
            {
                push(
                    t,
                    Item {
                        kind: "agent",
                        text: Some(redact(&cut(text, MAX_TEXT))),
                        ..Item::default()
                    },
                );
            }
            if r["status"].as_str() == Some("ERROR") {
                if let Some(e) = r["error"].as_str().filter(|e| !e.is_empty()) {
                    push(
                        t,
                        note(format!("Antigravity reported an error: {}", first_line(e))),
                    );
                }
            }
        }
        _ => {}
    }
}

/// The program that runs Antigravity: `VERB_AGY_BIN` in tests, otherwise `agy` on the PATH.
pub(crate) fn program() -> PathBuf {
    std::env::var_os("VERB_AGY_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("agy"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(lines: &[&str]) -> Talk {
        let mut t = Talk::default();
        for l in lines {
            apply(
                &mut t,
                &serde_json::from_str(l).unwrap(),
                Path::new("/w/shop"),
            );
        }
        t
    }

    #[test]
    fn a_turn_becomes_steps_a_streamed_reply_and_a_conversation_id() {
        let t = run(&[
            r#"{"event":"init","conversation_id":"c1","init":{"permission_mode":"request-review"}}"#,
            r#"{"event":"step_update","step_update":{"step_index":0,"state":"DONE","step_type":"user_input"}}"#,
            r#"{"event":"step_update","step_update":{"step_index":1,"state":"ACTIVE","step_type":"tool","tool_name":"view_file","tool_info":{"name":"view_file","parameters":{"AbsolutePath":"/w/shop/greet.js"}}}}"#,
            r#"{"event":"step_update","step_update":{"step_index":1,"state":"DONE","step_type":"tool","tool_name":"view_file","tool_info":{"name":"view_file","parameters":{"AbsolutePath":"/w/shop/greet.js"}}}}"#,
            r#"{"event":"step_update","step_update":{"step_index":2,"state":"ACTIVE","step_type":"agent_response","text_delta":"It returns "}}"#,
            r#"{"event":"step_update","step_update":{"step_index":2,"state":"DONE","step_type":"agent_response","text_delta":"'Hello ' + name."}}"#,
            r#"{"event":"result","result":{"conversation_id":"c1","status":"SUCCESS","response":"It returns 'Hello ' + name.","usage":{"total_tokens":1234}}}"#,
        ]);
        assert_eq!(t.conversation_id.as_deref(), Some("c1"));
        assert_eq!(t.tokens, Some(1234));
        let kinds: Vec<_> = t.items.iter().map(|i| i.kind).collect();
        assert_eq!(
            kinds,
            ["step", "agent"],
            "the final response is not repeated once it streamed"
        );
        assert_eq!(t.items[0].target.as_deref(), Some("greet.js"));
        assert_eq!(
            t.items[1].text.as_deref(),
            Some("It returns 'Hello ' + name.")
        );
    }

    #[test]
    fn a_refused_command_becomes_an_offer_to_continue_in_the_terminal() {
        let t = run(&[
            r#"{"event":"step_update","step_update":{"step_index":3,"state":"ACTIVE","step_type":"tool","tool_name":"run_command","tool_info":{"parameters":{"CommandLine":"node -e \"console.log(1)\""}}}}"#,
            r#"{"event":"step_update","step_update":{"step_index":3,"state":"ERROR","step_type":"tool","tool_name":"run_command","tool_info":{"parameters":{"CommandLine":"node -e \"console.log(1)\""},"error":{"type":"TOOL_ERROR","message":"permission check failed for unsandboxed ..."}}}}"#,
        ]);
        let kinds: Vec<_> = t.items.iter().map(|i| (i.kind, i.failed)).collect();
        assert_eq!(kinds, [("step", true), ("permission", false)]);
        assert_eq!(
            t.items[1].target.as_deref(),
            Some("node -e \"console.log(1)\"")
        );
    }

    #[test]
    fn reads_outside_the_project_are_failed_steps_not_offers() {
        let t = run(&[
            r#"{"event":"step_update","step_update":{"step_index":4,"state":"ERROR","step_type":"tool","tool_name":"view_file","tool_info":{"parameters":{"AbsolutePath":"/Users/me/.gemini/antigravity-cli/brain/x/session.json"},"error":{"message":"permission check failed"}}}}"#,
            r#"{"event":"step_update","step_update":{"step_index":5,"state":"ERROR","step_type":"tool","tool_name":"replace_file_content","tool_info":{"parameters":{"TargetFile":"/w/shop/greet.js"},"error":{"message":"permission check failed"}}}}"#,
        ]);
        let kinds: Vec<_> = t.items.iter().map(|i| i.kind).collect();
        assert_eq!(
            kinds,
            ["step", "step", "permission"],
            "only the edit inside the project is offered"
        );
        assert_eq!(t.items[2].target.as_deref(), Some("greet.js"));
    }

    #[test]
    fn a_reply_that_never_streamed_comes_from_the_result_and_secrets_are_hidden() {
        let token = format!("ghp_{}", "x".repeat(36));
        let line = format!(
            r#"{{"event":"result","result":{{"status":"SUCCESS","response":"Use {token} here"}}}}"#
        );
        let t = run(&[&line]);
        assert_eq!(t.items.len(), 1);
        assert!(!t.items[0].text.as_deref().unwrap().contains(&token));
    }

    #[test]
    fn turns_run_one_at_a_time_and_continue_the_same_conversation() {
        // A stand-in for agy that prints a turn and records its arguments.
        let dir = std::env::temp_dir().join(format!("verb-talk-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("agy");
        let args_file = dir.join("args");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" >> '{}'\necho '---' >> '{}'\n\
                 echo '{{\"event\":\"init\",\"conversation_id\":\"conv-9\"}}'\n\
                 echo '{{\"event\":\"result\",\"result\":{{\"conversation_id\":\"conv-9\",\"status\":\"SUCCESS\",\"response\":\"done\"}}}}'\n",
                args_file.display(),
                args_file.display()
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let talks = Talks::default();
        talks.create("t1".into(), Some("001".into()));
        let program = script.to_string_lossy().to_string();
        talks.send("t1", &dir, "first", &program).unwrap();
        assert!(
            talks.send("t1", &dir, "too soon", &program).is_err()
                || talks.view("t1").unwrap().items.len() >= 2
        );
        let wait = || {
            for _ in 0..100 {
                if !talks.view("t1").unwrap().working {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
            panic!("turn did not finish");
        };
        wait();
        talks.send("t1", &dir, "second", &program).unwrap();
        wait();
        let view = talks.view("t1").unwrap();
        assert_eq!(view.conversation_id.as_deref(), Some("conv-9"));
        let args = std::fs::read_to_string(&args_file).unwrap();
        let turns: Vec<&str> = args.split("---").filter(|s| !s.trim().is_empty()).collect();
        assert_eq!(turns.len(), 2);
        assert!(turns[0].contains("plan") && !turns[0].contains("--conversation"));
        assert!(turns[1].contains("--conversation\nconv-9"));
        assert!(!args.contains("dangerously"), "never skips permissions");
        let kinds: Vec<_> = view.items.iter().map(|i| i.kind).collect();
        assert_eq!(kinds, ["you", "agent", "you", "agent"]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
