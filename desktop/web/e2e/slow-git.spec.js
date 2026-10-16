import { test, expect } from "@playwright/test";
import { execFileSync, spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, unlinkSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const TOKEN = "synthetic-slow-git-browser-token";
const quote = (value) => `'${value.replaceAll("'", "'\"'\"'")}'`;
const freePort = () => new Promise((resolvePort) => {
  const socket = createServer().listen(0, "127.0.0.1", () => {
    const { port } = socket.address();
    socket.close(() => resolvePort(port));
  });
});

for (const [name, endpoint] of [
  ["summary", "/api/git"],
  ["diff", "/api/git/diff?path=README.md"],
]) {
  test(`slow Git ${name} does not block terminal launch or input`, async ({ page }) => {
    test.skip(process.platform === "win32", "POSIX synthetic Git gate");
    const fixture = mkdtempSync(join(tmpdir(), "verb-slow-git-"));
    const repo = join(fixture, "project");
    const home = join(fixture, "home");
    const bin = join(fixture, "bin");
    const gate = join(fixture, "gate");
    const started = join(fixture, "started");
    for (const path of [repo, home, bin]) mkdirSync(path, { mode: 0o700 });
    const realGit = execFileSync("/usr/bin/which", ["git"], { encoding: "utf8" }).trim();
    const environment = { PATH: process.env.PATH, HOME: home, LANG: "C", SHELL: "/bin/bash" };
    execFileSync(realGit, ["init", "-q", "-b", "main", repo], { env: environment });
    writeFileSync(join(repo, "README.md"), "Synthetic slow-Git fixture.\n");
    // An operator-selected PATH executable outside the project holds a real Git
    // status read. No repository hook or production-only sleep flag is needed.
    writeFileSync(join(bin, "git"), `#!/bin/sh
for arg do
  if [ "$arg" = "--porcelain=v1" ] && [ -e ${quote(gate)} ]; then
    : > ${quote(started)}
    while [ -e ${quote(gate)} ]; do sleep 0.05; done
  fi
done
exec ${quote(realGit)} "$@"
`, { mode: 0o755 });
    const port = await freePort();
    const origin = `http://127.0.0.1:${port}`;
    const server = spawn(resolve(process.env.VERB_BIN ?? "../target/debug/verb"), ["web", "--port", String(port)], {
      cwd: repo,
      env: { ...environment, PATH: `${bin}:${environment.PATH}`, VERB_STATE_DIR: join(fixture, "state"), VERB_TOKEN: TOKEN },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let pending;
    let sessionId;
    try {
      await new Promise((ready, reject) => {
        const timer = setTimeout(() => reject(new Error("synthetic host did not start")), 15_000);
        server.stdout.on("data", (data) => {
          if (String(data).includes("Verb web UI")) { clearTimeout(timer); ready(); }
        });
        server.once("exit", () => { clearTimeout(timer); reject(new Error("synthetic host exited")); });
      });
      const initialGit = page.waitForResponse((response) => response.url() === `${origin}/api/git`);
      await page.goto(`${origin}/#${TOKEN}`);
      expect((await initialGit).status()).toBe(200);
      writeFileSync(gate, "synthetic gate\n");
      pending = page.request.get(`${origin}${endpoint}`, { headers: { "X-Verb-Token": TOKEN } });
      // Prove Git is actually blocked before testing unrelated terminal work.
      await expect.poll(() => existsSync(started)).toBe(true);
      await page.locator("#quick-terminal-empty").click();
      const input = page.getByRole("textbox", { name: "Terminal input" });
      await expect(input).toBeVisible({ timeout: 2000 });
      sessionId = (await page.locator(".terminal-tile").getAttribute("id")).slice("terminal-".length);
      await input.pressSequentially("printf 'SLOW_GIT_%s\\n' INPUT_OK");
      await input.press("Enter");
      await expect.poll(async () => {
        const response = await page.request.get(`${origin}/api/terminals/${sessionId}/output`, { headers: { "X-Verb-Token": TOKEN } });
        return Buffer.from((await response.json()).data, "base64").toString();
      }, { timeout: 2000 }).toContain("SLOW_GIT_INPUT_OK");
      expect(existsSync(gate)).toBe(true);
      unlinkSync(gate);
      const result = await pending;
      expect(result.status()).toBe(200);
      const body = await result.json();
      expect(name === "summary" ? body.changes.some((file) => file.path === "README.md") : body.path === "README.md").toBe(true);
    } finally {
      if (existsSync(gate)) unlinkSync(gate);
      await pending?.catch(() => {});
      if (sessionId) await page.request.delete(`${origin}/api/terminals/${sessionId}`, { headers: { "X-Verb-Token": TOKEN } }).catch(() => {});
      server.kill();
      await new Promise((done) => server.exitCode !== null ? done() : server.once("exit", done));
    }
  });
}
