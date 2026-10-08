// The spec-driven workbench, driven like a person would: through the browser, against the real host.
import { test, expect } from "@playwright/test";
import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createServer } from "node:net";

const TOKEN = "e2e-token-0000000000000000000000000000";
let server;
let repo;
let url;

const git = (...args) => execFileSync("git", ["-C", repo, ...args], { encoding: "utf8" });
const freePort = () =>
  new Promise((ok) => {
    const s = createServer().listen(0, "127.0.0.1", () => {
      const { port } = s.address();
      s.close(() => ok(port));
    });
  });

test.beforeAll(async () => {
  const bin = resolve(process.env.VERB_BIN ?? "../target/debug/verb");
  repo = mkdtempSync(join(tmpdir(), "verb-e2e-"));
  execFileSync("git", ["init", "-q", "-b", "main", repo]);
  git("config", "user.name", "E2E Person");
  git("config", "user.email", "e2e@example.com");
  writeFileSync(join(repo, "README.md"), "# e2e\n");
  writeFileSync(join(repo, ".gitignore"), ".env\n");
  writeFileSync(join(repo, ".env"), "SECRET_TOKEN=do-not-show\n");
  writeFileSync(join(repo, "AGENTS.md"), "# Team rules\n\nUse tabs.\n");
  git("add", "-A");
  git("commit", "-qm", "initial");
  const port = await freePort();
  url = `http://127.0.0.1:${port}/#${TOKEN}`;
  server = spawn(bin, ["web", "--port", String(port)], {
    cwd: repo,
    env: { ...process.env, VERB_TOKEN: TOKEN },
    stdio: ["ignore", "pipe", "pipe"],
  });
  await new Promise((ok, fail) => {
    const timer = setTimeout(() => fail(new Error("verb web did not start")), 20_000);
    server.stdout.on("data", (chunk) => {
      if (String(chunk).includes("Verb web UI")) {
        clearTimeout(timer);
        ok();
      }
    });
    server.on("exit", (code) => fail(new Error(`verb web exited with ${code}`)));
  });
});

test.afterAll(() => server?.kill());

test("a spec goes from idea to a committed, audited change", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(url);

  // Specs is the home screen and explains the workflow when empty.
  await expect(page.getByRole("heading", { name: "Start with a spec" })).toBeVisible();

  // Create a spec through the guided form.
  await page.getByRole("button", { name: "＋ Write your first spec" }).click();
  await page.locator("#spec-title-input").fill("Let people sign in with email");
  await page.locator("#spec-problem").fill("Returning customers cannot sign in.");
  await page.locator("#spec-criteria").fill("A sign-in link arrives\nA bad email shows a message");
  await page.getByRole("button", { name: "Create spec" }).click();
  await expect(page.locator("#spec-title")).toHaveText("Let people sign in with email");
  await expect(page.locator(".spec-item.current")).toContainText("0/2 proven");

  // Prove a criterion: Verb asks for evidence and records it.
  await page.locator('[data-criterion="1"]').click({ force: true });
  await expect(page.getByRole("heading", { name: "How do you know it works?" })).toBeVisible();
  await page.locator("#evidence-text").fill("unit test rejects a@");
  await page.getByRole("button", { name: "Mark as proven" }).click();
  await expect(page.locator(".spec-item.current")).toContainText("1/2 proven");
  await expect(page.locator("#audit-list li").first()).toContainText("evidence: unit test rejects a@");

  // Skipping stages is allowed, warned about, and shown as skipped.
  await page.locator('#stage-bar [data-stage="build"]').click();
  await expect(page.locator("#stage-warnings")).toContainText("skipped plan");
  await page.getByRole("button", { name: "Move to Build" }).click();
  await expect(page.locator("#stage-bar li.skipped")).toHaveCount(1);

  // Commit from the dialog; the audit line lands in the same commit.
  await page.keyboard.press("Alt+KeyC");
  await expect(page.locator("#commit-message")).toHaveValue(/^spec:001 /);
  await page.locator("#commit-message").fill("spec:001 add the sign-in spec");
  await page.getByRole("button", { name: "Commit", exact: true }).click();
  await expect(page.locator("#git-status")).toHaveText("All changes saved");
  expect(git("log", "-1", "--format=%s").trim()).toBe("spec:001 add the sign-in spec");
  const file = readFileSync(join(repo, "specs/001-let-people-sign-in-with-email.md"), "utf8");
  expect(file).toContain("committed: spec:001 add the sign-in spec");
  expect(git("status", "--porcelain").trim()).toBe("");

  expect(errors).toEqual([]);
});

test("palette, theme and a terminal in the spec", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(url);
  await expect(page.locator("#spec-title")).toBeVisible();

  // The command palette finds and runs actions.
  await page.locator(".palette-hint").click();
  await page.locator("#palette-input").fill("go to overview");
  await page.keyboard.press("Enter");
  await expect(page.locator("#view-overview")).toHaveClass(/active/);
  await page.keyboard.press("Alt+Digit1");
  await expect(page.locator("#view-specs")).toHaveClass(/active/);

  // The theme toggle cycles and the page follows.
  const resolved = () => page.evaluate(() => document.documentElement.dataset.resolvedTheme);
  await page.locator("#theme-toggle").click(); // system → light
  expect(await resolved()).toBe("light");
  await page.locator("#theme-toggle").click(); // light → dark
  expect(await resolved()).toBe("dark");

  // A terminal opened from the spec appears in the spec's work area.
  await page.locator('[data-action="spec-terminal"]').click();
  await expect(page.locator("#spec-work .terminal-tile")).toHaveCount(1, { timeout: 15_000 });
  await expect(page.locator("#spec-work .work-hint")).toBeHidden();
  // A terminal must start without page errors (it once threw a CSP error compiling the image
  // addon's WebAssembly decoder, which silently disabled inline images).
  await page.waitForTimeout(500);
  expect(errors).toEqual([]);
});

test("project view: files, preview, brief, and agent context sync", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(url);
  await page.keyboard.press("Alt+Digit2");
  await expect(page.locator("#view-project")).toHaveClass(/active/);

  // The tree shows what Git tracks or would track, and never ignored files.
  await expect(page.locator('#file-tree [data-file="README.md"]')).toBeVisible();
  await expect(page.locator('#file-tree [data-file=".env"]')).toHaveCount(0);
  await page.locator("#file-filter").fill("env");
  await expect(page.locator('#file-tree [data-file=".env"]')).toHaveCount(0);
  await page.locator("#file-filter").fill("");

  // Read-only preview with line numbers.
  await page.locator('#file-tree [data-file="README.md"]').click();
  await expect(page.locator("#preview-path")).toHaveText("README.md");
  await expect(page.locator(".code-lines li")).toHaveText(["# e2e"]);

  // Nudges point at the next useful step; nothing is forced.
  await expect(page.locator("#hub-nudges")).toContainText("project brief");
  await page.locator('[data-nudge="write-brief"]').click();
  await page.locator("#brief-problem").fill("Shops lose returning customers.");
  await page.locator("#brief-users").fill("Returning shoppers");
  await page.getByRole("button", { name: "Create brief" }).click();
  await expect(page.locator("#preview-path")).toHaveText("docs/project/BRIEF.md");
  await expect(page.locator("#brief-progress")).toHaveText("2/5 filled");

  // Sync writes Verb's section and keeps the user's own text.
  await page.locator('#view-project [data-action="sync-agents"]').click();
  await expect(page.locator('#agent-files li[data-state="current"]')).toHaveCount(2);
  const agents = readFileSync(join(repo, "AGENTS.md"), "utf8");
  expect(agents.startsWith("# Team rules\n\nUse tabs.\n")).toBe(true);
  expect(agents).toContain("**Problem:** Shops lose returning customers.");
  expect(readFileSync(join(repo, "docs/project/BRIEF.md"), "utf8")).toContain("synced agent context into AGENTS.md and CLAUDE.md");

  // The palette finds files.
  await page.locator(".palette-hint").click();
  await page.locator("#palette-input").fill("brief");
  await expect(page.locator("#palette-list")).toContainText("Open file docs/project/BRIEF.md");
  await page.keyboard.press("Escape");
  expect(errors).toEqual([]);
});

test("host view: read-only machine health", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(url);
  await expect(page.locator("#spec-title:visible, #spec-empty h1:visible").first()).toBeVisible();
  await page.keyboard.press("Alt+Digit7");
  await expect(page.locator("#view-host")).toHaveClass(/active/);
  await expect(page.locator("#host-verb")).toContainText("Version");
  await expect(page.locator("#host-verb")).toContainText("Running for");
  await expect(page.locator("#host-memory .usage-number")).toContainText("%");
  await expect(page.locator("#host-storage .usage-bar")).toBeVisible();
  await expect(page.locator("#host-updated")).toContainText("Updated");
  // The sidebar footer opens the same page.
  await page.keyboard.press("Alt+Digit1");
  await page.locator(".sidebar-bottom").click();
  await expect(page.locator("#view-host")).toHaveClass(/active/);
  expect(errors).toEqual([]);
});

test("ask verb answers from evidence and its sources open", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(url);
  await expect(page.locator("#spec-title:visible, #spec-empty h1:visible").first()).toBeVisible();

  // Alt+A opens Ask Verb with starter questions.
  await page.keyboard.press("Alt+KeyA");
  await expect(page.locator("#ask-dialog")).toBeVisible();
  await expect(page.locator("#ask-suggestions")).toContainText("What's left?");

  // The first test proved criterion 2 of spec 001, so one criterion remains.
  await page.locator("#ask-input").fill("What's left on spec 1?");
  await page.keyboard.press("Enter");
  const answer = page.locator(".ask-turn").last();
  await expect(answer.locator(".ask-summary")).toHaveText("1 acceptance criterion still to prove across 1 spec.");
  await expect(answer.locator(".ask-points")).toContainText("criterion 1: A sign-in link arrives");
  await expect(page.locator(".ask-footnote")).toContainText("No AI model was used");

  // A source opens the spec it cites.
  await answer.locator('[data-source-kind="spec"]').click();
  await expect(page.locator("#ask-dialog")).toBeHidden();
  await expect(page.locator("#view-specs")).toHaveClass(/active/);
  await expect(page.locator("#spec-title")).toHaveText("Let people sign in with email");

  // A question typed in the palette is offered to Ask Verb first.
  await page.locator(".palette-hint").click();
  await page.locator("#palette-input").fill("where are we?");
  await expect(page.locator("#palette-list li").first()).toContainText("Ask Verb: where are we?");
  await page.keyboard.press("Enter");
  await expect(page.locator(".ask-turn").last().locator(".ask-summary")).toContainText("acceptance criteria still to prove");

  // An unanswerable question is declined honestly.
  await page.locator("#ask-input").fill("compose me a sonnet");
  await page.keyboard.press("Enter");
  await expect(page.locator(".ask-turn").last()).toContainText("I only answer from evidence");
  expect(errors).toEqual([]);
});

test("session board and handoff with a written note", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(url);
  await expect(page.locator("#spec-title")).toBeVisible();

  // An earlier test opened a terminal from the spec; the board lists it from the audit trail.
  await expect(page.locator("#spec-sessions")).toBeVisible();
  await expect(page.locator("#spec-sessions .board-row").first()).toContainText("Terminal");

  // Hand off (to a plain terminal here: CI has no agent CLIs installed).
  await page.locator('[data-action="handoff"]').click();
  await expect(page.locator("#handoff-from")).toHaveText("Terminal");
  await page.locator("#handoff-to").selectOption("shell");
  await page.locator("#handoff-note").fill("Validation is done; persistence is next.");
  await page.locator("#handoff-submit").click();
  await expect(page.locator("#handoff-dialog")).toBeHidden();
  await expect(page.locator("#spec-sessions .board-row.live")).toHaveCount(2, { timeout: 15_000 });

  const file = readFileSync(join(repo, "specs/001-let-people-sign-in-with-email.md"), "utf8");
  expect(file).toContain("## Handoff notes");
  expect(file).toContain("shell → shell");
  expect(file).toContain("- Note from E2E Person: Validation is done; persistence is next.");
  expect(file).toContain("handed off from shell to shell — Validation is done; persistence is next.");
  expect(file.indexOf("## Handoff notes")).toBeLessThan(file.indexOf("## Audit trail"));
  await expect(page.locator("#audit-list li").first()).toContainText("started shell on this spec");

  // Context meters cover running Claude/Codex sessions only; plain terminals have none.
  const meters = await page.request.get(url.replace(/#.*/, "api/meters"), { headers: { "X-Verb-Token": TOKEN } });
  expect(meters.ok()).toBe(true);
  expect(await meters.json()).toEqual({ meters: {} });
  expect(errors).toEqual([]);
});

test("observer: opt-in badges from real terminal activity, secrets by kind only", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const api = (path, body) =>
    page.request[body ? "post" : "get"](url.replace(/#.*/, path), {
      headers: { "X-Verb-Token": TOKEN, ...(body ? { "Content-Type": "application/json" } : {}) },
      ...(body ? { data: body } : {}),
    });
  await page.goto(url);
  await expect(page.locator("#spec-title")).toBeVisible();

  // Off until chosen, and it explains its boundaries before asking.
  await expect(page.locator("#observer-label")).toHaveText("Observer off");
  await page.locator("#observer-pill").click();
  await expect(page.locator("#observer-panel")).toContainText("never acts on its own");
  await page.locator('[data-observer="enable"]').click();
  // Earlier tests left uncommitted work on main while spec 001 (in Build) has its own branch, so
  // the browser-side wrong-branch signal is correctly the first thing it notices.
  await expect(page.locator("#observer-label")).toHaveText("1 to look at");
  await expect(page.locator('.observer-badge[data-kind="branch"]')).toContainText("spec 001 has its own branch");
  await page.locator('[data-action="observer-close"]').click();

  // Real terminal activity: a fake token on screen, and the same command failing three times.
  await page.locator('[data-action="spec-terminal"]').click();
  const tile = page.locator("#spec-work .terminal-tile").last();
  await tile.locator(".terminal-mount").click();
  const fake = "ghp_" + "abcdefghijklmnopqrstuvwxyz0123456789";
  await page.keyboard.type(`echo ${fake}\n`);
  for (let i = 0; i < 3; i += 1) await page.keyboard.type("ls /verb-e2e-missing\n");

  await expect
    .poll(async () => (await (await api("api/observer")).json()).signals.map((s) => s.kind).sort(), { timeout: 15_000 })
    .toEqual(["failing", "secret"]);
  const raw = await (await api("api/observer")).text();
  expect(raw).not.toContain(fake.slice(4)); // the value never leaves the server

  await page.locator("#observer-pill").click();
  await expect(page.locator("#observer-label")).toHaveText("3 to look at");
  await expect(page.locator(".observer-badge")).toHaveCount(3);
  await expect(page.locator('.observer-badge[data-kind="secret"]')).toContainText("Possible secret on screen");
  await expect(page.locator('.observer-badge[data-kind="failing"] .badge-title')).toHaveText("`ls /verb-e2e-missing` failed 3 times");

  // Dismiss hides one; muting a kind hides the rest of it and is remembered server-side.
  await page.locator('.observer-badge[data-kind="secret"] [data-observer="dismiss"]').click();
  await expect(page.locator(".observer-badge")).toHaveCount(2);
  await page.locator('.observer-badge[data-kind="failing"] [data-observer="mute"]').click();
  await expect(page.locator(".observer-badge")).toHaveCount(1);
  await expect(page.locator('.observer-badge[data-kind="branch"]')).toBeVisible();
  expect((await (await api("api/observer")).json()).muted).toEqual(["failing"]);

  await page.locator('[data-observer="disable"]').click();
  await expect(page.locator("#observer-label")).toHaveText("Observer off");
  await api("api/observer", { unmute: "failing" });
  expect(errors).toEqual([]);
});
