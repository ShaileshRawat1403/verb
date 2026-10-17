import { test, expect } from "@playwright/test";
import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

// A stalled execution-boundary stop: the pane closes, and the browser must say that process
// cleanup was not confirmed instead of implying it was. Needs a `verb` built with the test-only
// `synthetic-boundary` feature (VERB_SYNTHETIC_BIN); Linux only, like the boundary itself.
const TOKEN = "synthetic-stalled-cleanup-browser-token";
// The external path requires a registered-project-shaped directory name.
const PROJECT = "0f1e2d3c4b5a69788796a5b4c3d2e1f0";
// Stand-in boundary client: admits terminals at once, fails background jobs, and stops leases
// unless `stop-stall` holds a delay. Same arguments and receipt fields as the real client.
const CLIENT = `#!/bin/bash
root="$(cd "$(dirname "$0")" && pwd)"
if [ "$1" = --stop-client ]; then
  if [ -f "$root/stop-stall" ]; then sleep "$(cat "$root/stop-stall")"; exit 126; fi
  rm -f "$root/receipts/$2.json"
  exit 0
fi
while [ $# -gt 0 ]; do
  case "$1" in
    --project) project="$2"; shift 2 ;;
    --session) session="$2"; shift 2 ;;
    --mode) mode="$2"; shift 2 ;;
    --program) program="$2"; shift 2 ;;
    --) shift; break ;;
    *) exit 64 ;;
  esac
done
[ "$mode" = terminal ] || exit 126
printf '{"generation":"0123456789abcdef0123456789abcdef","operation":"stop","project":"%s","session":"%s","version":1}\\n' \\
  "$project" "$session" > "$root/receipts/.pending-$$"
mv "$root/receipts/.pending-$$" "$root/receipts/$$.json"
exec "$(command -v "$program")" "$@"
`;
const freePort = () => new Promise((resolvePort) => {
  const socket = createServer().listen(0, "127.0.0.1", () => {
    const { port } = socket.address();
    socket.close(() => resolvePort(port));
  });
});

test("a stalled boundary stop closes the pane but reports cleanup as unconfirmed", async ({ page }) => {
  test.skip(process.platform !== "linux" || !process.env.VERB_SYNTHETIC_BIN, "needs the synthetic boundary build on Linux");
  const fixture = mkdtempSync(join(tmpdir(), "verb-stalled-cleanup-"));
  const repo = join(fixture, PROJECT);
  const boundary = join(fixture, "boundary");
  for (const path of [repo, join(fixture, "home"), join(boundary, "receipts")]) mkdirSync(path, { recursive: true, mode: 0o700 });
  writeFileSync(join(boundary, "client"), CLIENT, { mode: 0o755 });
  const environment = { PATH: process.env.PATH, HOME: join(fixture, "home"), LANG: "C", SHELL: "/bin/bash" };
  // Not a Git repository: project identity then needs no Git through the stand-in boundary.
  const port = await freePort();
  const origin = `http://127.0.0.1:${port}`;
  const server = spawn(resolve(process.env.VERB_SYNTHETIC_BIN), ["web", "--port", String(port)], {
    cwd: repo,
    env: { ...environment, VERB_STATE_DIR: join(fixture, "state"), VERB_TOKEN: TOKEN, VERB_SYNTHETIC_BOUNDARY: boundary },
    stdio: ["ignore", "pipe", "pipe"],
  });
  const openTerminal = async () => {
    const before = await page.locator(".terminal-tile").count();
    await page.locator("#quick-terminal-button, #quick-terminal-empty").first().click();
    await expect(page.locator(".terminal-tile")).toHaveCount(before + 1, { timeout: 10_000 });
    return (await page.locator(".terminal-tile").last().getAttribute("id")).slice("terminal-".length);
  };
  const endTerminal = async (id) => {
    await page.locator(`[data-close-terminal="${id}"]`).click();
    await page.locator("#end-session-confirm").click();
    await expect(page.locator(`#terminal-${id}`)).toHaveCount(0, { timeout: 15_000 });
  };
  try {
    await new Promise((ready, reject) => {
      const timer = setTimeout(() => reject(new Error("synthetic host did not start")), 15_000);
      server.stdout.on("data", (data) => {
        if (String(data).includes("Verb web UI")) { clearTimeout(timer); ready(); }
      });
      server.once("exit", () => { clearTimeout(timer); reject(new Error("synthetic host exited")); });
    });
    await page.goto(`${origin}/#${TOKEN}`);

    // Confirmed cleanup: the pane closes and nothing claims otherwise.
    await endTerminal(await openTerminal());
    await expect(page.locator("#cleanup-notice")).toBeHidden();

    // The boundary stalls the stop past its deadline.
    const stalled = await openTerminal();
    writeFileSync(join(boundary, "stop-stall"), "3");
    await endTerminal(stalled);
    const notice = page.locator("#cleanup-notice");
    await expect(notice).toBeVisible();
    await expect(notice).toContainText("cleanup not confirmed");
    await expect(notice).toContainText("has not confirmed its cleanup");
    await expect(notice.locator(`li[data-session-id="${stalled}"]`)).toHaveCount(1);
    // It stays until dismissed, unlike a toast.
    await page.waitForTimeout(6000);
    await expect(notice).toBeVisible();
    await page.locator("#cleanup-notice-dismiss").click();
    await expect(notice).toBeHidden();
  } finally {
    server.kill();
    await new Promise((done) => server.exitCode !== null ? done() : server.once("exit", done));
    rmSync(fixture, { recursive: true, force: true });
  }
});
