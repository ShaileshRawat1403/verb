// Background project reads (files, Git, specs, work items, ...) share the host's small pool of
// read workers (six, see MAX_BACKGROUND_READS in web.rs). A page load asks for about ten at once,
// and on a slow host each can take seconds, so the page queues its own reads below that pool and
// waits for a slot rather than racing for it. A read the host still turns away as busy (another
// tab, the state poll) backs off and tries again for a while before the error is shown.
export const READ_SLOTS = 4;
export const BUSY_DELAYS = [250, 500, 1000, 2000, 3000, 4000, 5000];
const BUSY = "desktop host is busy; retry shortly";

export function createReadQueue(slots = READ_SLOTS) {
  let active = 0;
  const waiting = [];
  const release = () => {
    active--;
    const next = waiting.shift();
    if (next) next();
  };
  return async function run(task) {
    if (active >= slots) await new Promise(resolve => waiting.push(resolve));
    active++;
    try {
      return await task();
    } finally {
      release();
    }
  };
}

const sharedQueue = createReadQueue();

// Only a rejected, read-only request may retry. Never accept a write callback.
export async function readWithBusyRetry(api, path, {
  wait = ms => new Promise(resolve => setTimeout(resolve, ms)),
  queue = sharedQueue,
  delays = BUSY_DELAYS,
} = {}) {
  for (let attempt = 0; ; attempt++) {
    try {
      return await queue(() => api("GET", path));
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
