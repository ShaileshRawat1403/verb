//! Runtime version facts: what a project declares it needs, and what actually runs here.
//!
//! Backlog C4. The mockups promise a band for "runtime mismatch detected", and `docs/TUI_VISION.md`
//! is explicit that it may only appear from "runtime version against a declared requirement" --
//! never from a guess about what a project probably wants. So this module has two halves that never
//! blur:
//!
//! * **Declared** requirements are read from files the project itself ships (`.nvmrc`,
//!   `package.json` engines, `.python-version`, `pyproject.toml`, `rust-toolchain.toml`,
//!   `Cargo.toml` `rust-version`, `go.mod`, `.ruby-version`, `.tool-versions`). A runtime nothing
//!   declares is not reported at all.
//! * **Found** versions come from running the runtime's own `--version` in the project directory,
//!   so version managers that honour the directory (nvm shims, pyenv, rustup) answer with what would
//!   really run here.
//!
//! The comparison has three outcomes and a fourth for absence, following `Unknown ≠ No`: a
//! requirement Verb cannot parse (`lts/*`, `stable`, `system`) is `Unknown`, never `Mismatch`.
//!
//! Two probes are deliberately refused rather than run: a `rust-toolchain.toml` with a `path` key and
//! a `.tool-versions` entry of the form `path:` or `ref:`. Both make the version manager execute a
//! binary chosen by the repository, and opening a project in Verb must never do that.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How long one `--version` may take before Verb stops waiting and reports it as failed.
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);
/// Declaration files are small; anything larger is not one Verb should be parsing.
const MAX_DECLARATION_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Runtime {
    Node,
    Python,
    Rust,
    Go,
    Ruby,
}

impl Runtime {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Python => "python",
            Self::Rust => "rust",
            Self::Go => "go",
            Self::Ruby => "ruby",
        }
    }

    /// Candidates tried in order; the first that runs is the answer.
    fn probes(self) -> &'static [(&'static str, &'static [&'static str])] {
        match self {
            Self::Node => &[("node", &["--version"])],
            Self::Python => &[("python3", &["--version"]), ("python", &["--version"])],
            Self::Rust => &[("rustc", &["--version"])],
            Self::Go => &[("go", &["version"])],
            Self::Ruby => &[("ruby", &["--version"])],
        }
    }

    const ALL: [Runtime; 5] = [
        Runtime::Node,
        Runtime::Python,
        Runtime::Rust,
        Runtime::Go,
        Runtime::Ruby,
    ];
}

/// One requirement, as the project wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Declaration {
    pub runtime: Runtime,
    /// The file it came from, relative to the project. Never an absolute path.
    pub source: &'static str,
    /// The requirement text, verbatim, so the user can see exactly what Verb compared against.
    pub wants: String,
    spec: Option<Spec>,
    /// Why the found version must not be probed for this declaration, when it must not.
    refuse_probe: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Probe {
    Found(String),
    NotFound,
    Failed,
    Refused(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    Satisfied,
    Mismatch,
    /// The requirement is not one Verb can compare (`lts/*`, `stable`), or the probe gave nothing.
    Unknown,
    /// The runtime is declared but did not run here at all.
    Missing,
}

impl Verdict {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Satisfied => "satisfied",
            Self::Mismatch => "mismatch",
            Self::Unknown => "unknown",
            Self::Missing => "missing",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fact {
    pub declaration: Declaration,
    pub probe: Probe,
    pub verdict: Verdict,
}

impl Fact {
    pub(crate) fn found(&self) -> Option<&str> {
        match &self.probe {
            Probe::Found(version) => Some(version),
            _ => None,
        }
    }

    pub(crate) fn to_json(&self) -> String {
        let probe = match &self.probe {
            Probe::Found(_) => "found",
            Probe::NotFound => "notFound",
            Probe::Failed => "failed",
            Probe::Refused(_) => "refused",
        };
        format!(
            "{{\"runtime\":\"{}\",\"source\":\"{}\",\"wants\":\"{}\",\"found\":{},\"probe\":\"{}\",\"verdict\":\"{}\"}}",
            self.declaration.runtime.label(),
            crate::json_escape(self.declaration.source),
            crate::json_escape(&self.declaration.wants),
            self.found()
                .map(|version| format!("\"{}\"", crate::json_escape(version)))
                .unwrap_or_else(|| "null".to_owned()),
            probe,
            self.verdict.as_str()
        )
    }

    pub(crate) fn to_text(&self) -> String {
        let found = match &self.probe {
            Probe::Found(version) => version.clone(),
            Probe::NotFound => "not installed".to_owned(),
            Probe::Failed => "did not report a version".to_owned(),
            Probe::Refused(reason) => format!("not run: {reason}"),
        };
        format!(
            "{} {} · {} wants {} · {}",
            self.declaration.runtime.label(),
            found,
            self.declaration.source,
            self.declaration.wants,
            self.verdict.as_str()
        )
    }
}

/// Everything the project declares, each compared with what runs here. Probes run once per runtime.
pub(crate) fn observe(project: &Path) -> Vec<Fact> {
    let declarations = declarations(project);
    let mut facts = Vec::new();
    for runtime in Runtime::ALL {
        let mine: Vec<&Declaration> = declarations
            .iter()
            .filter(|declaration| declaration.runtime == runtime)
            .collect();
        if mine.is_empty() {
            continue;
        }
        // One refusal refuses the runtime: the version manager would read the dangerous file no
        // matter which declaration Verb was asking on behalf of.
        let probe = match mine.iter().find_map(|declaration| declaration.refuse_probe) {
            Some(reason) => Probe::Refused(reason),
            None => probe(runtime, project),
        };
        for declaration in mine {
            facts.push(Fact {
                verdict: verdict(declaration.spec.as_ref(), &probe),
                declaration: declaration.clone(),
                probe: probe.clone(),
            });
        }
    }
    facts
}

fn verdict(spec: Option<&Spec>, probe: &Probe) -> Verdict {
    match probe {
        Probe::NotFound => Verdict::Missing,
        Probe::Failed | Probe::Refused(_) => Verdict::Unknown,
        Probe::Found(found) => match (spec, Version::parse(found)) {
            (Some(spec), Some(version)) => {
                if spec.matches(&version) {
                    Verdict::Satisfied
                } else {
                    Verdict::Mismatch
                }
            }
            _ => Verdict::Unknown,
        },
    }
}

// ----- declarations ---------------------------------------------------------------------------

pub(crate) fn declarations(project: &Path) -> Vec<Declaration> {
    let mut found = Vec::new();
    let read = |name: &str| read_small(&project.join(name));

    for name in [".nvmrc", ".node-version"] {
        if let Some(text) = read(name).and_then(|text| first_value(&text)) {
            found.push(pin(Runtime::Node, static_name(name), &text));
        }
    }
    if let Some(text) = read("package.json") {
        if let Some(wants) = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|value| value.pointer("/engines/node")?.as_str().map(str::to_owned))
        {
            found.push(Declaration {
                runtime: Runtime::Node,
                source: "package.json",
                spec: Spec::npm(&wants),
                wants,
                refuse_probe: None,
            });
        }
    }

    if let Some(text) = read(".python-version").and_then(|text| first_value(&text)) {
        found.push(pin(Runtime::Python, ".python-version", &text));
    }
    if let Some(wants) = read("pyproject.toml").and_then(|text| requires_python(&text)) {
        found.push(Declaration {
            runtime: Runtime::Python,
            source: "pyproject.toml",
            spec: Spec::pep440(&wants),
            wants,
            refuse_probe: None,
        });
    }

    for name in ["rust-toolchain.toml", "rust-toolchain"] {
        if let Some(text) = read(name) {
            if let Some(declaration) = rust_toolchain(static_name(name), &text) {
                found.push(declaration);
            }
            break;
        }
    }
    if let Some(wants) = read("Cargo.toml").and_then(|text| toml_string(&text, "rust-version")) {
        found.push(minimum(Runtime::Rust, "Cargo.toml", &wants));
    }

    if let Some(wants) = read("go.mod").and_then(|text| go_directive(&text)) {
        found.push(minimum(Runtime::Go, "go.mod", &wants));
    }

    if let Some(text) = read(".ruby-version").and_then(|text| first_value(&text)) {
        found.push(pin(Runtime::Ruby, ".ruby-version", &text));
    }

    if let Some(text) = read(".tool-versions") {
        found.extend(tool_versions(&text));
    }
    found
}

fn static_name(name: &str) -> &'static str {
    match name {
        ".nvmrc" => ".nvmrc",
        ".node-version" => ".node-version",
        "rust-toolchain.toml" => "rust-toolchain.toml",
        "rust-toolchain" => "rust-toolchain",
        _ => "project file",
    }
}

fn read_small(path: &Path) -> Option<String> {
    let metadata = fs::symlink_metadata(path).ok()?;
    // A symlink could point anywhere on the machine; a declaration is a file in the project.
    if !metadata.file_type().is_file() || metadata.len() > MAX_DECLARATION_BYTES {
        return None;
    }
    fs::read_to_string(path).ok()
}

/// The first non-empty, non-comment line, trimmed.
fn first_value(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
}

/// A file that names one version (`.nvmrc`, `.python-version`): `20` means any 20.x.
fn pin(runtime: Runtime, source: &'static str, text: &str) -> Declaration {
    let trimmed = text.trim();
    let bare = trimmed.strip_prefix('v').unwrap_or(trimmed);
    Declaration {
        runtime,
        source,
        spec: parts(bare).map(|parts| Spec::all(vec![Comparator::prefix(parts)])),
        wants: trimmed.to_owned(),
        refuse_probe: None,
    }
}

/// A floor (`rust-version`, `go 1.22`): anything at or above it.
fn minimum(runtime: Runtime, source: &'static str, text: &str) -> Declaration {
    Declaration {
        runtime,
        source,
        spec: parts(text.trim()).map(|parts| Spec::all(vec![Comparator::new(Op::Ge, parts)])),
        wants: format!(">={}", text.trim()),
        refuse_probe: None,
    }
}

fn rust_toolchain(source: &'static str, text: &str) -> Option<Declaration> {
    let is_toml = source.ends_with(".toml") || text.contains('[');
    let (channel, has_path) = if is_toml {
        (
            toml_string(text, "channel"),
            text.lines()
                .any(|line| line.trim_start().starts_with("path") && line.contains('=')),
        )
    } else {
        (first_value(text), false)
    };
    if has_path {
        return Some(Declaration {
            runtime: Runtime::Rust,
            source,
            wants: "a toolchain at a local path".to_owned(),
            spec: None,
            refuse_probe: Some("the toolchain file points at a binary inside the project"),
        });
    }
    let channel = channel?;
    // `1.80.0`, or `1.80.0-x86_64-unknown-linux-gnu`; `stable`/`nightly-2024-01-01` stay unknown.
    let version = channel.split('-').next().unwrap_or(&channel);
    Some(Declaration {
        runtime: Runtime::Rust,
        source,
        spec: parts(version).map(|parts| Spec::all(vec![Comparator::prefix(parts)])),
        wants: channel,
        refuse_probe: None,
    })
}

fn go_directive(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let line = line.split("//").next().unwrap_or("").trim();
        let rest = line.strip_prefix("go ")?;
        Some(rest.trim().to_owned()).filter(|value| !value.is_empty())
    })
}

/// `requires-python = ">=3.10"` from `[project]`, the standard location.
fn requires_python(text: &str) -> Option<String> {
    let mut in_project = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_project = trimmed == "[project]";
            continue;
        }
        if in_project {
            if let Some(value) = key_value(trimmed, "requires-python") {
                return Some(value);
            }
        }
    }
    None
}

/// The first `key = "value"` anywhere in a small TOML file. Enough for the keys read here, which
/// are unique in practice; a real TOML parser is not worth a dependency for five strings.
fn toml_string(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| key_value(line.trim(), key))
}

fn key_value(line: &str, key: &str) -> Option<String> {
    let rest = line.strip_prefix(key)?.trim_start();
    let rest = rest.strip_prefix('=')?.trim();
    let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let body = &rest[1..];
    let end = body.find(quote)?;
    Some(body[..end].to_owned())
}

fn tool_versions(text: &str) -> Vec<Declaration> {
    let mut found = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        let mut fields = line.split_whitespace();
        let (Some(tool), Some(version)) = (fields.next(), fields.next()) else {
            continue;
        };
        let runtime = match tool {
            "nodejs" | "node" => Runtime::Node,
            "python" => Runtime::Python,
            "rust" => Runtime::Rust,
            "golang" | "go" => Runtime::Go,
            "ruby" => Runtime::Ruby,
            _ => continue,
        };
        let mut declaration = pin(runtime, ".tool-versions", version);
        if version.starts_with("path:") || version.starts_with("ref:") {
            declaration.spec = None;
            declaration.refuse_probe =
                Some("the version manager would run or build a binary the project names");
        }
        found.push(declaration);
    }
    found
}

// ----- probing --------------------------------------------------------------------------------

fn probe(runtime: Runtime, project: &Path) -> Probe {
    let mut outcome = Probe::NotFound;
    for (program, args) in runtime.probes() {
        match run_bounded(program, args, project) {
            Ok(Some(text)) => match extract_version(&text) {
                Some(version) => return Probe::Found(version),
                None => outcome = Probe::Failed,
            },
            Ok(None) => outcome = Probe::Failed,
            Err(()) => {}
        }
    }
    outcome
}

/// `Err` when the program does not exist; `Ok(None)` when it ran but failed or timed out.
fn run_bounded(program: &str, args: &[&str], directory: &Path) -> Result<Option<String>, ()> {
    let mut child = Command::new(program)
        .args(args)
        .current_dir(directory)
        // A pinned but uninstalled toolchain must be reported as such, not downloaded.
        .env("RUSTUP_AUTO_INSTALL", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| ())?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut text = String::new();
                if let Some(mut stdout) = child.stdout.take() {
                    let _ = stdout.read_to_string(&mut text);
                }
                // Python 2 and some managers print the version on stderr.
                if let Some(mut stderr) = child.stderr.take() {
                    let _ = stderr.read_to_string(&mut text);
                }
                return Ok(status.success().then_some(text));
            }
            Ok(None) if started.elapsed() < PROBE_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(10));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(None);
            }
        }
    }
}

/// The first dotted number in a `--version` line: `v20.11.1`, `Python 3.12.1`,
/// `rustc 1.80.0 (…)`, `go version go1.22.3 linux/amd64`, `ruby 3.3.0p0`.
fn extract_version(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_digit() {
            let start = index;
            while index < bytes.len() && (bytes[index].is_ascii_digit() || bytes[index] == b'.') {
                index += 1;
            }
            let candidate = text[start..index].trim_end_matches('.');
            if candidate.contains('.') {
                return Some(candidate.to_owned());
            }
        }
        index += 1;
    }
    None
}

// ----- versions and requirements --------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Version([u64; 4]);

impl Version {
    fn parse(text: &str) -> Option<Self> {
        parts(text.trim().trim_start_matches('v')).map(|parts| Self::padded(&parts))
    }

    fn padded(parts: &[u64]) -> Self {
        let mut value = [0; 4];
        for (slot, part) in value.iter_mut().zip(parts) {
            *slot = *part;
        }
        Self(value)
    }
}

/// `3.11.4` → `[3, 11, 4]`. Stops at the first non-numeric component (`3.13.0rc1` → `[3, 13, 0]`,
/// `20.x` → `[20]`); nothing numeric at all is `None`.
fn parts(text: &str) -> Option<Vec<u64>> {
    let mut parts = Vec::new();
    for component in text.split('.') {
        let digits: String = component.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() {
            break;
        }
        parts.push(digits.parse().ok()?);
        if digits.len() != component.len() {
            break;
        }
        if parts.len() == 4 {
            break;
        }
    }
    (!parts.is_empty()).then_some(parts)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Ge,
    Gt,
    Le,
    Lt,
    Eq,
    Ne,
    /// The first N components match exactly: `20` accepts `20.11.1`.
    Prefix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Comparator {
    op: Op,
    parts: Vec<u64>,
}

impl Comparator {
    fn new(op: Op, parts: Vec<u64>) -> Self {
        Self { op, parts }
    }

    fn prefix(parts: Vec<u64>) -> Self {
        Self::new(Op::Prefix, parts)
    }

    fn matches(&self, version: &Version) -> bool {
        let bound = Version::padded(&self.parts);
        match self.op {
            Op::Ge => version >= &bound,
            Op::Gt => version > &bound,
            Op::Le => version <= &bound,
            Op::Lt => version < &bound,
            Op::Eq => version == &bound,
            Op::Ne => version != &bound,
            Op::Prefix => version.0.iter().zip(&self.parts).all(|(a, b)| a == b),
        }
    }
}

/// Alternatives of conjunctions: `>=18 <21 || 22` is `[[>=18, <21], [22.x]]`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Spec(Vec<Vec<Comparator>>);

impl Spec {
    fn all(comparators: Vec<Comparator>) -> Self {
        Self(vec![comparators])
    }

    fn matches(&self, version: &Version) -> bool {
        self.0
            .iter()
            .any(|set| set.iter().all(|comparator| comparator.matches(version)))
    }

    /// The node-semver subset projects actually write in `engines`. Anything else is `None`.
    fn npm(text: &str) -> Option<Self> {
        let mut alternatives = Vec::new();
        for alternative in text.split("||") {
            let alternative = alternative.trim();
            if alternative.contains(" - ") {
                return None;
            }
            let mut set = Vec::new();
            // `>= 18` is written with a space as often as without.
            let joined = alternative
                .replace(">= ", ">=")
                .replace("<= ", "<=")
                .replace("> ", ">")
                .replace("< ", "<")
                .replace("= ", "=");
            let mut tokens = 0;
            for token in joined.split_whitespace() {
                tokens += 1;
                set.extend(npm_comparator(token)?);
            }
            // An empty set after real tokens is `*`: it accepts everything. No tokens at all is
            // an empty requirement, which says nothing.
            if tokens == 0 {
                return None;
            }
            alternatives.push(set);
        }
        (!alternatives.is_empty()).then_some(Self(alternatives))
    }

    /// PEP 440 specifiers as used in `requires-python`: comma-separated, all must hold.
    fn pep440(text: &str) -> Option<Self> {
        let mut set = Vec::new();
        for clause in text.split(',') {
            let clause = clause.trim();
            if clause.is_empty() {
                continue;
            }
            let (op, rest) = ["~=", "==", "!=", ">=", "<=", ">", "<"]
                .iter()
                .find_map(|op| clause.strip_prefix(op).map(|rest| (*op, rest.trim())))?;
            let wildcard = rest.ends_with(".*");
            let parts = parts(rest.trim_end_matches(".*"))?;
            match op {
                "==" if wildcard => set.push(Comparator::prefix(parts)),
                "==" => set.push(Comparator::new(Op::Eq, parts)),
                "!=" if wildcard => return None,
                "!=" => set.push(Comparator::new(Op::Ne, parts)),
                ">=" => set.push(Comparator::new(Op::Ge, parts)),
                "<=" => set.push(Comparator::new(Op::Le, parts)),
                ">" => set.push(Comparator::new(Op::Gt, parts)),
                "<" => set.push(Comparator::new(Op::Lt, parts)),
                "~=" => {
                    // `~=3.10` is `>=3.10, ==3.*`; `~=3.10.2` is `>=3.10.2, ==3.10.*`.
                    if parts.len() < 2 {
                        return None;
                    }
                    set.push(Comparator::new(Op::Ge, parts.clone()));
                    set.push(Comparator::prefix(parts[..parts.len() - 1].to_vec()));
                }
                _ => return None,
            }
        }
        (!set.is_empty()).then_some(Self::all(set))
    }
}

fn npm_comparator(token: &str) -> Option<Vec<Comparator>> {
    if matches!(token, "*" | "x" | "X" | "latest") {
        return if token == "latest" {
            None
        } else {
            Some(Vec::new())
        };
    }
    let (op, rest) = [">=", "<=", ">", "<", "=", "^", "~"]
        .iter()
        .find_map(|op| token.strip_prefix(op).map(|rest| (*op, rest)))
        .unwrap_or(("", token));
    let rest = rest.strip_prefix('v').unwrap_or(rest);
    // `18.x`, `18.*`: the wildcard ends the precision rather than being a number.
    let precise: Vec<&str> = rest
        .split('.')
        .take_while(|part| !matches!(*part, "x" | "X" | "*"))
        .collect();
    let parts = parts(&precise.join("."))?;
    let n = parts.len();
    let next = |at: usize| {
        let mut bumped = parts[..=at].to_vec();
        bumped[at] += 1;
        bumped
    };
    Some(match op {
        "" | "=" if n < 3 => vec![Comparator::prefix(parts)],
        "" | "=" => vec![Comparator::new(Op::Eq, parts)],
        ">=" => vec![Comparator::new(Op::Ge, parts)],
        "<" => vec![Comparator::new(Op::Lt, parts)],
        // `>18` means past every 18.x; `<=18` includes every 18.x.
        ">" if n < 3 => vec![Comparator::new(Op::Ge, next(n - 1))],
        ">" => vec![Comparator::new(Op::Gt, parts)],
        "<=" if n < 3 => vec![Comparator::new(Op::Lt, next(n - 1))],
        "<=" => vec![Comparator::new(Op::Le, parts)],
        "^" => {
            // The left-most non-zero component may not change.
            let pivot = parts.iter().position(|part| *part != 0).unwrap_or(n - 1);
            vec![
                Comparator::new(Op::Ge, parts.clone()),
                Comparator::new(Op::Lt, next(pivot.min(n - 1))),
            ]
        }
        "~" => {
            let at = if n >= 2 { 1 } else { 0 };
            vec![
                Comparator::new(Op::Ge, parts.clone()),
                Comparator::new(Op::Lt, next(at)),
            ]
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn satisfied(spec: Option<Spec>, version: &str) -> bool {
        spec.expect("spec parses")
            .matches(&Version::parse(version).unwrap())
    }

    #[test]
    fn versions_are_read_from_each_runtimes_own_banner() {
        assert_eq!(extract_version("v20.11.1\n").as_deref(), Some("20.11.1"));
        assert_eq!(extract_version("Python 3.12.1").as_deref(), Some("3.12.1"));
        assert_eq!(
            extract_version("rustc 1.80.0 (051478957 2024-07-21)").as_deref(),
            Some("1.80.0")
        );
        assert_eq!(
            extract_version("go version go1.22.3 linux/amd64").as_deref(),
            Some("1.22.3")
        );
        assert_eq!(
            extract_version("ruby 3.3.0p0 (2023-12-25 revision 5124f9ac75)").as_deref(),
            Some("3.3.0")
        );
        assert_eq!(extract_version("no digits here"), None);
    }

    #[test]
    fn npm_ranges_projects_actually_write() {
        assert!(satisfied(Spec::npm(">=18"), "20.11.1"));
        assert!(!satisfied(Spec::npm(">=18"), "16.20.0"));
        assert!(satisfied(Spec::npm(">= 18 < 21"), "20.0.0"));
        assert!(!satisfied(Spec::npm(">=18 <21"), "21.0.0"));
        assert!(satisfied(Spec::npm("^18.2.0"), "18.19.0"));
        assert!(!satisfied(Spec::npm("^18.2.0"), "19.0.0"));
        assert!(!satisfied(Spec::npm("^0.2.3"), "0.3.0"));
        assert!(satisfied(Spec::npm("~3.11"), "3.11.9"));
        assert!(!satisfied(Spec::npm("~3.11"), "3.12.0"));
        assert!(satisfied(Spec::npm("18.x || 20.x"), "20.1.0"));
        assert!(!satisfied(Spec::npm("18.x || 20.x"), "22.0.0"));
        assert!(satisfied(Spec::npm(">18"), "19.0.0"));
        assert!(!satisfied(Spec::npm(">18"), "18.99.0"));
        assert!(satisfied(Spec::npm("<=18"), "18.99.0"));
        assert!(satisfied(Spec::npm("*"), "1.0.0"));
    }

    #[test]
    fn requirements_verb_cannot_compare_are_unknown_not_mismatched() {
        assert_eq!(Spec::npm("lts/*"), None);
        assert_eq!(Spec::npm("latest"), None);
        assert_eq!(Spec::npm("1.0 - 2.0"), None);
        assert_eq!(pin(Runtime::Node, ".nvmrc", "lts/iron").spec, None);
        assert_eq!(pin(Runtime::Python, ".python-version", "system").spec, None);
        assert_eq!(
            verdict(None, &Probe::Found("20.0.0".to_owned())),
            Verdict::Unknown
        );
    }

    #[test]
    fn pep440_requires_python() {
        assert!(satisfied(Spec::pep440(">=3.10,<3.13"), "3.12.1"));
        assert!(!satisfied(Spec::pep440(">=3.10,<3.13"), "3.13.0"));
        assert!(satisfied(Spec::pep440("~=3.10"), "3.12.0"));
        assert!(!satisfied(Spec::pep440("~=3.10"), "4.0.0"));
        assert!(!satisfied(Spec::pep440("~=3.10.2"), "3.11.0"));
        assert!(satisfied(Spec::pep440("==3.11.*"), "3.11.4"));
        assert!(!satisfied(Spec::pep440("!=3.11.0"), "3.11.0"));
    }

    #[test]
    fn a_pinned_major_accepts_any_release_of_it() {
        let declaration = pin(Runtime::Node, ".nvmrc", "v20\n");
        assert_eq!(declaration.wants, "v20");
        assert!(satisfied(declaration.spec, "20.11.1"));
        assert!(!satisfied(
            pin(Runtime::Node, ".nvmrc", "20").spec,
            "18.0.0"
        ));
    }

    #[test]
    fn verdicts_keep_absence_and_uncertainty_apart() {
        let spec = Spec::npm(">=18");
        assert_eq!(verdict(spec.as_ref(), &Probe::NotFound), Verdict::Missing);
        assert_eq!(verdict(spec.as_ref(), &Probe::Failed), Verdict::Unknown);
        assert_eq!(
            verdict(spec.as_ref(), &Probe::Refused("x")),
            Verdict::Unknown
        );
        assert_eq!(
            verdict(spec.as_ref(), &Probe::Found("16.0.0".to_owned())),
            Verdict::Mismatch
        );
    }

    fn project(files: &[(&str, &str)]) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("verb-runtime-{}", crate::new_id()));
        fs::create_dir_all(&root).unwrap();
        for (name, body) in files {
            fs::write(root.join(name), body).unwrap();
        }
        root
    }

    #[test]
    fn declarations_are_read_from_the_files_projects_ship() {
        let root = project(&[
            (".nvmrc", "20\n"),
            ("package.json", r#"{"name":"x","engines":{"node":">=18"}}"#),
            (
                "pyproject.toml",
                "[tool.x]\nrequires-python = \"nope\"\n[project]\nname = \"x\"\nrequires-python = \">=3.10\"\n",
            ),
            ("rust-toolchain.toml", "[toolchain]\nchannel = \"1.80.0\"\n"),
            ("Cargo.toml", "[package]\nname = \"x\"\nrust-version = \"1.75\"\n"),
            ("go.mod", "module x\n\ngo 1.22 // minimum\n"),
            (".tool-versions", "ruby 3.3.0\nterraform 1.0\n"),
        ]);
        let found: Vec<(Runtime, &str, String)> = declarations(&root)
            .into_iter()
            .map(|d| (d.runtime, d.source, d.wants))
            .collect();
        assert_eq!(
            found,
            vec![
                (Runtime::Node, ".nvmrc", "20".to_owned()),
                (Runtime::Node, "package.json", ">=18".to_owned()),
                (Runtime::Python, "pyproject.toml", ">=3.10".to_owned()),
                (Runtime::Rust, "rust-toolchain.toml", "1.80.0".to_owned()),
                (Runtime::Rust, "Cargo.toml", ">=1.75".to_owned()),
                (Runtime::Go, "go.mod", ">=1.22".to_owned()),
                (Runtime::Ruby, ".tool-versions", "3.3.0".to_owned()),
            ]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_project_that_names_its_own_binary_is_never_probed() {
        let root = project(&[
            ("rust-toolchain.toml", "[toolchain]\npath = \"./evil\"\n"),
            (".tool-versions", "nodejs path:./bin\n"),
        ]);
        let facts = observe(&root);
        assert_eq!(facts.len(), 2);
        for fact in &facts {
            assert!(matches!(fact.probe, Probe::Refused(_)), "{fact:?}");
            assert_eq!(fact.verdict, Verdict::Unknown);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_undeclared_runtime_is_not_reported() {
        let root = project(&[("README.md", "hello")]);
        assert!(observe(&root).is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_declaration_is_ignored() {
        let root = project(&[]);
        let outside = project(&[("secret", "20\n")]);
        std::os::unix::fs::symlink(outside.join("secret"), root.join(".nvmrc")).unwrap();
        assert!(declarations(&root).is_empty());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }

    #[test]
    fn json_carries_the_source_name_never_a_path() {
        let fact = Fact {
            declaration: pin(Runtime::Node, ".nvmrc", "20"),
            probe: Probe::Found("18.19.0".to_owned()),
            verdict: Verdict::Mismatch,
        };
        assert_eq!(
            fact.to_json(),
            r#"{"runtime":"node","source":".nvmrc","wants":"20","found":"18.19.0","probe":"found","verdict":"mismatch"}"#
        );
        assert_eq!(fact.to_text(), "node 18.19.0 · .nvmrc wants 20 · mismatch");
    }
}
