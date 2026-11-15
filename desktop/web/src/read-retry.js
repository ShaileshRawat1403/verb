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
