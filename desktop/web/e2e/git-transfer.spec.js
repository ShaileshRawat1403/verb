// Real Git, local bare remote, isolated home: no credentials or external network.
import { test, expect } from "@playwright/test";
import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createServer } from "node:net";

const TOKEN = "git-transfer-fixture-000000000000000000000";
let server, local, other, remote, url;
const git = (at, ...args) => execFileSync("git", ["-C", at, ...args], {
  encoding: "utf8", env: { PATH: process.env.PATH, GIT_CONFIG_GLOBAL: "/dev/null", GIT_CONFIG_NOSYSTEM: "1" },
}).trim();

test.beforeAll(async () => {
  const root = mkdtempSync(join(tmpdir(), "verb-transfer-browser-"));
  local = join(root, "local"); other = join(root, "other"); remote = join(root, "remote.git");
  const home = join(root, "home"); mkdirSync(home, { mode: 0o700 });
  git(root, "init", "-q", "--bare", "-b", "main", remote);
  git(root, "clone", "-q", remote, local);
  git(local, "config", "user.name", "Transfer Fixture");
  git(local, "config", "user.email", "fixture@example.invalid");
  writeFileSync(join(local, "work.txt"), "initial\n");
  git(local, "add", "."); git(local, "commit", "-qm", "initial");
  git(local, "push", "-q", "-u", "origin", "main");
  git(root, "clone", "-q", remote, other);
  git(other, "config", "user.name", "Other Fixture");
  git(other, "config", "user.email", "other@example.invalid");
  const port = await new Promise(ok => {
    const listener = createServer().listen(0, "127.0.0.1", () => {
      const port = listener.address().port;
      listener.close(() => ok(port));
    });
  });
  url = `http://127.0.0.1:${port}/#${TOKEN}`;
  server = spawn(resolve(process.env.VERB_BIN ?? "../target/debug/verb"), ["web", "--port", String(port)], {
    cwd: local, env: { PATH: process.env.PATH, HOME: home, LANG: "C", VERB_STATE_DIR: join(root, "state"), VERB_TOKEN: TOKEN },
    stdio: ["ignore", "pipe", "pipe"],
  });
  await new Promise((ok, fail) => {
    const timer = setTimeout(() => fail(new Error("fixture host did not start")), 20_000);
    server.stdout.on("data", chunk => { if (String(chunk).includes("Verb web UI")) { clearTimeout(timer); ok(); } });
    server.on("exit", () => { clearTimeout(timer); fail(new Error("fixture host exited")); });
  });
});
test.afterAll(() => server?.kill());

test("a person checks, reviews, uploads and downloads without losing diverged work", async ({ page }) => {
  test.setTimeout(120_000);
  writeFileSync(join(local, "work.txt"), "local contribution\n");
  git(local, "commit", "-qam", "local contribution");
  const actions = [];
  page.on("request", request => {
    if (request.method() === "POST" && request.url().endsWith("/api/git/transfer")) actions.push(request.postDataJSON().action);
  });
  await page.goto(url);
  await page.keyboard.press("Alt+Digit2");
  const open = () => page.getByRole("button", { name: "Upload / download…", exact: true }).click();
  const status = page.locator("#git-transfer-status");
  const upload = page.locator("#git-transfer-upload");
  const download = page.locator("#git-transfer-download");
  const check = page.locator("#git-transfer-check");
  await open();
  await expect(status).toContainText("1 local commit to upload");
  await expect(upload).toBeDisabled();
  await expect(page.locator("#git-transfer-commits")).toContainText("local contribution");
  await expect(download).toBeDisabled();
  expect(actions).toEqual([]); // Opening a review never contacts the remote.
  await check.click();
  await expect(upload).toBeEnabled();
  await upload.click();
  await expect(status).toContainText("Git transfer completed.", { timeout: 30_000 });
  expect(git(remote, "rev-parse", "main")).toBe(git(local, "rev-parse", "HEAD"));
  await page.keyboard.press("Escape");

  git(other, "pull", "--ff-only", "-q");
  writeFileSync(join(other, "work.txt"), "remote contribution\n");
  git(other, "commit", "-qam", "remote contribution"); git(other, "push", "-q");
  await open(); await check.click();
  await expect(download).toBeEnabled();
  await expect(page.locator("#git-transfer-commits")).toContainText("remote contribution");
  await download.click();
  await expect(status).toContainText("Git transfer completed.", { timeout: 30_000 });
  expect(readFileSync(join(local, "work.txt"), "utf8")).toBe("remote contribution\n");
  await page.keyboard.press("Escape");

  writeFileSync(join(local, "work.txt"), "local divergent work\n");
  git(local, "commit", "-qam", "local divergent work");
  writeFileSync(join(other, "work.txt"), "remote divergent work\n");
  git(other, "commit", "-qam", "remote divergent work"); git(other, "push", "-q");
  await open(); await check.click();
  await expect(status).toContainText("Both this branch and its remote have new commits");
  await expect(upload).toBeDisabled(); await expect(download).toBeDisabled();
  expect(readFileSync(join(local, "work.txt"), "utf8")).toBe("local divergent work\n");
  expect(git(local, "status", "--porcelain")).toBe("");
  expect(actions).toEqual(["check", "upload", "check", "download", "check"]);
  await page.setViewportSize({ width: 390, height: 844 });
  for (const control of [check, upload, download]) {
    const bounds = await control.boundingBox();
    expect(bounds.x).toBeGreaterThanOrEqual(0);
    expect(bounds.x + bounds.width).toBeLessThanOrEqual(390);
  }
  await page.screenshot({ path: test.info().outputPath("git-transfer-phone.png") });
});
