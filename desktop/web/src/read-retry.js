// Only a rejected, read-only file-list request may retry. Never accept a write callback.
export async function readWithBusyRetry(api, path, { wait = ms => new Promise(resolve => setTimeout(resolve, ms)) } = {}) {
  for (let attempt = 0; ; attempt++) {
    try {
      return await api("GET", path);
    } catch (error) {
      if (attempt >= 2 || error.status !== 503 || error.message !== "desktop host is busy; retry shortly") throw error;
      await wait(200 * (attempt + 1) + Math.floor(Math.random() * 100));
    }
  }
}
