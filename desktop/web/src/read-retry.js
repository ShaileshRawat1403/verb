// Background project reads (files, Git, specs, work items, ...) share the host's small pool of
// read workers (six, see MAX_BACKGROUND_READS in web.rs). A page load asks for about ten at once,
// and on a slow host each can take seconds, so the page queues its own reads below that pool and
// waits for a slot rather than racing for it. A read the host still turns away as busy (another
// tab, the state poll) backs off and tries again for a while before the error is shown.
export const READ_SLOTS = 4;
export const BUSY_DELAYS = [250, 500, 1000, 2000, 3000, 4000, 5000];
const BUSY = "desktop host is busy; retry shortly";

// The order a first load needs its reads in: the sidebar and work tree first, then the rest. On a
// node the execution boundary serves one read at a time (it says so in /api/workspace), so the
// page starts one read at a time, in this order, rather than queueing six behind the boundary in
// whatever order they were asked. Any other read comes after these.
export const FIRST_PAINT = ["/api/state", "/api/hub", "/api/work-items", "/api/git", "/api/specs", "/api/files"];

export function readRank(path) {
  const index = FIRST_PAINT.indexOf(String(path).split("?")[0]);
  return index < 0 ? FIRST_PAINT.length : index;
}

// Waiting reads start lowest rank first, then in the order they were asked. Reads asked in the
// same turn of the event loop are ordered together: starting waits for that turn to end.
export function createReadQueue(slots = READ_SLOTS, { defer = run => setTimeout(run, 0) } = {}) {
  let active = 0;
  let limit = slots;
  let asked = 0;
  let scheduled = false;
  const waiting = [];
  const pump = () => {
    scheduled = false;
    waiting.sort((a, b) => a.rank - b.rank || a.asked - b.asked);
    while (active < limit && waiting.length) {
      active++;
      waiting.shift().start();
    }
  };
  const schedule = () => {
    if (scheduled) return;
    scheduled = true;
    defer(pump);
  };
  function run(task, rank = FIRST_PAINT.length) {
    return new Promise((resolve, reject) => {
      waiting.push({
        rank,
        asked: asked++,
        start: () => {
          Promise.resolve()
            .then(task)
            .then(resolve, reject)
            .finally(() => {
              active--;
              schedule();
            });
        },
      });
      schedule();
    });
  }
  // How many reads the host can serve at once; never more than READ_SLOTS from one page.
  run.setSlots = (count) => {
    limit = Math.max(1, Math.min(READ_SLOTS, Math.floor(Number(count)) || READ_SLOTS));
    schedule();
  };
  run.slots = () => limit;
  return run;
}

const sharedQueue = createReadQueue();

// Called with /api/workspace's `boundaryReads` (absent on a host without a boundary).
export function setReadSlots(count) {
  sharedQueue.setSlots(count ?? READ_SLOTS);
}

// Only a rejected, read-only request may retry. Never accept a write callback.
export async function readWithBusyRetry(api, path, {
  wait = ms => new Promise(resolve => setTimeout(resolve, ms)),
  queue = sharedQueue,
  delays = BUSY_DELAYS,
} = {}) {
  for (let attempt = 0; ; attempt++) {
    try {
      return await queue(() => api("GET", path), readRank(path));
    } catch (error) {
      if (attempt >= delays.length || error.status !== 503 || error.message !== BUSY) throw error;
      await wait(delays[attempt] + Math.floor(Math.random() * 100));
    }
  }
}

// What the page last read, kept for this browser tab only (sessionStorage, like the access key),
// so a reload shows the last known project at once instead of empty panels while slow reads
// cross the host again. The fresh answer always replaces it within the same load.
const REMEMBERED = "verb.read.v1:";
const REMEMBER_LIMIT = 512 * 1024;

export function remembered(path, storage = globalThis.sessionStorage) {
  try {
    const raw = storage?.getItem(REMEMBERED + path);
    return raw == null ? undefined : JSON.parse(raw);
  } catch {
    return undefined;
  }
}

export function remember(path, value, storage = globalThis.sessionStorage) {
  try {
    const text = JSON.stringify(value);
    if (text.length <= REMEMBER_LIMIT) storage?.setItem(REMEMBERED + path, text);
    else storage?.removeItem(REMEMBERED + path);
  } catch {
    // A full or blocked store only means the next reload waits for the host.
  }
}

export function forgetRemembered(storage = globalThis.sessionStorage) {
  try {
    for (let i = (storage?.length ?? 0) - 1; i >= 0; i--) {
      const key = storage.key(i);
      if (key?.startsWith(REMEMBERED)) storage.removeItem(key);
    }
  } catch {
    // Nothing stored.
  }
}

// Shows the last answer for `path` first (if any), then reads the host and returns the fresh one.
export async function readRemembered(api, path, onRemembered, options = {}) {
  const prior = remembered(path, options.storage);
  if (prior !== undefined) onRemembered(prior);
  const value = await readWithBusyRetry(api, path, options);
  remember(path, value, options.storage);
  return value;
}
