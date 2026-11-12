import { test, expect, openWorkspaceView } from "./workspace-navigation.js";
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
async function action(page, name) {
  const menu = page.locator("#bench-review").getByText("Evidence actions", { exact: true });
  await menu.click();
  await page.locator("#bench-review").getByRole("button", { name, exact: true }).click();
}
const review = page => page.locator("#bench-review");
const history = page => page.locator("#sdlc-history");
const runCheck = page => review(page).getByRole("button", { name: "Run a check", exact: true }).click();
async function save(page) { await page.locator("#sdlc-submit").click(); await expect(page.locator("#sdlc-dialog")).not.toBeVisible({ timeout: 30_000 }); }
async function flow(request) { const response = await request.get(`${origin}/api/specs/${spec.id}/sdlc`, { headers }); expect(response.ok()).toBeTruthy(); return response.json(); }

test("Studio reviews a real project check before execution and keeps evidence acceptance separate", async ({ page, request }, testInfo) => {
  const created = await request.post(`${origin}/api/specs`, { headers, data: {
    title: "Studio guided check", problem: "Run a test without memorising commands", criteria: ["The expected answer is returned"],
  } });
  expect(created.ok()).toBeTruthy();
  const chosen = await created.json();
  let runs = 0;
  page.on("request", r => {
    if (r.method() === "POST" && r.url().endsWith(`/api/specs/${chosen.id}/sdlc`) && r.postDataJSON()?.action === "run") runs++;
  });
  await page.goto(url);
  await openWorkspaceView(page, "Studio");
  await page.locator("#studio-thread").selectOption(chosen.id);
  await expect(page.locator("#studio-checks")).toBeVisible();
  await page.getByRole("button", { name: "Run project check", exact: true }).click();
  await expect(page.locator("#sdlc-dialog")).toBeVisible();
  expect(runs).toBe(0);
  await expect(page.locator("#sdlc-check-source")).toContainText(git("rev-parse", "--short=10", "HEAD"));
  await expect(page.locator("#sdlc-check-command")).toHaveText("cargo test --offline");
  await page.locator("#sdlc-check").selectOption("node");
  await expect(page.locator("#sdlc-check-command")).toHaveText("npm --offline test");
  await expect(page.locator("#sdlc-check-description")).toContainText("pretest and posttest");
  await page.locator("#sdlc-check").selectOption("python");
  await expect(page.locator("#sdlc-check-command")).toContainText("suite.countTestCases() == 0");
  await page.keyboard.press("Escape");
  await expect(page.locator("#sdlc-dialog")).toBeHidden();
  await expect(page.locator("#view-studio")).toBeVisible();
  await expect(page.locator("#studio-checks")).toBeVisible();
  expect(runs).toBe(0);
  await page.getByRole("button", { name: "Run project check", exact: true }).click();
  await page.locator("#sdlc-check").selectOption("python");
  let releaseResponse;
  const held = new Promise(resolve => { releaseResponse = resolve; });
  await page.route(`**/api/specs/${chosen.id}/sdlc`, async route => {
    if (route.request().method() !== "POST") return route.continue();
    const response = await route.fetch();
    await held;
    await route.fulfill({ response });
  });
  await page.locator("#sdlc-submit").click();
  try {
    await expect(page.locator("#sdlc-dialog-status")).toContainText("Check running");
    await expect(page.locator("#sdlc-submit")).toBeDisabled();
  } finally { releaseResponse(); }
  await expect(page.locator("#sdlc-dialog")).toBeHidden({ timeout: 30_000 });
  expect(runs).toBe(1);
  await expect(page.locator("#view-studio")).toBeVisible();
  await page.getByRole("button", { name: "Review evidence", exact: true }).click();
  await expect(review(page).getByRole("heading", { name: "Criterion proof", exact: true })).toBeVisible();
  await expect(review(page)).toContainText("Pass awaiting acceptance");
  const response = await request.get(`${origin}/api/specs/${chosen.id}/sdlc`, { headers });
  const result = await response.json();
  expect(result.criteria[0].evidence.source).toBe("observed");
  expect(result.criteria[0].evidence.exitCode).toBe(0);
  expect(result.criteria[0].evidence.binding.commit).toBe(git("rev-parse", "HEAD"));
  expect(JSON.stringify(result)).not.toContain("NO_RAW_LOG_SDLC_MARKER");
  await page.setViewportSize({ width: 390, height: 844 });
  await openWorkspaceView(page, "Studio");
  await page.locator("#studio-focus-work").click();
  await expect(page.locator("#studio-checks")).toBeVisible();
  for (const button of await page.locator(".studio-check-actions button").all())
    expect(Math.round((await button.boundingBox()).height * 1000) / 1000).toBeGreaterThanOrEqual(44);
  await page.getByRole("button", { name: "Run project check", exact: true }).click();
  expect(runs).toBe(1);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await page.locator("#sdlc-dialog").screenshot({ path: testInfo.outputPath("studio-guided-check-phone.png") });
  await page.keyboard.press("Escape");
  // Leave the original workflow test on its own Spec rather than this independent fixture.
});

test("complete evidence, acceptance, review and delivery workflow retains provenance and becomes stale after code changes", async ({ page, request }) => {
  test.setTimeout(120_000);
  await page.goto(url);
  const unlinked = page.locator('#bench-work-tree [data-bench-group="unlinked"]');
  await unlinked.locator("summary").click();
  await unlinked.locator(`[data-bench-spec="${spec.id}"]`).click();
  await page.getByRole("tab", { name: "Review", exact: true }).click();
  await action(page, "Record implementation plan");
  await page.locator("#sdlc-text").fill("Implement the answer, run the Python test, review the source, then report the fixture deployment and health.");
  await page.locator("#sdlc-target").fill("phone fixture"); await page.locator("#sdlc-goal").selectOption("service"); await save(page);
  await runCheck(page);
  await expect(page.locator("#sdlc-check-source")).toContainText("code committed");
  await page.locator("#sdlc-check").selectOption("python"); await save(page);
  await expect(review(page)).toContainText("Verb-observed");
  await expect(review(page)).toContainText("Pass awaiting acceptance");
  await review(page).getByRole("button", { name: "Accept evidence", exact: true }).click();
  await page.locator("#sdlc-text").fill("The observed assertion directly checks the promised answer on this source revision."); await save(page);
  await expect(review(page)).toContainText("Accepted proof");
  await review(page).getByRole("button", { name: "Changes", exact: true }).click();
  await expect(page.locator("#bench-review-diff pre")).toContainText("return 42");
  await page.keyboard.press("Escape");
  await action(page, "Record implementation review"); await page.locator("#sdlc-decision").selectOption("approved");
  await page.locator("#sdlc-text").fill("Reviewed the implementation and accepted check. No additional execution authority was granted."); await save(page);
  await action(page, "Check remote"); await save(page);
  await action(page, "Record delivery report"); await page.locator("#sdlc-milestone").selectOption("health");
  await page.locator("#sdlc-text").fill("Fixture health report"); await page.locator("#sdlc-submit").click();
  await expect(page.locator("#sdlc-dialog-error")).toContainText("successful deployment");
  await page.locator("#sdlc-milestone").selectOption("deployment"); await save(page);
  await action(page, "Record delivery report"); await page.locator("#sdlc-milestone").selectOption("health");
  await page.locator("#sdlc-text").fill("A person checked the fixture target; this is not an independent network probe."); await save(page);
  await action(page, "Evidence history");
  await expect(history(page)).toContainText("health");
  const complete = await flow(request); expect(complete.ready).toBe(true); expect(complete.goal).toBe("service");
  const downloadPromise = page.waitForEvent("download");
  await action(page, "Export evidence metadata");
  const download = await downloadPromise;
  expect(download.suggestedFilename()).toBe(`spec-${spec.id}-evidence.json`);
  const exportedText = readFileSync(await download.path(), "utf8");
  expect(exportedText).not.toContain("NO_RAW_LOG_SDLC_MARKER"); expect(exportedText).not.toContain(TOKEN);
  expect(exportedText).not.toContain("python3 -m unittest"); expect(JSON.parse(exportedText).records).toHaveLength(7);
  await action(page, "Record delivery report"); await page.locator("#sdlc-milestone").selectOption("deployment");
  await page.locator("#sdlc-text").fill("Second deployment to the same target"); await save(page);
  expect((await flow(request)).ready).toBe(false);
  await action(page, "Record delivery report"); await page.locator("#sdlc-milestone").selectOption("health");
  await page.locator("#sdlc-result").selectOption("failed");
  await page.locator("#sdlc-text").fill("Second deployment failed its health check"); await save(page);
  const listing = await request.get(`${origin}/api/specs`, { headers });
  expect(JSON.stringify((await listing.json()).summaries[spec.id].attention)).toContain("delivery needs attention");
  writeFileSync(join(repo, "feature.py"), "def answer():\n    return 43\n"); git("commit", "-qam", "change implementation after review");
  await expect(review(page)).toContainText("Stale proof", { timeout: 20_000 });
  await expect(review(page).getByRole("button", { name: "Accept evidence" })).toHaveCount(0);
  await runCheck(page); await page.locator("#sdlc-check").selectOption("python"); await save(page);
  await expect(review(page).locator(".bench-proof-state")).toHaveText("Failed check");
  await action(page, "Record implementation review");
  await expect(page.locator('#sdlc-decision option[value="approved"]')).toBeDisabled(); await page.keyboard.press("Escape");
  await page.setViewportSize({ width: 390, height: 844 });
  await page.locator("#bench-toggle-right").click();
  await expect(review(page)).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
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
