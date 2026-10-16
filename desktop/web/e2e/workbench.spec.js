// The spec-driven workbench, driven like a person would: through the browser, against the real host.
import { test, expect } from "@playwright/test";
import { spawn, execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
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
  const fixture = mkdtempSync(join(tmpdir(), "verb-e2e-"));
  repo = join(fixture, "project");
  const home = join(fixture, "home");
  mkdirSync(repo);
  mkdirSync(home, { mode: 0o700 });
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
    // Never load the developer's shell config, tokens, transcripts or durable session store.
    env: { PATH: process.env.PATH, HOME: home, LANG: "C", VERB_STATE_DIR: join(fixture, "state"), VERB_TOKEN: TOKEN },
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

test("a fresh project opens directly into a native terminal", async ({ page }) => {
  const errors = [];
  page.on("pageerror", e => errors.push(e.message));
  await page.goto(url);
  await expect(page.locator("#view-sessions")).toHaveClass(/active/);
  expect(await page.locator("#toast").evaluate((el) => getComputedStyle(el).opacity)).toBe("0");
  await page.locator("#quick-terminal-empty").click();
  const input = page.getByRole("textbox", { name: "Terminal input" });
  await expect(input).toBeVisible();
  expect(await page.locator("#view-sessions").evaluate((el) => getComputedStyle(el).animationName)).toBe("none");
  await input.pressSequentially("printf '\\164erminal-first-ok\\n'");
  await input.press("Enter");
  const sid = (await page.locator(".terminal-tile").getAttribute("id")).slice("terminal-".length);
  const origin = url.split("/#")[0];
  await expect.poll(async () => {
    const result = await page.request.get(`${origin}/api/terminals/${sid}/output`,
      { headers: { "X-Verb-Token": TOKEN } });
    return Buffer.from((await result.json()).data, "base64").toString();
  }).toContain("terminal-first-ok");
  await page.locator("[data-close-terminal]").click();
  await page.locator("#end-session-confirm").click();
  await expect(page.locator("#quick-terminal-empty")).toBeVisible();
  expect(errors).toEqual([]);
});

test("terminal tabs keep keyboard focus and finished output stays readable", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(url);
  await page.locator("#quick-terminal-empty").click();
  await page.locator("[data-terminal-new]").click();
  const tabs = page.getByRole("tab", { name: /Terminal/ });
  await expect(tabs).toHaveCount(2);
  await tabs.last().focus();
  await page.keyboard.press("ArrowLeft");
  await expect(tabs.first()).toBeFocused();
  await expect(tabs.first()).toHaveAttribute("aria-selected", "true");
  const closedId = await tabs.first().getAttribute("data-tab-terminal");
  await page.waitForTimeout(2500); // Includes a background state refresh.
  await expect(tabs.first()).toBeFocused();
  const input = page.getByRole("textbox", { name: "Terminal input" }).filter({ visible: true });
  await input.pressSequentially("exit");
  await input.press("Enter");
  const close = page.getByRole("button", { name: "Close terminal output", exact: true });
  await expect(close).toBeVisible();
  await page.waitForTimeout(4500); // Includes the lightweight workspace refresh after exit.
  await expect(close).toBeVisible();
  await expect(tabs).toHaveCount(2);
  await page.route("**/api/terminals/*", async (route) => {
    if (route.request().method() === "DELETE")
      return route.fulfill({ status: 503, contentType: "application/json", body: JSON.stringify({error: "Temporarily unavailable"}) });
    return route.continue();
  });
  await close.click();
  await expect(page.locator("#toast")).toHaveText(/Temporarily unavailable/);
  await expect(tabs).toHaveCount(2);
  await page.unroute("**/api/terminals/*");
  await close.click();
  await expect(page.locator("#end-session-dialog")).not.toBeVisible();
  await expect(tabs).toHaveCount(1);
  await page.waitForTimeout(2500); // A closed output buffer must stay closed after refresh.
  await expect(tabs).toHaveCount(1);
  const origin = url.split("/#")[0];
  const state = await page.request.get(`${origin}/api/state`, { headers: { "X-Verb-Token": TOKEN } });
  const record = (await state.json()).sessions.find((session) => session.id === closedId);
  expect(record).toBeDefined();
  expect(record.hasTerminal).toBe(false);
  await page.locator("[data-close-terminal]:visible").click();
  await page.locator("#end-session-confirm").click();
  await expect(page.locator("#quick-terminal-empty")).toBeVisible();
  expect(errors).toEqual([]);
});

test("refresh restores the selected terminal and ignores stale saved selections", async ({ page }) => {
  await page.goto(url);
  await page.locator("#quick-terminal-empty").click();
  await page.locator("[data-terminal-new]").click();
  const tabs = page.getByRole("tab", { name: /Terminal/ });
  await expect(tabs).toHaveCount(2);
  const ids = await tabs.evaluateAll((items) => items.map((item) => item.dataset.tabTerminal));
  const origin = url.split("/#")[0];
  const workspace = await page.request.get(`${origin}/api/workspace`, { headers: { "X-Verb-Token": TOKEN } });
  const projectId = (await workspace.json()).project.id;
  try {
    // Exercise both directions so server ordering cannot accidentally satisfy restoration.
    for (const id of [ids[1], ids[0], ids[1]]) {
      await page.locator(`[data-tab-terminal="${id}"]`).click();
      await page.reload();
      await expect(tabs).toHaveCount(2);
      await expect(page.locator(`[data-tab-terminal="${id}"]`)).toHaveAttribute("aria-selected", "true");
      await expect(page.locator(`#terminal-${id}`)).toHaveClass(/focused/);
      expect((await tabs.evaluateAll((items) => items.map((item) => item.dataset.tabTerminal))).sort()).toEqual([...ids].sort());
    }
    await page.evaluate((id) => sessionStorage.setItem(`verb.terminal-focus.${id}`, "gone-terminal"), projectId);
    await page.reload();
    await expect(tabs).toHaveCount(2);
    await expect(page.locator('.terminal-tab[aria-selected="true"]')).toHaveCount(1);
    await expect.poll(() => page.evaluate((id) => sessionStorage.getItem(`verb.terminal-focus.${id}`), projectId)).not.toBe("gone-terminal");
  } finally {
    for (const id of ids)
      await page.request.delete(`${origin}/api/terminals/${id}`, { headers: { "X-Verb-Token": TOKEN, Origin: origin } });
  }
});

test("reduced motion preserves an immediate terminal surface", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto(url);
  await page.locator(".palette-hint").click();
  expect(await page.locator("#palette-dialog").evaluate((el) => getComputedStyle(el).animationName)).toBe("none");
  await page.keyboard.press("Escape");
  await page.locator("#quick-terminal-empty").click();
  const view = await page.locator("#view-sessions").evaluate((el) => ({
    animation: getComputedStyle(el).animationName,
    transform: getComputedStyle(el).transform,
  }));
  expect(view).toEqual({ animation: "none", transform: "none" });
  const input = page.getByRole("textbox", { name: "Terminal input" });
  await input.pressSequentially("exit");
  await input.press("Enter");
  await page.getByRole("button", { name: "Close terminal output", exact: true }).click();
});

test("a late polling failure cannot overwrite a connected stream", async ({ page }) => {
  let releasePoll;
  let pollStarted;
  const pendingPoll = new Promise((resolve) => { pollStarted = resolve; });
  await page.routeWebSocket("**/api/terminals/ws?*", async (socket) => {
    await pendingPoll;
    socket.connectToServer();
  });
  await page.route("**/api/terminals/*/output?*", (route) => {
    releasePoll = () => route.fulfill({ status: 200, contentType: "application/json", body: "" });
    pollStarted();
  });
  await page.goto(url);
  await page.locator("#quick-terminal-empty").click();
  await pendingPoll;
  await expect(page.locator(".terminal-state")).toHaveText("Connected to Verb (streaming)");
  await releasePoll();
  await expect(page.locator(".terminal-state")).toHaveText("Connected to Verb (streaming)");
  await expect(page.locator(".terminal-state")).not.toHaveAttribute("data-tone", "warn");
  await expect(page.locator("#toast")).not.toHaveClass(/visible/);
  const input = page.getByRole("textbox", { name: "Terminal input" });
  await input.pressSequentially("exit");
  await input.press("Enter");
  await page.getByRole("button", { name: "Close terminal output", exact: true }).click();
  await expect(page.locator("#quick-terminal-empty")).toBeVisible();
});

test("a spec goes from idea to a committed, audited change", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(url);

  // The default workspace is a terminal; specs remain an explicit optional workflow.
  await expect(page.locator("#view-sessions")).toHaveClass(/active/);
  await page.locator('.nav-item[data-view="specs"]').click();
  await expect(page.getByRole("heading", { name: "Start with a spec" })).toBeVisible();

  // Create a spec through the guided form.
  await page.getByRole("button", { name: "Write your first spec" }).click();
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

  // The new spec file is reviewable as a diff before committing: every line added.
  const card = page.locator('.diff-card[data-diff-path="specs/001-let-people-sign-in-with-email.md"]');
  await card.locator("summary").click();
  await expect(card.locator(".diff-line.add").first()).toBeVisible();
  await expect(card.locator(".diff-line.del")).toHaveCount(0);

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
  await page.locator('.nav-item[data-view="specs"]').click();
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

  // Markdown reads formatted, with Raw for the exact lines; everything stays read-only.
  await page.locator('#file-tree [data-file="README.md"]').click();
  await expect(page.locator("#preview-path")).toHaveText("README.md");
  await expect(page.locator(".md-preview h1")).toHaveText("e2e");
  await page.locator("#preview-mode").click();
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
  await expect(page.locator("#view-sessions")).toHaveClass(/active/);
  await page.keyboard.press("Alt+Digit7");
  await expect(page.locator("#view-host")).toHaveClass(/active/);
  await expect(page.locator("#host-verb")).toContainText("Version");
  await expect(page.locator("#host-verb")).toContainText("Build");
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

test("host supervisor visibility does not imply service outage", async ({ page }) => {
  await page.route("**/api/host", async route => {
    const response = await route.fetch();
    const report = await response.json();
    report.services = [{name:"verb",state:"unknown",uptime_secs:null},{name:"worker",state:"down",uptime_secs:0}];
    await route.fulfill({response,json:report});
  });
  await page.goto(url);
  await page.keyboard.press("Alt+Digit7");
  const services = page.locator("#host-services");
  await expect(services).toContainText("Supervisor status unavailable");
  await expect(services).toContainText("does not mean those services are stopped");
  await expect(services.locator("tr").filter({hasText:"verb"}).locator(".state-dot")).toHaveAttribute("data-state","idle");
  await expect(services.locator("tr").filter({hasText:"worker"}).locator(".state-dot")).toHaveAttribute("data-state","bad");
});

test("the older views sit under More, on a desktop and on a phone", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(url);
  await expect(page.locator("#view-sessions")).toHaveClass(/active/);
  await page.locator(".nav-more > summary").click();
  await page.locator('.nav-item[data-view="memory"]').click();
  await expect(page.locator("#view-memory h1")).toHaveText("Shared notes");
  await expect(page.locator("#breadcrumb-view")).toHaveText("Shared notes");
  // On a phone, More is a menu: it must open, lead somewhere, and close behind the choice.
  await page.setViewportSize({ width: 390, height: 844 });
  await page.locator('.nav-item[data-view="specs"]').click();
  await expect(page.locator("#nav-more")).not.toHaveAttribute("open", "");
  await page.locator(".nav-more > summary").click();
  await expect(page.locator('.nav-item[data-view="tasks"]')).toBeVisible();
  await page.locator('.nav-item[data-view="tasks"]').click();
  await expect(page.locator("#view-tasks")).toHaveClass(/active/);
  await expect(page.locator("#nav-more")).not.toHaveAttribute("open", "");
  expect(errors).toEqual([]);
});

test("ask verb answers from evidence and its sources open", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(url);
  await expect(page.locator("#view-sessions")).toHaveClass(/active/);

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
  await page.locator('.nav-item[data-view="specs"]').click();
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
  await page.locator('.nav-item[data-view="specs"]').click();
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
  // Terminals still running from earlier tests come back with the page, so "the new one" is the
  // selected tab once one more terminal exists, not necessarily the last in the page.
  const before = await page.locator("#spec-work .terminal-tile").count();
  await page.locator('[data-action="spec-terminal"]').click();
  await expect(page.locator("#spec-work .terminal-tile")).toHaveCount(before + 1);
  const tile = page.locator("#spec-work .terminal-tile.focused");
  await tile.locator(".terminal-mount").click();
  const fake = "ghp_" + "abcdefghijklmnopqrstuvwxyz0123456789";
  await page.keyboard.type(`echo ${fake}\n`);
  for (let i = 0; i < 3; i += 1) await page.keyboard.type("ls /verb-e2e-missing\n");

  await expect
    .poll(async () => (await (await api("api/observer")).json()).signals.map((s) => s.kind).sort(), { timeout: 15_000 })
    .toEqual(["failing", "secret"]);
  // The same commands are blocks in the terminal: a red edge and the exit code on each failure,
  // and no "Ask agent" while no agent is open to hand to.
  await expect(tile.locator(".cmd-edge.failed")).toHaveCount(3);
  await expect(tile.locator(".cmd-meta.failed").first()).toContainText(/exit [12]/);
  await expect(tile.locator(".cmd-meta.failed").first().getByRole("button", { name: "Copy" })).toBeVisible();
  await expect(tile.getByRole("button", { name: "Ask agent" })).toHaveCount(0);
  // Its tab shows the failure too, and is the selected one.
  const selectedTab = page.locator('#spec-work .terminal-tab[aria-selected="true"]');
  await expect(selectedTab.locator(".tab-dot")).toHaveAttribute("data-tone", "failed");
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

test("terminal input stays hidden when the real PTY disables echo", async ({ page }) => {
  const probe = join(repo, "no-echo-probe.py");
  writeFileSync(probe, `import termios,sys,time\noriginal=termios.tcgetattr(0)\nhidden=original.copy()\nhidden[3] &= ~termios.ECHO\ntermios.tcsetattr(0,termios.TCSANOW,hidden)\nprint('NO_ECHO_READY',flush=True)\ntry:\n line=sys.stdin.readline()\nfinally:\n termios.tcsetattr(0,termios.TCSANOW,original)\nprint('INPUT_ACCEPTED',flush=True)\ntime.sleep(30)\n`);
  const origin = url.split("/#")[0];
  const headers = { "X-Verb-Token": TOKEN, Origin: origin };
  const response = await page.request.post(`${origin}/api/terminals`, {
    headers, data: { agent: "custom", command: "python3", args: ["-u", probe], isolated: false },
  });
  expect(response.status()).toBe(201);
  const { sessionId } = await response.json();
  try {
    // Slow delivery used to activate speculative echo and expose hidden input.
    await page.route("**/api/terminals/*/output*", async (route) => {
      await new Promise((ok) => setTimeout(ok, 250));
      await route.continue();
    });
    await page.goto(url);
    await page.keyboard.press("Alt+Digit3");
    await expect(page.locator("#view-sessions")).toHaveClass(/active/);
    await page.locator(`#session-list [data-session-id="${sessionId}"]`).click();
    const tile = page.locator(`#terminal-${sessionId}`);
    await expect(tile).toBeVisible();
    const output = async () => {
      const response = await page.request.get(`${origin}/api/terminals/${sessionId}/output`, { headers });
      return Buffer.from((await response.json()).data, "base64").toString();
    };
    await expect.poll(output).toContain("NO_ECHO_READY");
    await tile.locator(".xterm-helper-textarea").focus();
    await page.keyboard.type("SYNTHETIC_HIDDEN_INPUT", { delay: 80 });
    await expect(tile).not.toContainText("SYNTHETIC_HIDDEN_INPUT");
    await expect(tile.locator(".typeahead")).toHaveCount(0);
    await page.keyboard.press("Enter");
    await expect.poll(output).toContain("INPUT_ACCEPTED");
    expect(await output()).not.toContain("SYNTHETIC_HIDDEN_INPUT");
  } finally {
    await page.request.delete(`${origin}/api/terminals/${sessionId}`, { headers });
  }
});

test("Git diagnostics survive lightweight terminal refreshes", async ({page}) => {
  let facts = {gitStatus:"unavailable",branch:null,changedFiles:null};
  await page.route("**/api/state",async route => {
    const response=await route.fetch();const data=await response.json();
    data.project={...data.project,...facts};
    await route.fulfill({response,json:data});
  });
  await page.goto(url);
  await expect(page.locator("#top-branch")).toHaveText("Git status unavailable");
  await expect(page.locator("#top-changes")).toHaveText("Changes unavailable");
  await page.waitForTimeout(4500); // Includes a workspace poll that must retain the completed verdict.
  await expect(page.locator("#top-branch")).toHaveText("Git status unavailable");
  for (const [state,branch,label,count] of [["unborn","main","main · no commits","0 changed"],["not_repository",null,"Not a Git repository","No Git tracking"]]) {
    facts={gitStatus:state,branch,changedFiles:state === "unborn" ? 0 : null};
    await page.reload();
    await expect(page.locator("#top-branch")).toHaveText(label);
    await expect(page.locator("#top-changes")).toHaveText(count);
  }
});

test("real detached Git checkout is diagnosed without changing terminal availability", async ({page}) => {
  const branch=git("symbolic-ref","--short","HEAD").trim();
  try {
    git("checkout","--detach","-q");
    const origin=url.split("/#")[0];
    await expect.poll(async()=> (await (await page.request.get(`${origin}/api/state`,{headers:{"X-Verb-Token":TOKEN}})).json()).project.gitStatus).toBe("detached");
    await page.goto(url);
    await expect(page.locator("#top-branch")).toHaveText("Detached HEAD");
    const launch=page.locator("#quick-terminal-button, #quick-terminal-sessions, #quick-terminal-empty").filter({visible:true}).first();
    await expect(launch).toBeVisible();
    await launch.click();
    const tile=page.locator(".terminal-tile.focused");
    await expect(tile).toBeVisible();
    const id=(await tile.getAttribute("id")).slice("terminal-".length);
    await page.request.delete(`${origin}/api/terminals/${id}`,{headers:{"X-Verb-Token":TOKEN,Origin:origin}});
  } finally { git("checkout","-q",branch); }
});

test("two native sessions share project files and reattach to the same processes", async ({page}) => {
  const origin=url.split("/#")[0];const headers={"X-Verb-Token":TOKEN,Origin:origin};
  // Earlier stateful journeys deliberately retain fixture sessions. Clear only this disposable host.
  const workspace=await (await page.request.get(`${origin}/api/workspace`,{headers})).json();
  for (const id of Object.keys(workspace.terminalStates || {})) {
    const response=await page.request.delete(`${origin}/api/terminals/${id}`,{headers});
    expect(response.ok()).toBe(true);
  }
  const ids=[];
  const output=async id => Buffer.from((await (await page.request.get(`${origin}/api/terminals/${id}/output`,{headers})).json()).data,"base64").toString();
  try {
    await page.goto(url);
    await page.locator("#quick-terminal-empty").click();
    ids.push((await page.locator(".terminal-tile").getAttribute("id")).slice("terminal-".length));
    const input=()=>page.getByRole("textbox",{name:"Terminal input"}).filter({visible:true});
    // Escape the marker so matching output proves execution, not merely input echo.
    await input().pressSequentially("printf '\\163hared-project-ok\\n' > verb-shared-acceptance.txt; printf '\\146irst-session-ok:%s\\n' \"$$\"");
    await input().press("Enter");
    await expect.poll(()=>output(ids[0])).toMatch(/first-session-ok:\d+/);
    const firstPid=(await output(ids[0])).match(/first-session-ok:(\d+)/)[1];
    await page.locator("[data-terminal-new]").click();
    const tabs=page.getByRole("tab",{name:/Terminal/});
    await expect(tabs).toHaveCount(2);
    ids.push(await tabs.last().getAttribute("data-tab-terminal"));
    await input().pressSequentially("cat verb-shared-acceptance.txt; printf '\\163econd-session-ok:%s\\n' \"$$\"");
    await input().press("Enter");
    await expect.poll(()=>output(ids[1])).toContain("shared-project-ok");
    await expect.poll(()=>output(ids[1])).toMatch(/second-session-ok:\d+/);
    const secondPid=(await output(ids[1])).match(/second-session-ok:(\d+)/)[1];
    expect(secondPid).not.toBe(firstPid);
    if (process.env.VERB_ACCEPT_INSTALLED_CLIS === "1") {
      // Local opt-in uses native installed CLIs with the throwaway HOME; no sign-in or model request.
      await page.locator(`[data-tab-terminal="${ids[0]}"]`).click();
      await input().pressSequentially("claude --version; printf '\\143laude-cli-exit:%s\\n' \"$?\"");await input().press("Enter");
      await expect.poll(()=>output(ids[0]),{timeout:15000}).toContain("claude-cli-exit:0");
      await page.locator(`[data-tab-terminal="${ids[1]}"]`).click();
      await input().pressSequentially("codex --version; printf '\\143odex-cli-exit:%s\\n' \"$?\"");await input().press("Enter");
      await expect.poll(()=>output(ids[1]),{timeout:15000}).toContain("codex-cli-exit:0");
    }
    await page.reload();
    await expect(tabs).toHaveCount(2);
    await page.locator(`[data-tab-terminal="${ids[0]}"]`).click();
    await expect(page.locator(`#terminal-${ids[0]} .terminal-state`)).toContainText("Connected");
    await input().pressSequentially("printf '\\162eload-first:%s\\n' \"$$\"");await input().press("Enter");
    await expect.poll(()=>output(ids[0])).toContain(`reload-first:${firstPid}`);
    await page.locator(`[data-tab-terminal="${ids[1]}"]`).click();
    await expect(page.locator(`#terminal-${ids[1]} .terminal-state`)).toContainText("Connected");
    await input().pressSequentially("printf '\\162eload-second:%s\\n' \"$$\"; exit");await input().press("Enter");
    await expect.poll(()=>output(ids[1])).toContain(`reload-second:${secondPid}`);
    await expect(page.getByRole("button",{name:"Close terminal output",exact:true})).toBeVisible();
    await expect.poll(async()=> {
      const snapshot=await (await page.request.get(`${origin}/api/state`,{headers})).json();
      return snapshot.sessions.find(session=>session.id===ids[1])?.terminal.running;
    }).toBe(false); // The complete workspace snapshot has a bounded cache.
    const state=await (await page.request.get(`${origin}/api/state`,{headers})).json();
    const first=state.sessions.find(session=>session.id===ids[0]);
    const second=state.sessions.find(session=>session.id===ids[1]);
    expect(first.workspace).toBe(second.workspace);
    expect(first.terminal.running).toBe(true);expect(second.terminal.running).toBe(false);expect(second.terminal.replayAvailable).toBe(true);
  } finally {
    for (const id of ids) await page.request.delete(`${origin}/api/terminals/${id}`,{headers});
    const {unlinkSync}=await import("node:fs");
    try {unlinkSync(join(repo,"verb-shared-acceptance.txt"));} catch {}
  }
});
