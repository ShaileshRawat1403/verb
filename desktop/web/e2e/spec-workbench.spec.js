import { test, expect, runPaletteCommand } from "./workspace-navigation.js";
import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, appendFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createServer } from "node:net";

const TOKEN = "spec-workbench-fixture-0000000000000000000";
let server, repo, url, origin, first, second;
const headers = { "X-Verb-Token": TOKEN };
test.beforeAll(async ({ request }) => {
  const root = mkdtempSync(join(tmpdir(), "verb-spec-workbench-"));
  repo = join(root, "repo"); const home = join(root, "home");
  mkdirSync(repo); mkdirSync(home, { mode: 0o700 });
  for (const args of [["init", "-q", "-b", "main"], ["config", "user.name", "Fixture"], ["config", "user.email", "fixture@example.invalid"], ["commit", "--allow-empty", "-qm", "initial"]]) {
    execFileSync("git", ["-C", repo, ...args], { env: { PATH: process.env.PATH, HOME: home } });
  }
  const port = await new Promise(ok => {
    const listener = createServer().listen(0, "127.0.0.1", () => { const port = listener.address().port; listener.close(() => ok(port)); });
  });
  origin = `http://127.0.0.1:${port}`; url = `${origin}/#${TOKEN}`;
  server = spawn(resolve(process.env.VERB_BIN ?? "../target/debug/verb"), ["web", "--port", String(port)], {
    cwd: repo, env: { PATH: process.env.PATH, HOME: home, LANG: "C", VERB_TOKEN: TOKEN, VERB_STATE_DIR: join(root, "state") }, stdio: ["ignore", "pipe", "pipe"],
  });
  await new Promise((ok, fail) => {
    const timer = setTimeout(() => fail(new Error("fixture host did not start")), 20_000);
    server.stdout.on("data", chunk => { if (String(chunk).includes("Verb web UI")) { clearTimeout(timer); ok(); } });
    server.on("exit", () => { clearTimeout(timer); fail(new Error("fixture host exited")); });
  });
  async function post(path, data) {
    const response = await request.post(`${origin}${path}`, { headers, data });
    expect(response.ok()).toBeTruthy(); return response.json();
  }
  first = await post("/api/specs", { title: "Recover sessions", criteria: ["Reconnect restores the session", "Interrupted work remains visible"] });
  first = await post(`/api/specs/${first.id}/criteria/0`, { done: true, evidence: "A person reported checking the fixture; no automated result claimed." });
  second = await post("/api/specs", { title: "Improve contrast <label>", criteria: ["Text is readable"] });
  await post(`/api/specs/${second.id}/stage`, { stage: "review", note: "Fixture review request", expectedRevision: second.revision });
});
test.afterAll(() => server?.kill());

test("a failed spec read exposes an error instead of a false empty project", async ({ page }) => {
  await page.route("**/api/specs", route => route.fulfill({ status: 500, json: { error: "Could not read project specs" } }));
  await page.goto(url);
  await expect(page.locator("#bench-work-tree").getByRole("alert")).toContainText("Could not read project specs");
});

test("an unreadable individual Spec surfaces an error and refuses new IDs without replacing it", async ({ page, request }) => {
  const path = join(repo, first.file), original = readFileSync(path);
  const broken = Buffer.from([0xff, 0xfe, 0xfd]);
  writeFileSync(path, broken);
  try {
    const listing = await request.get(`${origin}/api/specs`, { headers });
    expect(listing.ok()).toBe(false);
    expect((await listing.json()).error).toContain("every spec file");
    const creation = await request.post(`${origin}/api/specs`, { headers, data: { title: "Must not reuse an unreadable ID", criteria: ["Safe identity allocation"] } });
    expect(creation.ok()).toBe(false);
    expect(readFileSync(path).equals(broken)).toBe(true);
    await page.goto(url);
    await expect(page.locator("#bench-work-tree").getByRole("alert")).toContainText("every spec file");
  } finally { writeFileSync(path, original); }
  // The shared host may retain the read failure in its brief state cache. Prove recovery
  // before another journey starts instead of leaking that deliberate failure into it.
  await expect.poll(async () => {
    const restored = await request.get(`${origin}/api/state`, { headers });
    expect(restored.ok()).toBeTruthy();
    return Boolean((await restored.json()).specError);
  }).toBe(false);
  await page.locator("#bench-work-tree").getByRole("button", { name: "Retry" }).click();
  await expect(page.locator("#bench-work-tree").getByRole("alert")).toBeHidden();
  await expect(page.locator("#bench-work-tree [data-bench-spec]")).toHaveCount(2);
});
