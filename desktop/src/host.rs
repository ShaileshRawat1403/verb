//! The Host page: a read-only report on the machine Verb is running on.
//!
//! Works on any host, and shows more when the host offers it: phones running Verb under Termux and
//! proot add temperatures, runit service states and a restart/crash timeline. Everything here is a
//! best-effort read. A fact that cannot be read is reported as unavailable, with the reason, never
//! guessed. Under proot, system uptime and load are fabricated, so they are not shown at all; Verb's
//! own uptime is measured from its process start instead.
//!
//! Nothing here identifies the host on a network: no addresses, user names or paths beyond what the
//! workbench already shows.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Instant;

static STARTED: OnceLock<(Instant, String)> = OnceLock::new();
static BINARY_SHA: OnceLock<Option<String>> = OnceLock::new();

/// Call once at startup so uptime is measured from launch, not from the first page view.
pub(crate) fn mark_started() {
    STARTED.get_or_init(|| (Instant::now(), crate::iso8601(crate::now_millis())));
}

#[derive(Serialize, Debug, Default)]
pub(crate) struct Report {
    pub verb: VerbFacts,
    pub machine: Machine,
    pub memory: Option<Usage>,
    pub storage: Option<Usage>,
    pub temperatures: Vec<Temperature>,
    pub battery: Battery,
    pub services: Option<Vec<Service>>,
    pub events: Vec<Event>,
    pub notes: Vec<String>,
}

#[derive(Serialize, Debug, Default)]
pub(crate) struct VerbFacts {
    pub version: &'static str,
    pub binary_sha256: Option<String>,
    pub commit: Option<String>,
    pub installed: Option<String>,
    pub started: Option<String>,
    pub uptime_secs: Option<u64>,
    pub rss_kb: Option<u64>,
}

#[derive(Serialize, Debug, Default)]
pub(crate) struct Machine {
    pub model: Option<String>,
    pub os: String,
    pub arch: String,
    pub container: Option<&'static str>,
}

#[derive(Serialize, Debug, PartialEq)]
pub(crate) struct Usage {
    pub total_kb: u64,
    pub available_kb: u64,
}

#[derive(Serialize, Debug, PartialEq)]
pub(crate) struct Temperature {
    pub name: String,
    pub celsius: f32,
}

#[derive(Serialize, Debug, Default)]
pub(crate) struct Battery {
    pub level: Option<u8>,
    pub charging: Option<bool>,
    pub reason: Option<String>,
}

#[derive(Serialize, Debug, PartialEq)]
pub(crate) struct Service {
    pub name: String,
    pub state: String,
    pub uptime_secs: Option<u64>,
}

#[derive(Serialize, Debug, PartialEq)]
pub(crate) struct Event {
    pub at: String,
    pub kind: &'static str,
    pub detail: String,
}

// ------------------------------------------------------------------------------------- parsers

pub(crate) fn parse_meminfo(raw: &str) -> Option<Usage> {
    let field = |name: &str| {
        raw.lines()
            .find(|l| l.starts_with(name))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<u64>().ok())
    };
    Some(Usage {
        total_kb: field("MemTotal:")?,
        available_kb: field("MemAvailable:")?,
    })
}

/// `df -kP` output: the data line's total and available 1K-blocks.
pub(crate) fn parse_df(raw: &str) -> Option<Usage> {
    let fields: Vec<&str> = raw.lines().nth(1)?.split_whitespace().collect();
    Some(Usage {
        total_kb: fields.get(1)?.parse().ok()?,
        available_kb: fields.get(3)?.parse().ok()?,
    })
}

/// One `sv status` line: `run: verb: (pid 11797) 43270s; run: log: …`.
pub(crate) fn parse_sv(line: &str) -> Option<Service> {
    let first = line.split(';').next()?;
    let (state, rest) = first.split_once(": ")?;
    let (name, rest) = rest.split_once(':')?;
    let uptime_secs = rest
        .split_whitespace()
        .find_map(|w| w.strip_suffix('s').and_then(|n| n.parse().ok()));
    let mut state = state.trim().to_owned();
    if line.contains("runsv not running") {
        // A service directory exists but nothing supervises it: the process may still be running,
        // started some other way. "fail" would claim more than runit knows.
        state = "not supervised".to_owned();
    } else if line.contains("got TERM") {
        state.push_str(" (stopping)");
    }
    Some(Service {
        name: name.trim().to_owned(),
        state,
        uptime_secs,
    })
}

fn signal_name(n: u32) -> &'static str {
    match n {
        1 => "SIGHUP",
        2 => "SIGINT",
        5 => "SIGTRAP",
        6 => "SIGABRT",
        7 => "SIGBUS",
        9 => "SIGKILL",
        11 => "SIGSEGV",
        15 => "SIGTERM",
        _ => "signal",
    }
}

/// Restarts and crashes from a runit/svlogd service log, newest first.
pub(crate) fn parse_service_log(raw: &str, limit: usize) -> Vec<Event> {
    let mut events: Vec<Event> = raw
        .lines()
        .filter_map(|line| {
            let (stamp, text) = line.split_once(' ')?;
            if stamp.len() < 19 || !stamp.as_bytes()[0].is_ascii_digit() {
                return None;
            }
            let at = format!("{} {}", &stamp[..10], &stamp[11..19]);
            if text.contains("Verb web UI:") {
                Some(Event {
                    at,
                    kind: "start",
                    detail: "Verb started".to_owned(),
                })
            } else {
                text.split("terminated with signal ")
                    .nth(1)
                    .and_then(|s| s.trim().parse::<u32>().ok())
                    .map(|sig| Event {
                        at,
                        kind: if matches!(sig, 9 | 15 | 1 | 2) {
                            "stop"
                        } else {
                            "crash"
                        },
                        detail: format!("stopped by {} ({sig})", signal_name(sig)),
                    })
            }
        })
        .collect();
    events.reverse();
    events.truncate(limit);
    events
}

pub(crate) fn pick_temperatures(zones: &[(String, i64)]) -> Vec<Temperature> {
    // Prefer zones a person recognises: the battery, the phone's casing, the CPU.
    const WANTED: [(&str, &str); 6] = [
        ("battery", "Battery"),
        ("shell_back", "Back of phone"),
        ("shell_front", "Front of phone"),
        ("skin-msm-therm-usr", "Inside the case"),
        ("cpuss-0-usr", "CPU"),
        ("x86_pkg_temp", "CPU"),
    ];
    let mut out = Vec::new();
    for (zone, label) in WANTED {
        if let Some((_, milli)) = zones.iter().find(|(name, _)| name == zone) {
            // Thermal zones report milli-degrees; a few report whole degrees.
            let c = if milli.abs() > 1000 {
                *milli as f32 / 1000.0
            } else {
                *milli as f32
            };
            if (-30.0..130.0).contains(&c) && !out.iter().any(|t: &Temperature| t.name == label) {
                out.push(Temperature {
                    name: label.to_owned(),
                    celsius: (c * 10.0).round() / 10.0,
                });
            }
        }
    }
    out
}

// -------------------------------------------------------------------------------------- probes

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn binary_sha() -> Option<String> {
    BINARY_SHA
        .get_or_init(|| {
            let exe = std::env::current_exe().ok()?;
            let bytes = fs::read(exe).ok()?;
            Some(format!("{:x}", Sha256::digest(bytes)))
        })
        .clone()
}

/// Matches this binary against `$HOME/verb-deployments/*/manifest.json`, which the deploy scripts
/// write, to say which commit is running.
fn deployment(sha: &str) -> Option<(String, Option<String>)> {
    deployment_in(
        &PathBuf::from(std::env::var_os("HOME")?).join("verb-deployments"),
        sha,
    )
}

fn deployment_in(dir: &Path, sha: &str) -> Option<(String, Option<String>)> {
    for entry in fs::read_dir(dir).ok()?.flatten() {
        // Older deployment folders have no manifest, or one in another shape: skip them.
        let Ok(raw) = fs::read_to_string(entry.path().join("manifest.json")) else {
            continue;
        };
        let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        if manifest.get("sha256").and_then(|v| v.as_str()) == Some(sha) {
            let Some(commit) = manifest.get("git").and_then(|v| v.as_str()) else {
                continue;
            };
            let commit = commit.to_owned();
            let installed = manifest
                .get("installed")
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            return Some((commit, installed));
        }
    }
    None
}

fn own_rss_kb() -> Option<u64> {
    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        return status
            .lines()
            .find(|l| l.starts_with("VmRSS:"))?
            .split_whitespace()
            .nth(1)?
            .parse()
            .ok();
    }
    run("ps", &["-o", "rss=", "-p", &std::process::id().to_string()])?
        .trim()
        .parse()
        .ok()
}

fn memory() -> Option<Usage> {
    if let Ok(raw) = fs::read_to_string("/proc/meminfo") {
        return parse_meminfo(&raw);
    }
    // macOS: total from sysctl; available approximated as free + inactive pages.
    let total: u64 = run("sysctl", &["-n", "hw.memsize"])?.trim().parse().ok()?;
    let vm = run("vm_stat", &[])?;
    let page: u64 = vm
        .split("page size of ")
        .nth(1)?
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    let pages = |name: &str| -> u64 {
        vm.lines()
            .find(|l| l.starts_with(name))
            .and_then(|l| l.split(':').nth(1))
            .and_then(|v| v.trim().trim_end_matches('.').parse().ok())
            .unwrap_or(0)
    };
    Some(Usage {
        total_kb: total / 1024,
        available_kb: (pages("Pages free") + pages("Pages inactive")) * page / 1024,
    })
}

fn thermal_zones() -> Vec<(String, i64)> {
    let Ok(entries) = fs::read_dir("/sys/class/thermal") else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("thermal_zone"))
        .filter_map(|e| {
            let name = fs::read_to_string(e.path().join("type")).ok()?;
            let temp = fs::read_to_string(e.path().join("temp")).ok()?;
            Some((name.trim().to_owned(), temp.trim().parse().ok()?))
        })
        .collect()
}

fn battery() -> Battery {
    let base = Path::new("/sys/class/power_supply/battery");
    match fs::read_to_string(base.join("capacity")) {
        Ok(level) => Battery {
            level: level.trim().parse().ok(),
            charging: fs::read_to_string(base.join("status"))
                .ok()
                .map(|s| s.trim() == "Charging" || s.trim() == "Full"),
            reason: None,
        },
        Err(e) if base.exists() => Battery {
            reason: Some(format!(
                "Android does not let apps read the charge level ({}). Installing Termux:API would allow it.",
                e.kind()
            )),
            ..Battery::default()
        },
        Err(_) => Battery {
            reason: Some("This machine reports no battery to Verb.".to_owned()),
            ..Battery::default()
        },
    }
}

/// The runit service directory, when Verb runs under Termux's runit (directly or inside proot).
fn runit_dir() -> Option<PathBuf> {
    std::env::var_os("VERB_RUNIT_SVDIR")
        .map(PathBuf::from)
        .or_else(|| Some(PathBuf::from("/data/data/com.termux/files/usr/var/service")))
        .filter(|p| p.is_dir())
}

fn services(svdir: &Path) -> Option<Vec<Service>> {
    let sv = svdir.parent()?.parent()?.join("bin/sv");
    let mut names: Vec<String> = fs::read_dir(svdir)
        .ok()?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let mut out = Vec::new();
    for name in names {
        let line = Command::new(&sv)
            .env("SVDIR", svdir)
            .args(["status", &name])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        out.push(parse_sv(line.trim()).unwrap_or(Service {
            name,
            state: "unknown".to_owned(),
            uptime_secs: None,
        }));
    }
    Some(out)
}

pub(crate) fn report() -> Report {
    let mut notes = Vec::new();
    let sha = binary_sha();
    let deployed = sha.as_deref().and_then(deployment);
    let started = STARTED.get();
    let kernel = run("uname", &["-r"]).unwrap_or_default();
    let container = (kernel.contains("PRoot")).then_some("proot");
    if container.is_some() {
        notes.push(
            "Running inside proot, which fabricates system uptime and load; they are not shown."
                .to_owned(),
        );
    }
    let model = run("getprop", &["ro.product.model"])
        .or_else(|| run("/system/bin/getprop", &["ro.product.model"]))
        .map(|m| m.trim().to_owned())
        .filter(|m| !m.is_empty())
        .or_else(|| run("sysctl", &["-n", "hw.model"]).map(|m| m.trim().to_owned()));
    let svdir = runit_dir();
    let log = std::env::var_os("VERB_SERVICE_LOG")
        .map(PathBuf::from)
        .or_else(|| {
            svdir
                .as_ref()
                .and_then(|d| d.parent())
                .map(|var| var.join("log/sv/verb/current"))
        });
    let events = log
        .and_then(|p| fs::read_to_string(p).ok())
        .map(|raw| parse_service_log(&raw, 12))
        .unwrap_or_default();
    Report {
        verb: VerbFacts {
            version: env!("CARGO_PKG_VERSION"),
            commit: deployed.as_ref().map(|d| d.0.clone()),
            installed: deployed.and_then(|d| d.1),
            binary_sha256: sha,
            started: started.map(|s| s.1.clone()),
            uptime_secs: started.map(|s| s.0.elapsed().as_secs()),
            rss_kb: own_rss_kb(),
        },
        machine: Machine {
            model,
            os: std::env::consts::OS.to_owned(),
            arch: std::env::consts::ARCH.to_owned(),
            container,
        },
        memory: memory(),
        storage: std::env::current_dir()
            .ok()
            .and_then(|dir| run("df", &["-kP", &dir.to_string_lossy()]))
            .and_then(|raw| parse_df(&raw)),
        temperatures: pick_temperatures(&thermal_zones()),
        battery: battery(),
        services: svdir.as_deref().and_then(services),
        events,
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meminfo_and_df_parse() {
        let mem = "MemTotal:        7439804 kB\nMemFree: 1 kB\nMemAvailable:    2462520 kB\n";
        assert_eq!(
            parse_meminfo(mem),
            Some(Usage {
                total_kb: 7439804,
                available_kb: 2462520
            })
        );
        let df = "Filesystem 1024-blocks Used Available Capacity Mounted on\n/dev/block/dm-79 108000000 19000000 88000000 19% /\n";
        assert_eq!(
            parse_df(df),
            Some(Usage {
                total_kb: 108000000,
                available_kb: 88000000
            })
        );
    }

    #[test]
    fn sv_status_lines_parse_including_a_stalled_stop() {
        assert_eq!(
            parse_sv("run: verb: (pid 11797) 43270s; run: log: (pid 14592) 664714s"),
            Some(Service {
                name: "verb".into(),
                state: "run".into(),
                uptime_secs: Some(43270)
            })
        );
        assert_eq!(
            parse_sv("run: verb: (pid 22290) 1295s, got TERM; run: log: (pid 14592) 1s")
                .unwrap()
                .state,
            "run (stopping)"
        );
        assert_eq!(
            parse_sv("down: sshd: 12s, normally up").unwrap().state,
            "down"
        );
    }

    #[test]
    fn service_log_yields_starts_and_crashes_newest_first() {
        let log = "2026-10-07_09:40:35.85606 proot info: vpid 1: terminated with signal 7\n\
                   2026-10-07_09:40:37.21562 Verb web UI: http://127.0.0.1:3005 (configured token in use)\n\
                   2026-10-07_09:40:37.21572 Local only. Close this process to stop its hosted agent sessions.\n\
                   2026-10-07_12:14:06.10000 proot info: vpid 1: terminated with signal 15\n";
        let events = parse_service_log(log, 10);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].kind, "stop");
        assert_eq!(
            events[1],
            Event {
                at: "2026-10-07 09:40:37".into(),
                kind: "start",
                detail: "Verb started".into()
            }
        );
        assert_eq!(events[2].kind, "crash");
        assert_eq!(events[2].detail, "stopped by SIGBUS (7)");
    }

    #[test]
    fn temperatures_use_friendly_names_and_degrees() {
        let zones = vec![
            ("modem-lte-sub6-pa1".to_owned(), 40000),
            ("battery".to_owned(), 39600),
            ("shell_back".to_owned(), 38648),
            ("cpuss-0-usr".to_owned(), 41800),
        ];
        let t = pick_temperatures(&zones);
        assert_eq!(
            t[0],
            Temperature {
                name: "Battery".into(),
                celsius: 39.6
            }
        );
        assert_eq!(
            t.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            ["Battery", "Back of phone", "CPU"]
        );
    }

    #[test]
    fn deployments_without_a_manifest_do_not_hide_later_ones() {
        let home = std::env::temp_dir().join(format!("verb-host-home-{}", std::process::id()));
        let dirs = home.join("verb-deployments");
        fs::create_dir_all(dirs.join("old-no-manifest")).unwrap();
        fs::create_dir_all(dirs.join("other-shape")).unwrap();
        fs::write(dirs.join("other-shape/manifest.json"), r#"{"commit":"x"}"#).unwrap();
        fs::create_dir_all(dirs.join("abc1234")).unwrap();
        fs::write(
            dirs.join("abc1234/manifest.json"),
            r#"{"git":"abc1234","sha256":"feed","installed":"2026-10-08T00:00:00+00:00"}"#,
        )
        .unwrap();
        assert_eq!(
            deployment_in(&dirs, "feed"),
            Some((
                "abc1234".to_owned(),
                Some("2026-10-08T00:00:00+00:00".to_owned())
            ))
        );
        assert_eq!(deployment_in(&dirs, "nope"), None);
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn the_report_runs_on_this_machine() {
        mark_started();
        let r = report();
        assert_eq!(r.verb.version, env!("CARGO_PKG_VERSION"));
        assert!(r
            .verb
            .binary_sha256
            .as_deref()
            .is_some_and(|s| s.len() == 64));
        assert!(r.memory.is_some_and(|m| m.total_kb > 0));
        assert!(r.storage.is_some_and(|s| s.total_kb > 0));
    }
}
