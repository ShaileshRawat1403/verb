// A person opens a terminal, types, reads the answer, and leaves. The command only prints.
// The terminal draws with WebGL, so its text is not in the page. The journey reads what the host
// sends back over the terminal's socket instead: that is the echo the person sees, less one frame.
import { BUDGET } from "./budget.js";
import { expect, open, record, test } from "./support.js";

const decoder = new TextDecoder();
// A binary frame is [id length][session id][output bytes]; text frames are control messages.
const text = payload => (typeof payload === "string" ? "" : decoder.decode(payload.subarray(1 + payload[0])));
// Terminal control sequences (colour, cursor, shell marks) are not text the person reads.
const plain = raw => raw.replace(/\x1b\][^\x07\x1b]*(\x07|\x1b\\)/g, "").replace(/\x1b\[[0-9;?]*[ -\/]*[@-~]/g, "").replace(/\x1b[()][0-9A-Za-z]/g, "");

test("a terminal opens, echoes typing quickly and runs a command", async ({ page }) => {
  let output = "";
  // Every terminal stream after the first is a reconnect; a healthy run has none.
  let streams = 0;
  page.on("websocket", socket => {
    if (new URL(socket.url()).pathname === "/api/terminals/ws") streams++;
    socket.on("framereceived", ({ payload }) => { output += text(payload); });
  });
  await open(page);
  const tiles = page.locator("#terminal-grid .terminal-tile");
  await expect(page.locator("#bench-work-tree")).not.toContainText("Loading work items", { timeout: 15_000 });
  // Terminals from earlier visits can come back or close while the page settles, so the new one
  // is the tile whose id was not there before, not a count.
  const ids = async () => tiles.evaluateAll(list => list.map(tile => tile.id));
  const known = new Set(await ids());
  // Alt+T opens a new terminal whether or not others are already open.
  await page.keyboard.press("Alt+KeyT");
  let id;
  await expect.poll(async () => (id = (await ids()).find(each => !known.has(each))), { timeout: 30_000 }).toBeTruthy();
  const tile = page.locator(`#${id}`);
  const input = tile.getByRole("textbox", { name: "Terminal input" });
  // The shell is ready once its prompt has arrived.
  await expect.poll(() => /[$#%>] ?$/m.test(output), { timeout: 30_000 }).toBe(true);
  const marker = `verb-journey-${Date.now().toString(36)}`;
  try {
    // Keystroke to echo: one character at a time, timed until the host sends it back.
    await input.focus();
    const echoes = [];
    for (const ch of "echo ") {
      const seen = output.length;
      const started = Date.now();
      await page.keyboard.type(ch);
      await expect.poll(() => output.length > seen, { timeout: 2_000, intervals: [2] }).toBe(true);
      echoes.push(Date.now() - started);
    }
    const median = echoes.sort((a, b) => a - b)[Math.floor(echoes.length / 2)];
    record("terminal", "keystroke to echo, median", median, "ms", median <= 50 ? "pass" : median <= BUDGET.keystrokeEcho ? "slow" : "fail");
    await page.keyboard.type(marker);
    await page.keyboard.press("Enter");
    // The command's own output is the marker alone on a line (the typed line also has "echo ").
    await expect.poll(() => new RegExp(`(^|[\\r\\n])${marker}\\r?\\n`).test(plain(output)), { timeout: 15_000 }).toBe(true);
    expect(median, "keystroke to echo").toBeLessThanOrEqual(BUDGET.keystrokeEcho);
  } finally {
    await input.focus();
    await page.keyboard.type("exit");
    await page.keyboard.press("Enter");
  }
  const reconnects = Math.max(0, streams - 1);
  record("terminal", "socket reconnects", reconnects, "count", reconnects ? "fail" : "pass");
  expect(reconnects, "terminal socket reconnects").toBe(0);
});
