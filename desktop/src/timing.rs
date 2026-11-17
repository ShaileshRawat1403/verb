//! Where one web request spends its time, reported in its `Server-Timing` header.
//!
//! Phases, in milliseconds:
//! * `queue`: from the request reaching the reply code until a worker starts its read.
//! * `boundary`: time inside execution-boundary entries (every Git or helper process a read runs),
//!   with `desc="n=<count>"`. On a host without a boundary the same calls run directly and are still
//!   counted, so a desktop run shows how many entries the read would make on the node.
//! * `work`: everything else.
//! * `total`: all of it.
//!
//! The recorder belongs to one request and is reached through a thread-local handle installed on
//! the worker thread that runs the read, so counting takes no global lock. The header carries numbers
//! only: no paths, names or ids.
use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

pub(crate) struct Recorder {
    received: Instant,
    started: OnceLock<Instant>,
    boundary_nanos: AtomicU64,
    entries: AtomicU32,
}

thread_local! {
    static CURRENT: RefCell<Option<Arc<Recorder>>> = const { RefCell::new(None) };
    /// Nested boundary calls (a helper that itself wraps a counted spawn) count once.
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}

impl Recorder {
    pub(crate) fn new(received: Instant) -> Arc<Self> {
        Arc::new(Self {
            received,
            started: OnceLock::new(),
            boundary_nanos: AtomicU64::new(0),
            entries: AtomicU32::new(0),
        })
    }

    /// Runs `work` on this thread with this recorder installed; the queue phase ends here.
    pub(crate) fn run<T>(self: &Arc<Self>, work: impl FnOnce() -> T) -> T {
        let _ = self.started.set(Instant::now());
        let previous = CURRENT.with(|current| current.replace(Some(Arc::clone(self))));
        struct Restore(Option<Arc<Recorder>>);
        impl Drop for Restore {
            fn drop(&mut self) {
                let previous = self.0.take();
                CURRENT.with(|current| *current.borrow_mut() = previous);
            }
        }
        let _restore = Restore(previous);
        work()
    }

    /// The `Server-Timing` value as of `now`.
    pub(crate) fn header(&self, now: Instant) -> String {
        let total = now.saturating_duration_since(self.received);
        let queue = self
            .started
            .get()
            .map_or(Duration::ZERO, |started| {
                started.saturating_duration_since(self.received)
            })
            .min(total);
        let boundary = Duration::from_nanos(self.boundary_nanos.load(Ordering::Relaxed));
        let work = total.saturating_sub(queue).saturating_sub(boundary);
        format!(
            "queue;dur={}, boundary;dur={};desc=\"n={}\", work;dur={}, total;dur={}",
            millis(queue),
            millis(boundary),
            self.entries.load(Ordering::Relaxed),
            millis(work),
            millis(total)
        )
    }
}

/// The header for a reply answered on the request loop, without a worker.
pub(crate) fn immediate(received: Instant) -> String {
    let recorder = Recorder::new(received);
    let _ = recorder.started.set(received);
    recorder.header(Instant::now())
}

fn millis(duration: Duration) -> u128 {
    duration.as_millis()
}

/// Runs one execution-boundary entry, counting it against the current request when there is one.
pub(crate) fn boundary<T>(entry: impl FnOnce() -> T) -> T {
    #[cfg(test)]
    trace::note();
    let recorder = CURRENT.with(|current| current.borrow().clone());
    let Some(recorder) = recorder else {
        return entry();
    };
    let outer = DEPTH.with(|depth| {
        let value = depth.get();
        depth.set(value + 1);
        value == 0
    });
    let started = Instant::now();
    let result = entry();
    DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    if outer {
        let spent = started.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        recorder.boundary_nanos.fetch_add(spent, Ordering::Relaxed);
        recorder.entries.fetch_add(1, Ordering::Relaxed);
    }
    result
}

/// Test builds: names the code each outermost boundary entry on this thread comes from, as the
/// chain of Verb functions that led to it (innermost first). Nested entries are not named again.
#[cfg(test)]
pub(crate) mod trace {
    use std::cell::{Cell, RefCell};

    thread_local! {
        static NAMES: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
        static INSIDE: Cell<bool> = const { Cell::new(false) };
    }

    /// Runs `work` and returns the boundary entries it made, one chain per entry.
    pub(crate) fn entries<T>(work: impl FnOnce() -> T) -> (T, Vec<String>) {
        let previous = NAMES.with(|names| names.replace(Some(Vec::new())));
        let value = work();
        let names = NAMES
            .with(|names| names.replace(previous))
            .unwrap_or_default();
        (value, names)
    }

    /// Marks the entries that a boundary call makes on this thread as nested.
    pub(crate) fn inside<T>(work: impl FnOnce() -> T) -> T {
        let was = INSIDE.with(|inside| inside.replace(true));
        let value = work();
        INSIDE.with(|inside| inside.set(was));
        value
    }

    pub(super) fn note() {
        if INSIDE.with(Cell::get) || NAMES.with(|names| names.borrow().is_none()) {
            return;
        }
        let chain = chain(&std::backtrace::Backtrace::force_capture().to_string());
        NAMES.with(|names| {
            if let Some(names) = names.borrow_mut().as_mut() {
                names.push(chain);
            }
        });
    }

    /// `project::common_dir < project::location < ...`: Verb frames only, without the plumbing
    /// that every entry shares, closures folded into their function.
    fn chain(backtrace: &str) -> String {
        let mut frames: Vec<String> = Vec::new();
        for line in backtrace.lines() {
            let Some((_, name)) = line.trim().split_once(": ") else {
                continue;
            };
            let Some(name) = name.strip_prefix("verb::") else {
                continue;
            };
            let name = match name.rsplit_once("::h") {
                Some((head, hash))
                    if hash.len() == 16 && hash.chars().all(|c| c.is_ascii_hexdigit()) =>
                {
                    head
                }
                _ => name,
            };
            let name = name.split("::{{closure}}").next().unwrap_or(name);
            let name = name.split("::<").next().unwrap_or(name).to_owned();
            if name.starts_with("isolation::simulated::") {
                break;
            }
            if [
                "timing::",
                "exec::bounded::",
                "exec::read_output",
                "project_read::request",
            ]
            .iter()
            .any(|plumbing| name.starts_with(plumbing))
                || name.contains("::tests::")
                || frames.last() == Some(&name)
            {
                continue;
            }
            frames.push(name);
        }
        frames.join(" < ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(header: &str) -> &str {
        header
            .split(", ")
            .find(|phase| phase.starts_with("boundary;"))
            .and_then(|phase| phase.split("desc=").nth(1))
            .unwrap()
    }

    #[test]
    fn counts_entries_only_inside_a_request() {
        boundary(|| ());
        let recorder = Recorder::new(Instant::now());
        recorder.run(|| {
            boundary(|| boundary(|| std::thread::sleep(Duration::from_millis(5))));
        });
        boundary(|| ());
        let header = recorder.header(Instant::now());
        assert_eq!(entries(&header), "\"n=1\"");
        let dur: u128 = header
            .split("boundary;dur=")
            .nth(1)
            .and_then(|rest| rest.split(';').next())
            .unwrap()
            .parse()
            .unwrap();
        assert!(dur >= 5, "{header}");
    }

    #[test]
    fn requests_on_other_threads_do_not_share_counts() {
        let a = Recorder::new(Instant::now());
        let b = Recorder::new(Instant::now());
        let a2 = Arc::clone(&a);
        std::thread::spawn(move || a2.run(|| (0..3).for_each(|_| boundary(|| ()))))
            .join()
            .unwrap();
        b.run(|| boundary(|| ()));
        assert_eq!(entries(&a.header(Instant::now())), "\"n=3\"");
        assert_eq!(entries(&b.header(Instant::now())), "\"n=1\"");
    }

    #[test]
    fn phases_add_up_and_name_nothing_but_numbers() {
        let received = Instant::now() - Duration::from_millis(30);
        let recorder = Recorder::new(received);
        let header = recorder.run(|| recorder.header(received + Duration::from_millis(50)));
        assert!(header.starts_with("queue;dur="), "{header}");
        assert!(header.ends_with("total;dur=50"), "{header}");
        assert!(header
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || " ;=,\"".contains(c)));
    }
}
