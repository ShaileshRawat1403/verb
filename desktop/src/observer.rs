//! The observer: quiet, opt-in badges about running terminals.
//!
//! Boundaries, as the owner approved them (docs/AI_ASSIST_BRIEF.md): opt-in per project, local-only,
//! read-only, secret-redacting, badges and never pop-ups. Concretely:
//!
//! - Facts are evaluated on request from what Verb already holds in memory: each terminal's current
//!   screen, when it last produced output, and recent command exit codes from shell integration.
//!   Nothing here is written to disk except the on/off setting and which kinds are muted.
//! - A possible secret is reported by *kind* and terminal only. The matched text never leaves this
//!   module, not even to the browser.
//! - Every signal says why it fired. Nothing is ever done automatically; a badge can at most offer
//!   an action the person then takes.
//!
//! Server-side signals live here. Context pressure and wrong-branch are evaluated in the browser,
//! which already holds the meters, the Git state and the selected spec.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub(crate) const KINDS: [&str; 6] = ["waiting", "stuck", "failing", "secret", "context", "branch"];

const WAITING_AFTER_SECS: u64 = 60;
const PERMISSION_AFTER_SECS: u64 = 3;
const STUCK_AFTER_SECS: u64 = 10 * 60;
const FAILING_WINDOW_SECS: u64 = 15 * 60;
const FAILING_COUNT: usize = 3;

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub(crate) struct Settings {
    pub enabled: bool,
    #[serde(default)]
    pub muted: Vec<String>,
}

pub(crate) fn load(store: &Path) -> Settings {
    fs::read_to_string(store.join("observer.json"))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub(crate) fn save(store: &Path, settings: &Settings) -> Result<(), String> {
    fs::create_dir_all(store).map_err(|e| e.to_string())?;
    let raw = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(store.join("observer.json"), raw).map_err(|e| e.to_string())
}

/// What the observer may look at for one running terminal.
pub(crate) struct TerminalFacts<'a> {
    pub id: &'a str,
    pub agent: &'a str,
    pub is_agent: bool,
    pub idle_secs: u64,
    pub screen: &'a str,
    /// Failed commands: (short label, seconds ago).
    pub failures: &'a [(String, u64)],
}

#[derive(Serialize, Debug, PartialEq)]
pub(crate) struct Signal {
    /// Stable for the same situation, so a dismissed badge stays dismissed.
    pub key: String,
    pub kind: &'static str,
    pub terminal: Option<String>,
    pub title: String,
    pub why: String,
    /// A palette-style action the browser can offer: "show-terminal".
    pub action: Option<&'static str>,
}

/// The last non-empty lines of a screen, where prompts and recent output live.
fn tail(screen: &str, lines: usize) -> String {
    let all: Vec<&str> = screen.lines().filter(|l| !l.trim().is_empty()).collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

/// Prompts that mean an agent or program is waiting for the person.
pub(crate) fn waiting_prompt(screen: &str) -> Option<&'static str> {
    // Fourteen lines: Antigravity's permission prompt lists four wrapped options and a footer under
    // its question, which pushed the question out of a shorter window (seen in a user test).
    let t = tail(screen, 14).to_lowercase();
    // Agents reword these; "trust this folder" is Claude Code's and Gemini's wording since late 2026
    // (Claude asks "Is this a project you created or one you trust?", seen in a user test).
    const PATTERNS: [(&str, &str); 15] = [
        ("requesting permission for", "a permission question"),
        ("trust this folder", "a permission question"),
        ("one you trust?", "a permission question"),
        ("run this command?", "a permission question"),
        ("do you trust the contents", "a permission question"),
        ("do you trust the files", "a permission question"),
        ("do you want to proceed", "a permission question"),
        ("do you want to make this edit", "a permission question"),
        ("do you want to create", "a permission question"),
        ("allow this", "a permission question"),
        ("approve", "an approval prompt"),
        ("(y/n)", "a yes/no question"),
        ("[y/n]", "a yes/no question"),
        ("press enter to continue", "a prompt to continue"),
        ("waiting for your input", "a request for input"),
    ];
    PATTERNS
        .iter()
        .find(|(p, _)| t.contains(p))
        .map(|(_, what)| *what)
}

/// High-confidence secret shapes. Returns the kind, never the text.
pub(crate) fn secret_kind(screen: &str) -> Option<&'static str> {
    let has_run = |s: &str, prefix: &str, min: usize, ok: fn(char) -> bool| {
        s.match_indices(prefix)
            .any(|(i, _)| s[i + prefix.len()..].chars().take_while(|c| ok(*c)).count() >= min)
    };
    let alnum = |c: char| c.is_ascii_alphanumeric();
    let token = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    if screen.contains("-----BEGIN") && screen.contains("PRIVATE KEY-----") {
        return Some("a private key");
    }
    if has_run(screen, "AKIA", 16, |c| {
        c.is_ascii_uppercase() || c.is_ascii_digit()
    }) {
        return Some("an AWS access key");
    }
    if ["ghp_", "gho_", "ghs_", "github_pat_"]
        .iter()
        .any(|p| has_run(screen, p, 20, token))
    {
        return Some("a GitHub token");
    }
    if ["sk-ant-", "sk-proj-"]
        .iter()
        .any(|p| has_run(screen, p, 20, token))
        || has_run(screen, "sk-", 32, token)
    {
        return Some("an API key");
    }
    if ["xoxb-", "xoxp-"]
        .iter()
        .any(|p| has_run(screen, p, 20, token))
    {
        return Some("a Slack token");
    }
    // NAME_TOKEN=<long value> or similar on one line.
    for line in screen.lines() {
        let upper = line.to_uppercase();
        if ["TOKEN=", "SECRET=", "API_KEY=", "PASSWORD="]
            .iter()
            .any(|k| upper.contains(k))
        {
            if let Some((_, value)) = line.split_once('=') {
                let v = value.trim().trim_matches(['"', '\'']);
                if v.len() >= 20 && v.chars().all(|c| alnum(c) || "-_./+=".contains(c)) {
                    return Some("a value assigned to a secret-looking name");
                }
            }
        }
    }
    None
}

/// A failed command's label as shown in a badge: the first two words, and nothing that looks like
/// an assignment (which might carry a value).
pub(crate) fn short_label(label: &str) -> Option<String> {
    let words: Vec<&str> = label.split_whitespace().take(2).collect();
    if words.is_empty() || words.iter().any(|w| w.contains('=')) {
        return None;
    }
    Some(words.join(" "))
}

/// Agent terminals that are explicitly asking the person for permission right now. Reported with or
/// without the observer: it is the agent asking, and the tab says "Needs you".
pub(crate) fn asking(terminals: &[TerminalFacts]) -> Vec<String> {
    terminals
        .iter()
        .filter(|t| {
            t.is_agent
                && t.idle_secs >= PERMISSION_AFTER_SECS
                && waiting_prompt(t.screen) == Some("a permission question")
        })
        .map(|t| t.id.to_owned())
        .collect()
}

pub(crate) fn evaluate(terminals: &[TerminalFacts], muted: &[String]) -> Vec<Signal> {
    let on = |kind: &str| !muted.iter().any(|m| m == kind);
    let mut out = Vec::new();
    for t in terminals {
        let short = &t.id[..t.id.len().min(8)];
        let prompt = waiting_prompt(t.screen);
        // An agent asking permission is waiting from the moment it asks; anything vaguer gets a
        // minute's grace so a prompt that is only passing by never raises a badge.
        let asking = prompt == Some("a permission question");
        let after = if asking {
            PERMISSION_AFTER_SECS
        } else {
            WAITING_AFTER_SECS
        };
        if on("waiting") && t.idle_secs >= after {
            if let Some(what) = prompt {
                out.push(Signal {
                    key: format!("waiting:{}", t.id),
                    kind: "waiting",
                    terminal: Some(t.id.to_owned()),
                    title: format!("{} is waiting for you", t.agent),
                    why: if asking {
                        format!("It is asking {what} and will not continue until you answer ({short}).")
                    } else {
                        format!("Its screen has shown {what} for over a minute with no new output ({short}).")
                    },
                    action: Some("show-terminal"),
                });
            }
        }
        if on("stuck") && t.is_agent && prompt.is_none() && t.idle_secs >= STUCK_AFTER_SECS {
            out.push(Signal {
                key: format!("stuck:{}", t.id),
                kind: "stuck",
                terminal: Some(t.id.to_owned()),
                title: format!("{} has been quiet for {} min", t.agent, t.idle_secs / 60),
                why: format!("No output from this agent session for {} minutes, and no question on its screen ({short}).", t.idle_secs / 60),
                action: Some("show-terminal"),
            });
        }
        if on("failing") {
            let mut counts: Vec<(String, usize)> = Vec::new();
            for (label, ago) in t.failures {
                if *ago > FAILING_WINDOW_SECS {
                    continue;
                }
                let Some(label) = short_label(label) else {
                    continue;
                };
                match counts.iter_mut().find(|(l, _)| *l == label) {
                    Some((_, n)) => *n += 1,
                    None => counts.push((label, 1)),
                }
            }
            for (label, n) in counts.into_iter().filter(|(_, n)| *n >= FAILING_COUNT) {
                out.push(Signal {
                    key: format!("failing:{}:{label}", t.id),
                    kind: "failing",
                    terminal: Some(t.id.to_owned()),
                    title: format!("`{label}` failed {n} times"),
                    why: format!("Shell integration saw `{label}` exit with an error {n} times in the last 15 minutes ({short}). Ask Verb or hand off for a fresh look."),
                    action: Some("show-terminal"),
                });
            }
        }
        if on("secret") {
            if let Some(kind) = secret_kind(t.screen) {
                out.push(Signal {
                    key: format!("secret:{}:{kind}", t.id),
                    kind: "secret",
                    terminal: Some(t.id.to_owned()),
                    title: format!("Possible secret on screen ({short})"),
                    why: format!("The screen shows what looks like {kind}. If it is real, consider rotating it: anything on screen can end up in logs, recordings or an agent's context. Verb did not store or send it."),
                    action: Some("show-terminal"),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts<'a>(
        screen: &'a str,
        idle: u64,
        is_agent: bool,
        failures: &'a [(String, u64)],
    ) -> TerminalFacts<'a> {
        TerminalFacts {
            id: "abcd1234-full",
            agent: "Claude Code",
            is_agent,
            idle_secs: idle,
            screen,
            failures,
        }
    }

    #[test]
    fn a_permission_question_is_waiting_at_once_anything_vaguer_after_a_minute() {
        let claude = "Edit src/app.js\n\nDo you want to proceed?\n❯ 1. Yes\n  2. No";
        let agy = "Requesting permission for:\n   node -e \"import('./greet.js')\"\n\nRun this command?\n> 1. Yes, run command\n  2. Yes, and always allow in this conversation for commands that start with 'node -e\n\"import('./greet.js')\"'\n  3. Yes, and always allow for commands that start with 'node -e \"import('./greet.js')\"'\n(Persist to settings.json)\n  4. No, cancel\n\n esc to cancel                         Gemini 3.8 Flash · high";
        for screen in [claude, agy] {
            assert!(
                evaluate(&[facts(screen, 1, true, &[])], &[]).is_empty(),
                "still drawing"
            );
            let s = evaluate(&[facts(screen, 5, true, &[])], &[]);
            assert_eq!(s.len(), 1, "{screen}");
            assert_eq!(s[0].kind, "waiting");
            assert!(s[0].why.contains("will not continue until you answer"));
        }
        let vague = "Overwrite config? (y/n)";
        assert!(
            evaluate(&[facts(vague, 30, true, &[])], &[]).is_empty(),
            "too soon"
        );
        assert!(evaluate(&[facts(vague, 90, true, &[])], &[])[0]
            .why
            .contains("over a minute"));
        assert!(
            evaluate(&[facts(claude, 90, true, &[])], &["waiting".into()]).is_empty(),
            "muted"
        );
    }

    #[test]
    fn claude_codes_folder_trust_question_is_a_permission_question() {
        let screen = "Accessing workspace:\n/tmp/real\nQuick safety check: Is this a project you created or one you trust? (Like your own code, a well-known open source project, or work from your team).\nClaude Code'll be able to read, edit, and execute files here.\nSecurity guide\n❯ No, exit\n  Yes, I trust this folder\nEnter to confirm · Esc to cancel";
        assert_eq!(waiting_prompt(screen), Some("a permission question"));
        assert_eq!(asking(&[facts(screen, 5, true, &[])]).len(), 1);
    }

    #[test]
    fn asking_is_only_agents_with_a_permission_question() {
        let prompt = "Requesting permission for:\n  rm -rf build\n\nRun this command?\n> 1. Yes";
        assert_eq!(asking(&[facts(prompt, 5, true, &[])]).len(), 1);
        assert!(
            asking(&[facts(prompt, 1, true, &[])]).is_empty(),
            "still drawing"
        );
        assert!(
            asking(&[facts(prompt, 5, false, &[])]).is_empty(),
            "a shell is not an agent"
        );
        assert!(
            asking(&[facts("Overwrite? (y/n)", 90, true, &[])]).is_empty(),
            "only permission questions"
        );
    }

    #[test]
    fn stuck_is_for_quiet_agents_without_a_question() {
        assert_eq!(
            evaluate(&[facts("working…", 700, true, &[])], &[])[0].kind,
            "stuck"
        );
        assert!(
            evaluate(&[facts("$ ", 7000, false, &[])], &[]).is_empty(),
            "an idle shell is normal"
        );
        let waiting = evaluate(&[facts("Continue? (y/n)", 700, true, &[])], &[]);
        assert_eq!(
            waiting.iter().map(|s| s.kind).collect::<Vec<_>>(),
            ["waiting"],
            "waiting, not stuck"
        );
    }

    #[test]
    fn failing_counts_the_same_command_in_the_window() {
        let f = vec![
            ("npm test -- --watch".to_owned(), 60),
            ("npm test".to_owned(), 300),
            ("npm test".to_owned(), 600),
            ("npm run lint".to_owned(), 30),
            ("npm test".to_owned(), 5000),
        ];
        let s = evaluate(&[facts("$ ", 5, false, &f)], &[]);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].title, "`npm test` failed 3 times");
        assert_eq!(
            short_label("API_KEY=abc deploy"),
            None,
            "assignments are never shown"
        );
    }

    #[test]
    fn secrets_are_reported_by_kind_never_by_value() {
        let cases = [
            (
                "export GITHUB_TOKEN=ghp_abcdefghijklmnopqrstuvwxyz0123456789",
                "a GitHub token",
            ),
            ("AKIAIOSFODNN7EXAMPLE", "an AWS access key"),
            (
                "-----BEGIN OPENSSH PRIVATE KEY-----\nb3Blbn...",
                "a private key",
            ),
            ("key: sk-ant-api03-ABCDEFGHIJKLMNOPQRSTUVWXYZ", "an API key"),
            (
                "VERB_TOKEN=ef800e307029b8d1751dfa9443cf23e927413d7d",
                "a GitHub token",
            ),
        ];
        for (screen, kind) in &cases[..4] {
            assert_eq!(secret_kind(screen), Some(*kind), "{screen}");
        }
        assert_eq!(
            secret_kind(cases[4].0),
            Some("a value assigned to a secret-looking name")
        );
        let s = evaluate(&[facts(cases[0].0, 0, false, &[])], &[]);
        let json = serde_json::to_string(&s).unwrap();
        assert!(
            !json.contains("ghp_abcdef"),
            "the value never leaves the observer: {json}"
        );
        assert_eq!(secret_kind("ls -la\ncargo test"), None);
        assert_eq!(secret_kind("TOKEN=short"), None);
    }

    #[test]
    fn settings_round_trip() {
        let dir = std::env::temp_dir().join(format!("verb-observer-{}", std::process::id()));
        assert_eq!(load(&dir), Settings::default(), "off unless chosen");
        let s = Settings {
            enabled: true,
            muted: vec!["stuck".into()],
        };
        save(&dir, &s).unwrap();
        assert_eq!(load(&dir), s);
        let _ = fs::remove_dir_all(dir);
    }
}
