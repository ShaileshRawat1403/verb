import { test, expect } from "@playwright/test";
import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, readdirSync, renameSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createServer } from "node:net";
const TOKEN = "sdlc-fixture-owner-token-00000000000000000000";
const headers = { "X-Verb-Token": TOKEN };
let repo, stateRoot, origin, url, server, spec;
const git = (...args) => execFileSync("git", ["-C", repo, ...args], { encoding: "utf8", env: { PATH: process.env.PATH, GIT_CONFIG_GLOBAL: "/dev/null", GIT_CONFIG_NOSYSTEM: "1" } }).trim();
test.beforeAll(async ({ request }) => {
  const root = mkdtempSync(join(tmpdir(), "verb-sdlc-"));
  repo = join(root, "repo"); stateRoot = join(root, "state"); const home = join(root, "home");
  mkdirSync(repo); mkdirSync(home, { mode: 0o700 });
  const remote = join(root, "remote.git");
  execFileSync("git", ["init", "-q", "--bare", "-b", "main", remote]);
  git("init", "-q", "-b", "main"); git("config", "user.name", "SDLC Fixture"); git("config", "user.email", "fixture@example.invalid");
  writeFileSync(join(repo, ".gitignore"), "__pycache__/\n");
  writeFileSync(join(repo, "feature.py"), "def answer():\n    return 42\n");
  writeFileSync(join(repo, "test_feature.py"), "import os, unittest\nfrom feature import answer\nclass FeatureTest(unittest.TestCase):\n    def test_answer(self):\n        print('NO_RAW_LOG_SDLC_MARKER')\n        self.assertIsNone(os.environ.get('VERB_TOKEN'))\n        self.assertIsNone(os.environ.get('VERB_STATE_DIR'))\n        self.assertEqual(answer(), 42)\n");
  git("add", "."); git("commit", "-qm", "fixture implementation"); git("remote", "add", "origin", remote); git("push", "-q", "-u", "origin", "main");
  const port = await new Promise(ok => { const listener = createServer().listen(0, "127.0.0.1", () => { const port = listener.address().port; listener.close(() => ok(port)); }); });
  origin = `http://127.0.0.1:${port}`; url = `${origin}/#${TOKEN}`;
  server = spawn(resolve(process.env.VERB_BIN ?? "../target/debug/verb"), ["web", "--port", String(port)], { cwd: repo,
    env: { PATH: process.env.PATH, HOME: home, VERB_STATE_DIR: stateRoot, VERB_TOKEN: TOKEN, LANG: "C" }, stdio: ["ignore", "pipe", "pipe"] });
  await new Promise((ok, fail) => { const timer = setTimeout(() => fail(new Error("fixture host did not start")), 20_000);
    server.stdout.on("data", chunk => { if (String(chunk).includes("Verb web UI")) { clearTimeout(timer); ok(); } });
    server.on("exit", () => { clearTimeout(timer); fail(new Error("fixture host exited")); }); });
  const response = await request.post(`${origin}/api/specs`, { headers, data: { title: "Answer survives the workflow", problem: "A reproducible evidence workflow", criteria: ["The feature returns the expected answer"] } });
  expect(response.ok()).toBeTruthy(); spec = await response.json();
});
test.afterAll(() => server?.kill());
const tab = (page, name) => page.locator("#sdlc-tabs").getByRole("button", { name, exact: true }).click();
const action = (page, name) => page.locator("#sdlc-content").getByRole("button", { name, exact: true }).click();
async function save(page) { await page.locator("#sdlc-submit").click(); await expect(page.locator("#sdlc-dialog")).not.toBeVisible({ timeout: 30_000 }); }
async function flow(request) { const response = await request.get(`${origin}/api/specs/${spec.id}/sdlc`, { headers }); expect(response.ok()).toBeTruthy(); return response.json(); }

test("complete evidence, acceptance, review and delivery workflow retains provenance and becomes stale after code changes", async ({ page, request }) => {
  test.setTimeout(120_000);
  await page.goto(url); await page.keyboard.press("Alt+Digit1");
  await page.locator(`#spec-work-items [data-spec-id="${spec.id}"]`).click();
  await expect(page.locator("#sdlc-context")).toContainText("code committed");
  await tab(page, "Plan"); await action(page, "Write plan…");
  await page.locator("#sdlc-text").fill("Implement the answer, run the Python test, review the source, then report the fixture deployment and health.");
  await page.locator("#sdlc-target").fill("phone fixture"); await page.locator("#sdlc-goal").selectOption("service"); await save(page);
  await expect(page.locator("#sdlc-content")).toContainText("service · phone fixture");
  await tab(page, "Verify"); await action(page, "Run a check…"); await page.locator("#sdlc-check").selectOption("python"); await save(page);
  await expect(page.locator(".sdlc-criteria")).toContainText("Verb-observed");
  await expect(page.locator(".sdlc-result")).toHaveText("Not accepted · passed · current");
  await page.getByRole("button", { name: "Accept this evidence…" }).click();
  await page.locator("#sdlc-text").fill("The observed assertion directly checks the promised answer on this source revision."); await save(page);
  await expect(page.locator(".sdlc-result")).toHaveText("Accepted · passed · current");
  await tab(page, "Review"); await action(page, "Inspect implementation changes");
  await expect(page.locator("#sdlc-diff")).toContainText("return 42");
  await action(page, "Record review…"); await page.locator("#sdlc-decision").selectOption("approved");
  await page.locator("#sdlc-text").fill("Reviewed the implementation and accepted check. No additional execution authority was granted."); await save(page);
  await expect(page.locator("#sdlc-content")).toContainText("Review: approved");
  await tab(page, "Ship"); await action(page, "Check remote and record observation…"); await save(page);
  await expect(page.locator("#sdlc-content")).toContainText("service · phone fixture");
  await expect(page.locator("#sdlc-content")).toContainText("Implementation present in the last explicit remote check");
  await action(page, "Record delivery report…"); await page.locator("#sdlc-milestone").selectOption("health");
  await page.locator("#sdlc-text").fill("Fixture health report"); await page.locator("#sdlc-submit").click();
  await expect(page.locator("#sdlc-dialog-error")).toContainText("successful deployment");
  await page.locator("#sdlc-milestone").selectOption("deployment"); await save(page);
  await action(page, "Record delivery report…"); await page.locator("#sdlc-milestone").selectOption("health");
  await page.locator("#sdlc-text").fill("A person checked the fixture target; this is not an independent network probe."); await save(page);
  await expect(page.locator("#sdlc-content")).toContainText("Delivery conditions recorded");
  const complete = await flow(request); expect(complete.ready).toBe(true); expect(complete.goal).toBe("service");
  const exported = await request.get(`${origin}/api/specs/${spec.id}/sdlc-export`, { headers });
  const exportedText = await exported.text();
  expect(exportedText).not.toContain("NO_RAW_LOG_SDLC_MARKER"); expect(exportedText).not.toContain(TOKEN);
  expect(exportedText).not.toContain("python3 -m unittest"); expect(JSON.parse(exportedText).records).toHaveLength(7);
  // A new deployment cannot reuse the previous deployment's health attestation.
  await action(page, "Record delivery report…"); await page.locator("#sdlc-milestone").selectOption("deployment");
  await page.locator("#sdlc-text").fill("Second deployment to the same target"); await save(page);
  expect((await flow(request)).ready).toBe(false);
  await action(page, "Record delivery report…"); await page.locator("#sdlc-milestone").selectOption("health");
  await page.locator("#sdlc-result").selectOption("failed");
  await page.locator("#sdlc-text").fill("Second deployment failed its health check"); await save(page);
  await expect(page.locator("#sdlc-content")).toContainText("Delivery failure reported");
  const listing = await request.get(`${origin}/api/specs`, { headers });
  expect(JSON.stringify((await listing.json()).summaries[spec.id].attention)).toContain("delivery needs attention");
  writeFileSync(join(repo, "feature.py"), "def answer():\n    return 43\n"); git("commit", "-qam", "change implementation after review");
  await tab(page, "Verify"); await expect(page.locator(".sdlc-result")).toHaveText("Not accepted · passed · stale", { timeout: 20_000 });
  await expect(page.getByRole("button", { name: "Accept this evidence…" })).toHaveCount(0);
  await action(page, "Run a check…"); await page.locator("#sdlc-check").selectOption("python"); await save(page);
  await expect(page.locator(".sdlc-result")).toHaveText("Not accepted · failed · current");
  await tab(page, "Review"); await action(page, "Record review…");
  await expect(page.locator('#sdlc-decision option[value="approved"]')).toBeDisabled(); await page.keyboard.press("Escape");
  await page.setViewportSize({ width: 390, height: 844 }); await tab(page, "Verify");
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await page.locator("#sdlc-workflow").scrollIntoViewIfNeeded();
  await page.screenshot({ path: test.info().outputPath("sdlc-verification-phone.png") });
});

test("Python discovery without any tests remains inconclusive", async ({ request }) => {
  renameSync(join(repo, "test_feature.py"), join(repo, "fixture_check.py"));
  git("add", "."); git("commit", "-qm", "fixture without discoverable tests");
  const before = await flow(request);
  const response = await request.post(`${origin}/api/specs/${spec.id}/sdlc`, { headers, data: {
    action: "run", criterion: 0, check: "python", expectedRevision: spec.revision, expectedVersion: before.version, expectedContext: before.context,
  } });
  expect(response.ok()).toBe(true);
  const after = await response.json();
  expect(after.criteria[0].result).toBe("inconclusive"); expect(after.criteria[0].accepted).toBe(false);
});

test("forged observation, arbitrary commands, stale versions and corrupt ledgers fail without replacement", async ({ request }) => {
  const original = await flow(request);
  const base = { expectedRevision: spec.revision, expectedVersion: original.version, expectedContext: original.context };
  for (const data of [
    { ...base, action: "report", criterion: 0, source: "observed", result: "passed", text: "forged observed result" },
    { ...base, action: "run", criterion: 0, check: "python", command: "not permitted" },
    { ...base, expectedVersion: "stale", action: "plan", goal: "code", text: "stale write" },
  ]) {
    const response = await request.post(`${origin}/api/specs/${spec.id}/sdlc`, { headers, data }); expect(response.ok()).toBe(false);
  }
  expect((await flow(request)).version).toBe(original.version);
  const directory = join(stateRoot, "sdlc/v1", readdirSync(join(stateRoot, "sdlc/v1"))[0]);
  const path = join(directory, `${spec.id}.json`); writeFileSync(path, "damaged fixture ledger");
  const response = await request.post(`${origin}/api/specs/${spec.id}/sdlc`, { headers, data: { ...base, action: "plan", goal: "code", text: "must not replace damaged records" } });
  expect(response.ok()).toBe(false); expect(readFileSync(path, "utf8")).toBe("damaged fixture ledger");
});
