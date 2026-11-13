import { test, expect } from "@playwright/test";
import { execFileSync, spawn } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const token = "bench-shell-fixture-token-000000000000000";
let fixture;
let child;
let origin;

test.beforeAll(async () => {
  fixture = mkdtempSync(join(tmpdir(), "verb-bench-shell-"));
  const project = join(fixture, "Bench workspace");
  mkdirSync(join(project, "work"), { recursive: true });
  cpSync("../../work/001-bench-consolidation", join(project, "work/001-bench-consolidation"), {
    recursive: true,
    filter: path => !path.includes("/screens"),
  });
  cpSync("../../specs", join(project, "specs"), { recursive: true });
  execFileSync("git", ["init", "-q", project]);
  writeFileSync(join(project, "package.json"), JSON.stringify({ scripts: { test: "node -e \"process.exit(require('node:fs').existsSync('specs/sessions/fixture-check-passes.md') ? 0 : 1)\"" } }));
  writeFileSync(join(project, "README.md"), "# Bench Preview\n\nA document in the project.");
  writeFileSync(join(project, "preview.html"), "<!doctype html><h1>Bench preview app</h1>");
  writeFileSync(join(project, "preview.png"), Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/n3sAAAAASUVORK5CYII=", "base64"));
  for (const id of ["090", "091", "092", "093", "094", "095", "096", "097"]) {
    writeFileSync(join(project, `specs/${id}-review.md`), `---\nid: ${id}\ntitle: Review sample ${id}\nstage: review\n---\n## Acceptance criteria\n- [x] A claim without proof\n- [ ] The check must pass\n`);
  }
  execFileSync("git", ["-C", project, "add", "."]);
  execFileSync("git", ["-C", project, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
    "commit", "-qm", "Start"]);
  const bin = join(fixture, "bin");
  mkdirSync(bin);
  writeFileSync(join(bin, "agy"), "#!/bin/sh\nprintf '%s\\n' '{}'\n", { mode: 0o755 });
  writeFileSync(join(bin, "claude"), "#!/bin/sh\nprintf '\\033[?2004hAgent ready\\n'\nexec cat\n", { mode: 0o755 });
  writeFileSync(join(project, "specs/099-legacy.md"),
    "---\nid: 099\ntitle: Legacy sample\nstage: plan\n---\n## Acceptance criteria\n- [ ] A legacy criterion\n");
  writeFileSync(join(bin, "codex"), String.raw`#!/bin/sh
for prompt do :; done
draft=$(printf '%s\n' "$prompt" | sed -n 's/^Draft path: //p' | head -n 1)
case "$draft" in
  *.brd.draft.md|*/brd.draft.md)
    item=$(basename "$(dirname "$draft")" | cut -d- -f2- | tr '-' ' ')
    printf '# %s\n\nSize: small\n\nProblem: Show work planning\nG1: Trace the outcome\nDone when: The owner publishes\n' "$item" > "$draft"
    printf 'Review this scope change\n' > plan-fixture-change.txt
    ;;
  */prd.draft.md)
    { printf '# Fixture PRD\n'; printf '%s\n' "$prompt" | grep '^- R1 · '; } > "$draft"
    ;;
  *)
    printf '%s\n' "$prompt"
    exec cat
    ;;
esac
`, { mode: 0o755 });
  child = spawn(resolve(process.env.VERB_BIN ?? "../target/debug/verb"), ["web", "--port", "0"], {
    cwd: project,
    env: { PATH: `${bin}:${process.env.PATH}`, HOME: fixture, VERB_STATE_DIR: join(fixture, "state"), VERB_TOKEN: token },
    stdio: ["ignore", "pipe", "inherit"],
  });
  origin = await new Promise((resolve, reject) => {
    let stdout = "";
    const timeout = setTimeout(() => reject(new Error("Bench shell host did not start")), 20_000);
    child.stdout.on("data", chunk => {
      stdout += chunk;
      const url = /^Verb web UI: (http:\/\/127\.0\.0\.1:\d+)/m.exec(stdout)?.[1];
      if (url) { clearTimeout(timeout); resolve(url); }
    });
    child.once("error", error => { clearTimeout(timeout); reject(error); });
    child.once("exit", code => { clearTimeout(timeout); reject(new Error(`Bench shell host exited with ${code}`)); });
  });
});

test.afterAll(async () => {
  if (child?.pid && child.exitCode === null && child.signalCode === null) {
    await new Promise(resolve => { child.once("exit", resolve); child.kill(); });
  }
  if (fixture) rmSync(fixture, { recursive: true, force: true });
});

test.afterEach(async ({ request }) => {
  if (!origin) return;
  const headers = { "X-Verb-Token": token };
  const workspace = await request.get(`${origin}/api/workspace`, { headers });
  for (const session of (await workspace.json()).sessions ?? []) {
    await request.delete(`${origin}/api/terminals/${session.id}`, { headers });
  }
});

for (const width of [1440, 390]) {
  for (const theme of ["light", "dark"]) {
    test(`Bench Shell at ${width} in ${theme}`, async ({ page }) => {
      test.setTimeout(90_000);
      page.setDefaultTimeout(8_000);
      const mobile = width === 390;
      await page.setViewportSize({ width, height: mobile ? 844 : 900 });
      await page.emulateMedia({ colorScheme: theme });
      await page.goto(`${origin}/#${token}`);
      const app = page.locator("#app");
      const nav = page.getByRole("navigation", { name: "Bench navigation" });
      const navigation = page.locator("#bench-toggle-navigation");
      const details = page.locator("#bench-toggle-right");
      const panel = page.getByRole("complementary", { name: "Spec details" });
      const palette = page.getByRole("button", { name: "Search or run" });
      const run = async title => {
        await palette.click();
        await page.locator("#palette-input").fill(title);
        await page.locator("#palette-input").press("Enter");
      };
      const openSettings = async () => {
        if (!(await app.evaluate(node => node.classList.contains("bench-layout")))) {
          await page.locator("#bench-settings").click();
          return;
        }
        await run("Open Settings");
      };
      const shot = async name => {
        if (process.env.VERB_SHELL_SCREEN_DIR) {
          await page.screenshot({ animations: "disabled", path: join(process.env.VERB_SHELL_SCREEN_DIR, `${name}-${width}-${theme}.png`) });
        }
      };
      await expect(app).toHaveClass(/bench-layout/);
      await expect(page.locator("#view-sessions")).toBeVisible();
      if (mobile) await expect(nav).toBeHidden();
      else await expect(panel).toBeVisible();
      await expect(page.locator(".topbar .breadcrumb")).toBeHidden();
      await expect(page.locator("#toggle-navigation")).toBeHidden();
      await expect(page.locator("#refresh-button")).toBeHidden();
      await expect(page.locator("#theme-toggle")).toBeHidden();
      await expect(page.locator("#observer-pill")).toBeHidden();
      await expect(page.locator('[data-action="ask"]')).toBeHidden();
      await expect(page.locator("#bench-settings")).toBeHidden();
      const top = await page.locator(".topbar").boundingBox();
      expect(top.height).toBeLessThanOrEqual(56);
      await shot("shell");

      if (mobile) await navigation.click();
      const group = nav.locator('details[data-bench-group="001-bench-consolidation"]');
      await group.locator("summary").click({ position: { x: 6, y: 8 } });
      await expect(group.locator("summary")).toHaveText(/Bench consolidation\s+14/);
      await expect(nav.locator('[data-bench-group="unlinked"]')).not.toHaveAttribute("open", "");
      await shot("navigation");
      await group.getByRole("button", { name: /006 · Terminal-first Shell/ }).click();
      await expect(panel.getByRole("heading", { name: /Terminal-first Shell/ })).toBeVisible();
      await expect(panel.locator(".bench-documents > li > div")).toHaveText(["Why (BRD)Published", "What (PRD)Published"]);
      await shot("plan");
      for (const [label, file] of [["Why (BRD)", "brd.md"], ["What (PRD)", "prd.md"]]) {
        await panel.getByRole("button", { name: label }).click();
        await expect(panel.locator("#studio-document-path")).toHaveText(`work/001-bench-consolidation/${file}`);
        await panel.getByRole("tab", { name: "Plan", exact: true }).click();
      }
      await panel.getByRole("tab", { name: "Plan" }).press("ArrowRight");
      await expect(panel.getByRole("tab", { name: "Review", exact: true })).toHaveAttribute("aria-selected", "true");
      await shot("review");
      await panel.getByRole("tab", { name: "Preview" }).click();
      await expect(panel.getByLabel("Find a file", { exact: true })).toBeVisible();
      await shot("preview");
      await panel.getByRole("tab", { name: "Plan" }).click();

      if (mobile) {
        await expect(page.locator("main")).toHaveAttribute("inert", "");
        await panel.getByRole("button", { name: "Close details" }).click();
        await expect(panel).toBeHidden();
        await details.click();
        await expect(panel).toBeVisible();
        await page.keyboard.press("Escape");
        await navigation.click();
        await nav.getByRole("button", { name: "Close navigation" }).click();
        await expect(nav).toBeHidden();
      } else {
        const separator = page.getByRole("separator", { name: "Resize details panel" });
        await separator.press("ArrowLeft");
        const remembered = await separator.getAttribute("aria-valuenow");
        await details.click();
        await expect(panel).toBeHidden();
        await shot("details-collapsed");
        await navigation.click();
        await expect(nav).toBeHidden();
        await shot("panels-collapsed");
        await navigation.click();
        await expect(nav).toBeVisible();
        await details.click();
        await expect(separator).toHaveAttribute("aria-valuenow", remembered);
      }

      if (mobile) await details.click();
      await panel.getByRole("button", { name: "Start terminal" }).click();
      await expect(page.locator("#session-form [name=specId]")).toHaveValue("006");
      await page.locator('#session-form button[type="submit"]').click();
      const tile = page.locator(".terminal-tile").first();
      await expect(tile).toBeVisible();
      await expect(tile.locator(".terminal-titlebar")).toBeHidden();
      await expect(tile.locator(".terminal-bottom")).toBeHidden();
      await expect(tile.locator(".bench-connection")).toBeHidden();
      await expect(page.locator(".topbar").getByRole("tab", { name: /Terminal/ })).toBeVisible();
      const terminalMenu = page.getByLabel("Terminal actions");
      await terminalMenu.click();
      await page.getByRole("button", { name: "Search terminal", exact: true }).click();
      await expect(page.getByRole("searchbox", { name: "Find text" })).toBeVisible();
      await terminalMenu.click();
      await expect(page.getByRole("button", { name: "Search terminal", exact: true })).toHaveAttribute("aria-pressed", "true");
      await terminalMenu.click();
      await tile.locator(".search-close").click();
      await terminalMenu.click();
      await expect(page.getByRole("button", { name: "Search terminal", exact: true })).toHaveAttribute("aria-pressed", "false");
      await terminalMenu.click();
      const id = await tile.getAttribute("id");
      const domTile = await tile.elementHandle();
      const input = page.getByRole("textbox", { name: "Terminal input" });
      const command = async marker => {
        await input.pressSequentially(`printf '%s%s\\n' '${marker.slice(0, 5)}' '${marker.slice(5)}'`);
        await input.press("Enter");
        await expect.poll(async () => {
          const response = await page.request.get(`${origin}/api/terminals/${id.slice("terminal-".length)}/output`, { headers: { "X-Verb-Token": token } });
          return Buffer.from((await response.json()).data, "base64").toString();
        }).toContain(marker);
      };
      await command("before-layout-switch");
      if (mobile) await navigation.click();
      await group.getByRole("button", { name: /006 · Terminal-first Shell/ }).click();
      await expect(page.locator("#view-sessions")).toBeVisible();
      await expect(tile).toHaveAttribute("id", id);
      if (mobile) await expect(panel).toBeHidden();
      await shot("terminal");
      if (mobile) await navigation.click();
      await group.getByRole("button", { name: /005 · Open another/ }).click();
      await expect(panel.getByRole("heading", { name: /Open another authorized project/ })).toBeVisible();
      if (mobile) await page.keyboard.press("Escape");
      if (mobile) await navigation.click();
      await nav.getByRole("button", { name: "Board", exact: true }).click();
      await expect(page.locator("#view-bench-board")).toBeVisible();
      await expect(page.locator("[data-board-item]").first()).toBeVisible();
      await expect(panel).toBeHidden();
      await shot("board");
      if (mobile) await navigation.click();
      await nav.locator('[data-view="needs-you"]').click();
      await expect(page.locator("#view-needs-you")).toBeVisible();
      await shot("needs-you");
      await openSettings();
      await shot("settings");
      await expect(page.getByRole("checkbox", { name: "Use previous navigation" })).toHaveCount(0);
      await expect(app).toHaveClass(/bench-layout/);
      await run("Open Studio");
      if (mobile) await page.locator("#studio-focus-work").click();
      const draft = page.locator("#studio-intent");
      const newChange = page.locator("#studio-new-change-toggle");
      if (await newChange.isVisible()) await newChange.click();
      await draft.fill("Keep this draft through navigation");
      await openSettings();
      await expect(app).toHaveClass(/bench-layout/);
      await run("Open Studio");
      await expect(draft).toHaveValue("Keep this draft through navigation");
      await openSettings();
      await run("Open Sessions");
      await expect(tile).toHaveAttribute("id", id);
      expect(await tile.evaluate((node, previous) => node === previous, domTile)).toBeTruthy();
      await command("after-navigation");
      await terminalMenu.click();
      await page.getByRole("button", { name: "End session", exact: true }).click();
      await expect(page.locator("#end-session-dialog")).toBeVisible();
      await page.locator("#end-session-confirm").click();
      await expect(tile).toHaveCount(0);
      const note = page.locator("#bench-note-dialog");
      await expect(note).toBeVisible();
      await note.getByRole("button", { name: "Skip" }).click();
    });
  }
}

test("Plan distinguishes a document with warnings from a missing document", async ({ page, request }) => {
  let data;
  await expect.poll(async () => {
    const response = await request.get(`${origin}/api/work-items`, { headers: { "X-Verb-Token": token } });
    if (response.ok()) data = await response.json();
    return response.status();
  }).toBe(200);
  await page.route("**/api/work-items", async route => {
    const item = data.items.find(item => item.id === "001-bench-consolidation");
    item.warnings = ["BRD is missing content in Scope"];
    item.prd = null;
    await route.fulfill({ json: data });
  });
  await page.goto(`${origin}/#${token}`);
  await page.locator('[data-bench-group="001-bench-consolidation"] summary').click({ position: { x: 6, y: 8 } });
  await page.locator('#bench-navigation [data-bench-spec="006"]').click();
  const docs = page.locator(".bench-documents");
  await expect(docs.locator(":scope > li > div")).toHaveText(["Why (BRD)Warnings", "What (PRD)Missing"]);
  await expect(docs.locator("[data-bench-document]")).toHaveCount(1);
});

test("the terminal shows connection text only during an interruption", async ({ page }) => {
  let socket;
  await page.routeWebSocket("**/api/terminals/ws?*", ws => {
    if (!socket) socket = ws;
    else return ws.close();
  });
  let releasePoll;
  const pausePoll = new Promise(resolve => { releasePoll = resolve; });
  await page.route("**/api/terminals/*/output?*", async route => {
    await pausePoll;
    await route.abort();
  });
  await page.goto(`${origin}/#${token}`);
  await page.locator("#quick-terminal-empty").click();
  await expect(page.locator(".terminal-tile")).toBeVisible();
  const connection = page.locator(".terminal-tile .bench-connection");
  try {
    await expect(connection).toBeHidden();
    await expect.poll(() => Boolean(socket)).toBeTruthy();
    await socket.close();
    await expect(connection.getByText("Reconnecting…")).toBeVisible();
    releasePoll();
    await expect(connection.getByText("Disconnected")).toBeVisible();
  } finally {
    releasePoll();
  }
});

for (const width of [1440, 390]) {
  for (const theme of ["light", "dark"]) {
    test(`spec terminals, context and notes at ${width} in ${theme}`, async ({ page }) => {
      test.setTimeout(120_000);
      page.setDefaultTimeout(12_000);
      const mobile = width === 390;
      await page.setViewportSize({ width, height: mobile ? 844 : 900 });
      await page.emulateMedia({ colorScheme: theme });
      const shot = name => process.env.VERB_SLICE2_SCREEN_DIR
        ? page.screenshot({ animations: "disabled", path: join(process.env.VERB_SLICE2_SCREEN_DIR, `${name}-${width}-${theme}.png`) })
        : Promise.resolve();
      const sizes = new Map();
      page.on("request", request => {
        if (request.method() !== "POST") return;
        const match = /\/api\/terminals\/([^/]+)\/resize$/.exec(request.url());
        if (match) sizes.set(match[1], request.postDataJSON());
      });
      await page.goto(`${origin}/#${token}`);
      const nav = page.locator("#bench-navigation");
      if (mobile) await page.locator("#bench-toggle-navigation").click();
      await nav.locator('[data-bench-group="001-bench-consolidation"] summary').click({ position: { x: 6, y: 8 } });
      await nav.locator('[data-bench-spec="001"]').click();
      if (mobile) await page.keyboard.press("Escape");
      const selected = page.getByRole("button", { name: /Start a terminal for 001/ });
      await expect(selected).toBeVisible();
      await expect(page.getByRole("heading", { name: "Open a terminal in this project" })).toBeHidden();
      await expect(page.locator(".terminal-empty").getByRole("button")).toHaveCount(1);
      await shot("spec-empty");
      const form = page.locator("#session-form");
      await selected.click();
      await expect(form.locator("[name=specId]")).toHaveValue("001");
      await shot("launch");
      const submit = async role => {
        await form.locator("label").filter({ has: page.locator("[value=claude]") }).click();
        await form.locator("[name=role]").selectOption(role);
        await form.locator('button[type="submit"]').click();
        await expect(form).toBeHidden();
      };
      await submit("implement");
      const tabs = page.getByRole("tablist", { name: "Terminals" });
      const first = tabs.getByRole("tab", { name: /Spec 001.*Implement/ });
      await expect(first).toBeVisible();
      for (const [id, role] of [["002", "review"], ["003", "investigate"], ["004", "implement"]]) {
        await page.getByRole("button", { name: "New terminal", exact: true }).click();
        await form.locator("[name=specId]").selectOption(id);
        await submit(role);
      }
      await expect(tabs.getByRole("tab")).toHaveCount(4);
      await first.click();
      if (mobile) {
        const bounds = await tabs.boundingBox();
        const activeTab = await first.boundingBox();
        expect(activeTab.x + activeTab.width).toBeLessThanOrEqual(bounds.x + bounds.width + 1);
      }
      const mount = page.locator(".terminal-tile.focused .terminal-mount");
      await expect(mount).toBeVisible();
      const box = await mount.boundingBox();
      expect(box.height).toBeGreaterThan(mobile ? 560 : 650);
      await expect(page.locator("#terminal-grid .term-resize")).toBeHidden();
      await expect(page.locator("#bench-terminal-context-summary")).toContainText("Spec 001 · Delivered");
      await shot("terminal");

      const menu = page.getByLabel("Terminal actions");
      await menu.click();
      await page.getByRole("button", { name: "Change assignment", exact: true }).click();
      const binding = page.locator("#bench-binding-dialog");
      await expect(binding.locator("[name=specId]")).toHaveValue("001");
      await shot("assignment");
      await binding.getByRole("button", { name: "Cancel" }).click();
      if (!mobile) {
        const tileId = await page.locator(".terminal-tile.focused").getAttribute("id");
        const id = tileId.slice("terminal-".length);
        await expect.poll(() => sizes.get(id)?.cols ?? 0).toBeGreaterThan(0);
        const originalColumns = sizes.get(id).cols;
        await page.locator("#bench-toggle-right").click();
        await expect(page.locator("#bench-right-panel")).toBeHidden();
        await expect.poll(async () => (await mount.boundingBox()).width).toBeGreaterThan(box.width);
        await expect.poll(() => sizes.get(id)?.cols ?? 0).toBeGreaterThan(originalColumns);
        await page.locator("#bench-toggle-right").click();
        await expect.poll(() => sizes.get(id)?.cols ?? 0).toBe(originalColumns);
        await menu.click();
        await page.getByRole("button", { name: "Show side by side" }).click();
        await expect(page.locator("#terminal-grid .terminal-tile:visible")).toHaveCount(2);
        await shot("split");
        await menu.click();
        await page.getByRole("button", { name: "Show one terminal" }).click();
      }
      await page.getByRole("button", { name: "Resend context" }).click();
      const review = page.locator("#bench-context-dialog");
      await expect(review.locator("pre")).toContainText("Project context");
      await shot("resend");
      await review.getByRole("button", { name: "Paste context" }).click();
      await expect(review).toBeHidden();
      await page.getByRole("button", { name: "View delivered context" }).click();
      await expect(review.locator("#bench-context-detail")).toContainText("resend");
      await review.getByRole("button", { name: "Close" }).click();

      await page.getByRole("button", { name: "Hand off", exact: true }).click();
      await expect(page.locator("#handoff-revision")).not.toBeEmpty();
      await shot("handoff");
      await page.locator('#handoff-dialog [data-close="handoff-dialog"]').last().click();
      await menu.click();
      await expect(page.locator("#bench-terminal-id")).toHaveText(/^Session [0-9a-f]+$/);
      await page.getByRole("button", { name: "End session", exact: true }).click();
      await page.locator("#end-session-confirm").click();
      const note = page.locator("#bench-note-dialog");
      await expect(note).toBeVisible();
      const marker = `Reviewed note ${width} ${theme}`;
      await note.locator("[name=outcome]").fill(marker);
      await shot("note");
      await note.getByRole("button", { name: "Save note" }).click();
      await expect(note).toBeHidden();

      await page.getByRole("button", { name: "New terminal", exact: true }).click();
      await form.locator("[name=specId]").selectOption("001");
      await submit("review");
      await expect(tabs.getByRole("tab", { name: /Spec 001.*Review/ })).toBeVisible();
      await page.getByRole("button", { name: "View delivered context" }).click();
      await expect(review.locator("pre")).toContainText(marker);
      await expect(review.locator("pre")).toContainText("reported, unverified");
      await shot("delivered");
    });
  }
}

for (const width of [1440, 390]) {
  for (const theme of ["light", "dark"]) {
    test(`Review leads with proof and refuses stale handoff at ${width} in ${theme}`, async ({ page, request }) => {
      test.setTimeout(120_000);
      const mobile = width === 390;
      const id = String(90 + (mobile ? 2 : 0) + (theme === "dark" ? 1 : 0)).padStart(3, "0");
      const headers = { "X-Verb-Token": token };
      const project = join(fixture, "Bench workspace");
      const shot = name => {
        const dir = process.env.VERB_SLICE5_SCREEN_DIR || process.env.VERB_SLICE4_SCREEN_DIR;
        return dir ? page.screenshot({ animations: "disabled", path: join(dir, `${name}-${width}-${theme}.png`) }) : Promise.resolve();
      };
      await page.setViewportSize({ width, height: mobile ? 844 : 900 });
      await page.emulateMedia({ colorScheme: theme });
      const marked = await request.post(`${origin}/api/good/mark`, { headers });
      expect(marked.ok()).toBe(true);
      await page.goto(`${origin}/#${token}`);
      const nav = page.locator("#bench-navigation");
      if (mobile) await page.locator("#bench-toggle-navigation").click();
      await nav.locator('[data-bench-group="unlinked"] summary').click();
      await nav.locator(`[data-bench-spec="${id}"]`).click();
      if (mobile) await page.keyboard.press("Escape");
      await expect(page.locator(".terminal-empty").getByRole("button")).toHaveCount(1);
      await expect(page.locator(".terminal-empty").getByRole("button")).toHaveText(new RegExp(`Start a terminal for ${id}`));
      await shot("spec-empty");
      if (mobile) await page.locator("#bench-toggle-right").click();
      await page.getByRole("tab", { name: "Review", exact: true }).click();
      const panel = page.locator("#bench-review");
      await expect(panel.locator(".bench-review-verdict")).toHaveCount(0);
      await expect(panel.getByRole("button", { name: "Hand off for review" })).toBeVisible();
      await expect(panel.getByText("Unproven", { exact: true }).first()).toBeVisible();
      await expect(panel.getByText("Marked complete in the spec. Proof still needed.")).toBeVisible();
      await panel.getByRole("button", { name: "Run a check" }).click();
      await page.locator("#sdlc-criterion").selectOption("1");
      await page.locator("#sdlc-check").selectOption("node");
      await shot("run-check");
      await page.locator("#sdlc-submit").click();
      await expect(panel.locator(".bench-failed-checks")).toContainText("Failed check", { timeout: 30_000 });
      await expect(panel.getByText("All checks passed.")).toBeHidden();
      await expect(panel.locator(".bench-review-verdict")).toHaveText("Not ready: 1 failed check, 1 unproven");
      await expect(panel.locator(".bench-proof-list > li").first()).toContainText("Criterion 2");
      await expect(panel.locator(".bench-proof-list > li").first()).toContainText("The check must pass");
      await expect(panel.locator(".bench-proof-list > li").nth(1)).toContainText("Unproven");
      await shot("review");

      await panel.locator('.bench-proof-list [data-criterion="0"][data-bench-review-action="report"]').click();
      await expect(page.locator("#sdlc-criterion")).toHaveValue("0");
      await page.locator("#sdlc-target").fill("Local review fixture");
      await page.locator("#sdlc-text").fill("Verified the first criterion manually.");
      await shot("record-proof");
      await page.locator("#sdlc-submit").click();
      await expect(panel.getByText("Pass awaiting acceptance")).toBeVisible();
      await panel.getByRole("button", { name: "Accept evidence" }).click();
      await page.locator("#sdlc-text").fill("The manual check directly addresses the criterion.");
      await page.locator("#sdlc-submit").click();
      await expect(panel.getByText("Accepted proof")).toBeVisible();

      const launched = await request.post(`${origin}/api/specs/${id}/agent`, { headers, data: { agent: "claude", role: "implement" } });
      expect(launched.ok()).toBe(true);
      const first = (await launched.json()).sessionId;
      await panel.getByRole("button", { name: "Hand fix to an agent" }).click();
      const form = page.locator("#handoff-dialog-form");
      const oldRevision = await form.locator("#handoff-revision").textContent();
      expect(oldRevision).toMatch(/^[a-f0-9]{10}$/);
      await expect(form.getByText("You reviewed the spec as it is now", { exact: false })).toBeVisible();
      await expect(form.getByLabel("Role")).toHaveValue("implement");
      await expect(form.locator("#handoff-from")).toHaveText(new RegExp(first.slice(0, 8)));
      await expect(form.locator("#handoff-running-note")).toBeVisible();
      if (!mobile) {
        expect(await form.evaluate(node => node.scrollHeight - node.clientHeight)).toBeLessThanOrEqual(1);
        const note = await form.locator("#handoff-running-note").boundingBox();
        const buttons = await form.locator("#handoff-submit").boundingBox();
        expect(note.y + note.height).toBeLessThanOrEqual(buttons.y);
      }
      await form.locator("#handoff-to").selectOption("codex");
      await form.getByLabel("Role").selectOption("review");
      await form.getByLabel("Approved scope").fill("Inspect package.json and report why the check fails.");
      await form.getByLabel("Approved checks").fill("npm test");
      await shot("handoff");
      const specPath = join(project, `specs/${id}-review.md`);
      writeFileSync(specPath, readFileSync(specPath, "utf8") + "\n## Owner update\nReview the newly observed failure.\n");
      let posts = 0;
      let release;
      const gate = new Promise(resolve => { release = resolve; });
      await page.route(`**/api/specs/${id}/handoff`, async route => {
        posts++;
        if (posts === 1) await gate;
        await route.continue();
      });
      const submit = form.getByRole("button", { name: "Hand off", exact: true });
      await submit.click();
      await expect(submit).toBeDisabled();
      await form.evaluate(element => element.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })));
      release();
      await expect(form.getByRole("alert")).toContainText(/revision|changed/i);
      expect(posts).toBe(1);
      await expect(submit).toBeDisabled();
      await shot("stale-handoff");
      await form.getByRole("button", { name: "Refresh spec revision" }).click();
      await expect(form.locator("#handoff-revision")).not.toHaveText(oldRevision);
      await expect(form.getByText("Review the newly observed failure.", { exact: false })).toBeVisible();
      await expect(form.getByLabel("Approved scope")).toHaveValue(/Inspect package.json/);
      await submit.click();
      await expect(form).toBeHidden();
      expect(posts).toBe(2);
      await expect.poll(async () => {
        const response = await request.get(`${origin}/api/workspace`, { headers });
        return (await response.json()).sessions?.filter(session => session.specId === id).map(session => [session.id, session.role]);
      }).toEqual(expect.arrayContaining([[first, "implement"], [expect.any(String), "review"]]));

      if (mobile && !(await panel.isVisible())) {
        await page.locator("#bench-toggle-right").click();
        await page.getByRole("tab", { name: "Review", exact: true }).click();
      }
      const codePath = join(project, "package.json");
      const original = readFileSync(codePath, "utf8");
      try {
        writeFileSync(codePath, original + "\n");
        await panel.getByRole("button", { name: "Changes", exact: true }).click();
        await expect(page.locator("#bench-review-diff pre")).toContainText("package.json");
        await expect(page.locator("#bench-review-diff-range")).toHaveValue("good");
        await shot("diff");
        await page.locator("#bench-review-diff-range").selectOption("implementation");
        await expect(page.locator("#bench-review-diff-scope")).toContainText(/Latest commit|Committed implementation/);
        await shot("implementation-diff");
        await page.locator("#bench-review-diff").getByRole("button", { name: "Close" }).click();
        await panel.getByLabel("Git actions").click();
        await shot("git-menu");
        await panel.getByRole("button", { name: "Review commit" }).click();
        await expect(page.locator("#commit-dialog")).toBeVisible();
        await shot("commit");
        await page.keyboard.press("Escape");
        await panel.getByLabel("Git actions").click();
        await panel.getByRole("button", { name: "Git transfer" }).click();
        await expect(page.locator("#git-transfer-dialog")).toBeVisible();
        await shot("git-transfer");
      } finally {
        writeFileSync(codePath, original);
      }
    });
  }
}

for (const width of [1440, 390]) {
  for (const theme of ["light", "dark"]) {
    test(`Board archives and routes observed attention at ${width} in ${theme}`, async ({ page, request }) => {
      test.setTimeout(100_000);
      const mobile = width === 390;
      const id = String(94 + (mobile ? 2 : 0) + (theme === "dark" ? 1 : 0)).padStart(3, "0");
      const headers = { "X-Verb-Token": token };
      const project = join(fixture, "Bench workspace");
      const shot = name => (process.env.VERB_SLICE7A_SCREEN_DIR || process.env.VERB_SLICE6_SCREEN_DIR)
        ? page.screenshot({ animations: "disabled", path: join(process.env.VERB_SLICE7A_SCREEN_DIR || process.env.VERB_SLICE6_SCREEN_DIR, `${name}-${width}-${theme}.png`) })
        : Promise.resolve();
      await page.setViewportSize({ width, height: mobile ? 844 : 900 });
      await page.emulateMedia({ colorScheme: theme });
      const launched = await request.post(`${origin}/api/specs/${id}/agent`, { headers, data: { agent: "shell" } });
      expect(launched.ok()).toBe(true);
      const session = (await launched.json()).sessionId;
      let asking = [];
      await page.route("**/api/observer", route =>
        route.fulfill({ json: { enabled: false, muted: [], kinds: [], signals: [], asking } }));
      await page.goto(`${origin}/#${token}`);
      const nav = page.locator("#bench-navigation");
      const go = async name => {
        if (mobile) {
          if (await page.locator("#app").getAttribute("data-bench-overlay") === "details") await page.keyboard.press("Escape");
          if (!(await nav.isVisible())) await page.locator("#bench-toggle-navigation").click();
        }
        await nav.locator(name === "Board" ? '[data-view="bench-board"]' : '[data-view="needs-you"]').click();
      };
      await go("Board");
      const row = page.locator('[data-board-item="001-bench-consolidation"]');
      const unlinked = page.locator("[data-board-unlinked]");
      await expect(unlinked).not.toHaveAttribute("open", "");
      await expect(row.getByText("In progress", { exact: false })).toBeVisible();
      await expect(row).toContainText("Standard");
      await expect(row.locator("time")).toHaveAttribute("datetime", /^\d{4}-\d{2}-\d{2}T/);
      await expect(row.locator(".bench-stage-counts dt")).toHaveText(["Spec", "Plan", "Build", "Verify", "Review", "Ship"]);
      await shot("board");
      const brd = join(project, "work", "001-bench-consolidation", "brd.md");
      const before = readFileSync(brd, "utf8");
      await row.getByRole("button", { name: "Archive", exact: true }).click();
      await expect(row).toHaveCount(0);
      await page.getByLabel("Show archived").check();
      await expect(row.getByText(/\bArchived\b/)).toBeVisible();
      await shot("board-archived");
      await row.getByRole("button", { name: "Unarchive", exact: true }).click();
      await expect(row.getByRole("button", { name: "Archive", exact: true })).toBeVisible();
      expect(readFileSync(brd, "utf8")).toBe(before);

      await expect(page.locator("#view-bench-board select")).toHaveCount(0);
      const selectFixtureSpec = async () => {
        if (mobile && !(await nav.isVisible())) {
          if (await page.locator("#app").getAttribute("data-bench-overlay") === "details") await page.keyboard.press("Escape");
          await page.locator("#bench-toggle-navigation").click();
        }
        const group = nav.locator('[data-bench-group="unlinked"]');
        if (!(await group.evaluate(node => node.open))) await group.locator("summary").click();
        await group.locator(`[data-bench-spec="${id}"]`).click();
        if (mobile && !(await page.locator("#bench-right-panel").isVisible())) await page.locator("#bench-toggle-right").click();
        await page.locator("#bench-tab-plan").click();
      };
      await selectFixtureSpec();
      const stage = page.getByLabel(`Stage for spec ${id}`);
      await stage.selectOption("ship");
      await expect(page.locator("#stage-warnings")).toBeVisible();
      await shot("stage-warning");
      await page.locator("#stage-dialog-submit").click();
      await expect(stage).toHaveValue("ship");

      const ledger = `${origin}/api/specs/${id}/sdlc`;
      const spec = `${origin}/api/specs/${id}`;
      const whenReady = async send => {
        let response;
        await expect.poll(async () => {
          response = await send();
          return (await response.json()).error !== "desktop host is busy; retry shortly";
        }).toBe(true);
        expect(response.ok(), await response.text()).toBe(true);
        return response.json();
      };
      const run = async () => {
        const current = await whenReady(() => request.get(ledger, { headers }));
        const { revision } = await whenReady(() => request.get(spec, { headers }));
        await whenReady(() => request.post(ledger, { headers, data: { action: "run", criterion: 0, check: "node",
          expectedRevision: revision, expectedVersion: current.version, expectedContext: current.context } }));
      };
      await run();
      await go(/Needs you/);
      const needs = page.locator("#bench-needs-list");
      const failed = needs.locator("section").filter({ hasText: `Failed check · ${id}` });
      const gate = needs.locator("section").filter({ hasText: `Recorded gate warning · ${id}` });
      const waiting = needs.locator("section").filter({ hasText: "Waiting for you" });
      await expect(failed).toBeVisible();
      await expect(gate.getByText(/^Moved to Ship with \d+ warnings$/)).toBeVisible();
      await expect(gate.locator("li").first()).toBeVisible();
      await expect(gate).not.toContainText("SDLC readiness");
      await expect(failed).toContainText("Clears when the check passes.");
      asking = [session];
      await expect(waiting).toBeVisible({ timeout: 10_000 });
      await expect(waiting.getByRole("button", { name: "Mark handled" })).toHaveCount(0);
      await shot("needs-you");
      await go("Board");
      await expect(page.locator("#bench-board-attention-list")).toContainText(`Failed check · ${id}`);
      await shot("board-attention");
      await go(/Needs you/);
      await waiting.getByRole("button", { name: "Open session" }).click();
      await expect(page.locator("#view-sessions")).toBeVisible();
      await go(/Needs you/);
      asking = [];
      await expect(waiting).toHaveCount(0, { timeout: 10_000 });
      await failed.getByRole("button", { name: "Open Review" }).click();
      await expect(page.locator("#bench-tab-review")).toHaveAttribute("aria-selected", "true");

      mkdirSync(join(project, "specs", "sessions"), { recursive: true });
      const pass = join(project, "specs", "sessions", "fixture-check-passes.md");
      writeFileSync(pass, "");
      try { await run(); } finally { rmSync(pass, { force: true }); }
      await go(/Needs you/);
      await expect(failed).toHaveCount(0);
      const html = join(project, "preview.html");
      const saved = readFileSync(html, "utf8");
      try {
        writeFileSync(html, saved + "\n<!-- changed -->");
        await go("Board");
        await go(/Needs you/);
        const stale = needs.locator("section").filter({ hasText: `Stale proof · ${id}` });
        await expect(stale).toBeVisible();
        await shot("stale-proof");
      } finally { writeFileSync(html, saved); }

      await selectFixtureSpec();
      await stage.selectOption("spec");
      await expect(page.locator("#stage-warnings")).toBeHidden();
      await page.locator("#stage-dialog-submit").click();
      await expect(stage).toHaveValue("spec");
      await go(/Needs you/);
      await expect(gate).toHaveCount(0);
      await expect(needs.locator("section").filter({ hasText: `Stale proof · ${id}` })).toHaveCount(0);
    });
  }
}

for (const width of [1440, 390]) {
  for (const theme of ["light", "dark"]) {
    test(`Plan traces work and publishes drafts at ${width} in ${theme}`, async ({ page, request }) => {
      const mobile = width === 390;
      test.setTimeout(90_000);
      await page.setViewportSize({ width, height: mobile ? 844 : 900 });
      await page.emulateMedia({ colorScheme: theme });
      const shot = name => {
        const changed = ["plan", "published-small-item", "needs-you", "handled"].includes(name);
        const fixtureShot = ["work-item-draft", "published-small-item"].includes(name);
        const directory = (fixtureShot && process.env.VERB_SLICE6_SCREEN_DIR)
          || (changed && process.env.VERB_SLICE4_SCREEN_DIR) || process.env.VERB_SLICE3_SCREEN_DIR;
        return directory ? page.screenshot({ animations: "disabled", path: join(directory, `${name}-${width}-${theme}.png`) }) : Promise.resolve();
      };
      const auth = { "X-Verb-Token": token };
      await page.goto(`${origin}/#${token}`);
      const nav = page.locator("#bench-navigation");
      const panel = page.locator("#bench-plan-spec");
      const openNav = async () => {
        if (!mobile || await nav.isVisible()) return;
        if (await page.locator("#app").getAttribute("data-bench-overlay") === "details") {
          await page.keyboard.press("Escape");
        }
        await page.locator("#bench-toggle-navigation").click();
      };
      await openNav();
      const group = nav.locator('[data-bench-group="001-bench-consolidation"]');
      await group.locator("summary").click({ position: { x: 6, y: 8 } });
      await group.locator('[data-bench-spec="006"]').click();
      await expect(panel.getByText("How (Spec)")).toBeVisible();
      await expect(panel.getByText("R6", { exact: true }).first()).toBeVisible();
      await expect(panel.getByText("G2", { exact: true }).first()).toBeVisible();
      await shot("plan");
      await panel.getByRole("button", { name: "New spec task" }).click();
      const task = page.locator("#task-form");
      const taskTitle = `Trace planning ${width} ${theme}`;
      await task.getByLabel("Title").fill(taskTitle);
      await shot("new-spec-task");
      await task.getByRole("button", { name: "Create task" }).click();
      await expect(panel.getByText(taskTitle, { exact: false })).toBeVisible();

      await panel.getByRole("button", { name: "Start terminal" }).click();
      const launch = page.locator("#session-form");
      await expect(launch.getByLabel("Role")).toBeHidden();
      await expect(launch.getByLabel("Spec")).toHaveValue("006");
      await launch.locator('label:has([value="claude"])').click();
      await expect(launch.getByLabel("Role")).toBeVisible();
      if (!mobile) {
        const fits = await launch.evaluate(form => {
          const option = form.querySelector(".isolation-option").getBoundingClientRect();
          return option.bottom <= form.getBoundingClientRect().bottom && form.scrollTop === 0;
        });
        expect(fits).toBe(true);
      }
      await shot("launch");
      await launch.locator('label:has([value="shell"])').click();
      await launch.getByRole("button", { name: /Open Terminal/ }).click();
      await expect(launch).toBeHidden();
      await expect.poll(async () => {
        const response = await request.get(`${origin}/api/state`, { headers: auth });
        return (await response.json()).sessions?.find(session => session.specId === "006" && session.terminal?.running)?.role;
      }).toBeNull();

      await openNav();
      const unlinked = nav.locator('[data-bench-group="unlinked"]');
      await unlinked.locator("summary").click();
      await unlinked.locator('[data-bench-spec="099"]').click();
      await expect(panel.getByText("No work item is linked.")).toBeVisible();
      await expect(panel.getByText("No requirement linked")).toBeVisible();
      await shot("unlinked");
      await openNav();
      await nav.getByRole("button", { name: "New work item" }).click();
      const draft = page.locator("#bench-draft-form");
      await expect(draft.getByLabel("Agent").locator('option[value="codex"]')).toHaveCount(1);
      await draft.getByLabel("Agent").selectOption("codex");
      const title = `Small example ${width} ${theme}`;
      await draft.getByLabel("Work item title").fill(title);
      await draft.getByLabel("Size").selectOption("small");
      await draft.getByLabel("Owner's brief").fill("Show the owner how a small work item starts.");
      await shot("draft-with-agent");
      await draft.getByRole("button", { name: "Start drafting" }).click();
      await expect(draft).toBeHidden();

      const item = nav.locator("details").filter({ hasText: new RegExp(`${title} · Draft`, "i") });
      await openNav();
      await expect(item).toBeVisible();
      const id = await item.getAttribute("data-bench-group");
      await expect.poll(async () => {
        const response = await request.get(`${origin}/api/work-items/${id}/draft/brd`, { headers: auth });
        return (await response.json()).content;
      }).toContain("G1: Trace the outcome");
      await item.getByRole("button", { name: new RegExp(`${title} · Draft`, "i") }).click();
      await expect(panel.getByText("Draft", { exact: true })).toBeVisible();
      await shot("work-item-draft");
      await panel.getByRole("button", { name: "Review BRD draft" }).click();
      const review = page.locator("#bench-draft-review-form");
      await expect(review.getByText("G1: Trace the outcome", { exact: false })).toBeVisible();
      await shot("review-draft");
      await review.getByRole("button", { name: "Publish reviewed draft" }).click();
      await expect(review).toBeHidden();
      await expect(panel.getByText("Published", { exact: true })).toBeVisible();
      await expect(panel.getByRole("button", { name: "Draft PRD with agent" })).toBeVisible();
      await expect(panel.getByRole("button", { name: "Draft BRD with agent" })).toBeHidden();
      await panel.getByLabel("BRD actions").click();
      await expect(panel.getByRole("button", { name: "Redraft with agent" })).toBeVisible();
      await shot("published-small-item");

      if (mobile) await page.keyboard.press("Escape");
      await openNav();
      await nav.locator('[data-view="needs-you"]').click();
      const needs = page.locator("#bench-planning-list");
      const attention = needs.locator("section").filter({ hasText: id });
      await expect(attention.getByText("plan-fixture-change.txt")).toBeVisible({ timeout: 40_000 });
      await shot("needs-you");
      await attention.getByRole("button", { name: "Open plan-fixture-change.txt" }).click();
      await expect(page.locator("#studio-document-path")).toHaveText("plan-fixture-change.txt");
      await openNav();
      await nav.locator('[data-view="needs-you"]').click();
      await attention.getByRole("button", { name: "Mark handled" }).click();
      await expect(attention.getByText(/Handled by/)).toBeVisible();
      await shot("handled");
      await expect.poll(async () => {
        const response = await request.get(`${origin}/api/state`, { headers: auth });
        return (await response.json()).sessions?.find(session => session.workItemId === id && session.planningHandled)?.planningHandled;
      }).toEqual({ actor: expect.any(String), at: expect.any(String) });
      const project = join(fixture, "Bench workspace");
      const redraft = await request.post(`${origin}/api/work-items/draft`, { headers: auth, data: { id, document: "brd", agent: "codex", brief: "Redraft the BRD." } });
      expect(redraft.ok()).toBe(true);
      const draftPath = join(project, "work", id, "brd.draft.md");
      await expect.poll(() => readFileSync(draftPath, "utf8")).toContain("G1: Trace the outcome");
      writeFileSync(draftPath, "# " + title + "\n\nSize: small\n\nProblem: Improve the plan\nG1: Trace the outcome\nDone when: The owner publishes\n");
      await page.reload();
      await openNav();
      await nav.locator(`[data-bench-work="${id}"]`).click();
      await panel.getByRole("button", { name: "Review BRD draft" }).click();
      await expect(review.locator("#bench-draft-difference")).toContainText("- Problem: Show work planning");
      await expect(review.locator("#bench-draft-difference")).toContainText("+ Problem: Improve the plan");
      if (process.env.VERB_SLICE4_SCREEN_DIR)
        await page.screenshot({ animations: "disabled", path: join(process.env.VERB_SLICE4_SCREEN_DIR, `redraft-difference-${width}-${theme}.png`) });
      await review.getByRole("button", { name: "Cancel" }).click();
      expect(readFileSync(draftPath, "utf8")).toContain("Improve the plan");
      rmSync(draftPath);
      rmSync(join(fixture, "Bench workspace", "plan-fixture-change.txt"), { force: true });
    });
  }
}

test("Preview Files mode stays reachable for an empty project", async ({ page }) => {
  await page.route("**/api/files", route => route.fulfill({ json: { files: [], truncated: false } }));
  const files = page.waitForResponse(response => response.url().endsWith("/api/files"));
  await page.goto(`${origin}/#${token}`);
  await files;
  await page.locator("#bench-tab-preview").click();
  await page.locator("#bench-files-mode").click();
  await expect(page.locator("#file-tree")).toContainText("No files yet.");
});

for (const width of [1440, 390]) {
  for (const theme of ["light", "dark"]) {
    test(`Preview keeps its editor and terminals at ${width} in ${theme}`, async ({ page, request }) => {
      test.setTimeout(90_000);
      const mobile = width === 390;
      const headers = { "X-Verb-Token": token };
      const dir = process.env.VERB_SLICE7A_SCREEN_DIR || process.env.VERB_SLICE6_SCREEN_DIR || process.env.VERB_SLICE5_SCREEN_DIR;
      const shot = name => dir
        ? page.screenshot({ animations: "disabled", path: join(dir, `${name}-${width}-${theme}.png`) })
        : Promise.resolve();
      for (let i = 0; i < 2; i++) {
        const response = await request.post(`${origin}/api/terminals`, { headers, data: { agent: "shell" } });
        expect(response.ok()).toBe(true);
      }
      await page.setViewportSize({ width, height: mobile ? 844 : 900 });
      await page.emulateMedia({ colorScheme: theme });
      await page.goto(`${origin}/#${token}`);
      const terminals = page.getByRole("tablist", { name: "Terminals", exact: true });
      await expect(terminals.getByRole("tab")).toHaveCount(2);
      await expect(page.locator("#bench-work-tree")).toContainText("Bench consolidation");
      const originalTile = await page.locator(".terminal-tile.focused").elementHandle();
      const panel = page.locator("#bench-preview");
      if (mobile) await page.locator("#bench-toggle-right").click();
      await page.locator("#bench-tab-preview").click();
      await expect(panel.locator("#studio-document")).toContainText("A document in the project.");
      await shot("preview-document");
      await expect(panel.getByLabel("Canvas file", { exact: true })).toBeHidden();
      await expect(panel.getByRole("button", { name: "Close current preview" })).toBeHidden();
      await expect(panel.getByRole("button", { name: "Refresh canvas", exact: true })).toBeHidden();
      const search = panel.locator("#studio-file-search");
      await search.fill("preview");
      await expect(panel.getByRole("button", { name: "preview.png", exact: true })).toBeVisible();
      await shot("preview-file-picker");
      await panel.getByRole("button", { name: "preview.png", exact: true }).click();
      const image = panel.locator("#studio-preview-image");
      await expect(image).toBeVisible();
      await expect.poll(() => image.evaluate(node => node.naturalWidth)).toBe(1);
      await shot("preview-image");
      await search.fill("preview");
      await panel.getByRole("button", { name: "preview.html", exact: true }).click();
      await expect(panel.frameLocator("#studio-app-frame iframe").getByRole("heading", { name: "Bench preview app" })).toBeVisible();
      await expect(panel.locator("#studio-app-frame iframe")).toHaveAttribute("sandbox", "allow-scripts");
      await shot("preview-app");
      await panel.getByRole("button", { name: "Source", exact: true }).click();
      if (width === 1440 && theme === "light") {
        let release;
        const gate = new Promise(resolve => { release = resolve; });
        await page.route("**/api/studio/preview", async route => { await gate; await route.continue(); }, { times: 1 });
        const pending = page.waitForRequest(request => request.url().endsWith("/api/studio/preview"));
        const response = page.waitForResponse(response => response.url().endsWith("/api/studio/preview"));
        await panel.getByRole("button", { name: "Preview", exact: true }).click();
        await pending;
        await page.locator("#bench-tab-plan").click();
        release();
        await response;
        await page.locator("#bench-tab-preview").click();
        await expect(panel.locator("#studio-app-frame iframe")).toHaveCount(0);
      }
      await panel.getByRole("button", { name: "Edit", exact: true }).click();
      await panel.getByRole("button", { name: "Plain text", exact: true }).click();
      const editor = panel.locator("#studio-editor-text");
      const draft = "<!doctype html><h1>Unsaved preview draft</h1>";
      await editor.fill(draft);
      const originalEditor = await editor.elementHandle();
      await shot("preview-draft");
      await page.locator("#bench-tab-review").click();
      await page.locator("#bench-tab-preview").click();
      await expect(editor).toHaveValue(draft);
      if (mobile) await page.getByRole("button", { name: "Close details" }).click();
      await terminals.getByRole("tab", { selected: false }).click();
      if (mobile) await page.locator("#bench-toggle-right").click();
      await expect(editor).toHaveValue(draft);
      expect(await editor.evaluate((node, previous) => node === previous, originalEditor)).toBe(true);
      if (mobile) await page.getByRole("button", { name: "Close details" }).click();
      await terminals.getByRole("tab", { selected: false }).click();
      expect(await page.locator(".terminal-tile.focused").evaluate((node, previous) => node === previous, originalTile)).toBe(true);
      expect(readFileSync(join(fixture, "Bench workspace", "preview.html"), "utf8")).toContain("Bench preview app");
    });
  }
}

for (const width of [1440, 390]) {
  for (const theme of ["light", "dark"]) {
    test(`Context and Settings cutover at ${width} in ${theme}`, async ({ page, request }) => {
      test.setTimeout(60_000);
      const headers = { "X-Verb-Token": token };
      const mobile = width === 390;
      const project = join(fixture, "Bench workspace");
      const dir = process.env.VERB_SLICE7A_SCREEN_DIR;
      const shot = name => dir && page.screenshot({ animations: "disabled", path: join(dir, `${name}-${width}-${theme}.png`) });
      const launched = await request.post(`${origin}/api/terminals`, { headers, data: { agent: "shell" } });
      expect(launched.ok()).toBe(true);
      const session = (await launched.json()).sessionId;
      const title = `Cutover task ${width} ${theme}`;
      const task = await request.post(`${origin}/api/tasks`, { headers, data: { title, brief: "Verify the full queue." } });
      expect(task.ok()).toBe(true);
      await page.setViewportSize({ width, height: mobile ? 844 : 900 });
      await page.emulateMedia({ colorScheme: theme });
      await page.addInitScript(() => localStorage.setItem("verb.layout.navigation", "legacy"));
      await page.goto(`${origin}/?view=memory#${token}`);
      const context = page.locator("#view-context");
      await expect(context).toBeVisible();
      await expect(page.locator("#app")).toHaveClass(/bench-layout/);
      await expect(context.getByRole("heading", { name: "Shared notes", exact: true })).toBeVisible();
      const brief = join(project, "docs", "project", "BRIEF.md");
      mkdirSync(join(project, "docs", "project"), { recursive: true });
      writeFileSync(brief, "# Project brief\n\n## Problem\n\nA shared place to plan.\n\n## Scope\n\nThis checkout.\n\n## Constraints\n\nBounded notes.\n");
      const refresh = context.getByRole("button", { name: "Refresh context" });
      await refresh.click();
      await expect(context.locator("#context-brief-text")).toContainText("A shared place to plan.");
      await expect(context.locator("#managed-context-size")).toContainText("/ 8192 bytes");
      await expect(context.locator("#managed-context-preview")).toContainText("A shared place to plan.");
      const sync = context.getByRole("button", { name: "Sync agent context", exact: true });
      await sync.click();
      await expect(context.locator("#agent-files [data-state=current]")).toHaveCount(2);
      await expect(context.locator("#managed-context-git")).toContainText("Git");
      expect(readFileSync(join(project, ".git", "info", "exclude"), "utf8")).toContain("/AGENTS.md");
      await context.getByRole("heading", { name: "Context", exact: true }).scrollIntoViewIfNeeded();
      await shot("context");
      const instructions = join(project, "AGENTS.md");
      writeFileSync(instructions, readFileSync(instructions, "utf8").replace("A shared place to plan.", "External edit."));
      await refresh.click();
      await expect(context.locator("#agent-files [data-state=outdated]")).toHaveCount(1);
      await shot("context-stale");
      await sync.click();
      await expect(context.locator("#agent-files [data-state=current]")).toHaveCount(2);

      const nav = page.locator("#bench-navigation");
      if (mobile) await page.locator("#bench-toggle-navigation").click();
      await nav.getByRole("button", { name: "Settings", exact: true }).click();
      const settings = page.locator("#view-settings");
      await expect(settings.locator("#host-verb")).toContainText("Version");
      await shot("settings-host");
      await settings.getByRole("button", { name: "Connections", exact: true }).click();
      await expect(settings.locator("#settings-connections")).toBeVisible();
      await shot("settings-connections");
      await settings.getByRole("button", { name: "Tasks", exact: true }).click();
      await expect(settings.getByRole("searchbox", { name: "Find a task" })).toBeVisible();
      await expect(settings.locator("#task-list")).toContainText(title);
      await shot("settings-tasks");
      await settings.getByRole("button", { name: "Session history", exact: true }).click();
      await expect(settings.locator(`#session-list [data-session-id="${session}"]`)).toBeVisible();
      await shot("settings-session-history");
      await settings.getByRole("button", { name: "Phone pairing", exact: true }).click();
      await expect(settings.locator(`[data-phone-terminal="${session}"]`)).toBeVisible();
      await shot("settings-phone");
      // Use the real terminal-scoped pairing offer, renew it and revoke it.
      await settings.locator(`[data-phone-terminal="${session}"]`).click();
      const pairing = page.locator("#phone-dialog");
      await expect(pairing).toBeVisible();
      await expect(pairing.locator("#phone-qr")).toHaveAttribute("src", /^data:image\/png/);
      const originalLink = await pairing.locator("#phone-link").inputValue();
      await pairing.locator("#phone-renew").click();
      await expect(pairing.locator("#phone-link")).not.toHaveValue(originalLink);
      await pairing.locator("#phone-stop").click();
      await expect(pairing).toBeHidden();
      rmSync(brief);
    });
  }
}

for (const width of [1440, 390]) {
  for (const theme of ["light", "dark"]) {
    test(`Moved workflows stay reachable at ${width} in ${theme}`, async ({ page, request }) => {
      const headers = { "X-Verb-Token": token };
      const suffix = `${width}-${theme}`;
      const created = await request.post(`${origin}/api/specs`, { headers, data: {
        title: `Removal journey ${suffix}`, criteria: ["A completion claim stays unproven"],
      } });
      expect(created.ok()).toBeTruthy();
      const spec = await created.json();
      const shot = name => process.env.VERB_REMOVAL_SCREEN_DIR
        ? page.screenshot({ animations: "disabled", path: join(process.env.VERB_REMOVAL_SCREEN_DIR, `${name}-${suffix}.png`) })
        : Promise.resolve();
      const palette = async title => {
        const close = page.getByRole("button", { name: "Close details", exact: true });
        if (await close.isVisible()) await close.click();
        await page.getByRole("button", { name: "Search or run", exact: true }).click();
        await page.locator("#palette-input").fill(title);
        await page.getByRole("option").filter({ has: page.getByText(title, { exact: true }) }).click();
      };
      await page.setViewportSize({ width, height: width === 390 ? 844 : 900 });
      await page.emulateMedia({ colorScheme: theme });
      await page.goto(`${origin}/?view=project#${token}`);
      const files = page.locator("#bench-files-panel");
      await expect(files).toBeVisible();
      const specsFolder = files.locator('[data-dir="specs"]');
      await specsFolder.click();
      await expect(files.locator('[data-file="specs/099-legacy.md"] .file-status')).toHaveText("U");
      await specsFolder.click();
      await files.getByRole("treeitem", { name: "README.md", exact: true }).click();
      await expect(page.locator("#studio-document-path")).toHaveText("README.md");
      await page.evaluate(() => {
        navigator.clipboard.writeText = async text => { window.fixtureCopiedPath = text; };
      });
      await files.getByRole("button", { name: "Copy path", exact: true }).click();
      await expect.poll(() => page.evaluate(() => window.fixtureCopiedPath)).toBe("README.md");
      await shot("files");
      await page.goto(`${origin}/?view=overview#${token}`);
      const mark = page.waitForResponse(response => response.url().endsWith("/api/good/mark") && response.request().method() === "POST");
      await page.getByRole("button", { name: "Mark as good", exact: true }).click();
      expect((await mark).ok()).toBeTruthy();
      await shot("checks");
      await palette(`Open spec ${spec.id} · ${spec.title}`);
      await page.getByRole("tab", { name: "Review", exact: true }).click();
      const row = page.locator(".bench-proof-list > li");
      await row.getByRole("button", { name: "Record completion claim" }).click();
      await shot("criterion-claim");
      await page.getByRole("button", { name: "Record claim", exact: true }).click();
      await expect(row).toContainText("Proof still needed");
      await expect(row).toContainText("Unproven");
      await row.getByRole("button", { name: "Record proof", exact: true }).click();
      await expect(page.locator("#sdlc-dialog-title")).toHaveText("Link an evidence report");
      await shot("criterion-proof");
      await page.keyboard.press("Escape");
      await page.getByText("Evidence actions", { exact: true }).click();
      await shot("evidence-actions");
      if (width === 1440 && theme === "light") {
        await page.route("**/api/checks", route => route.fulfill({ json: {
          clear: false, repository: [{ level: "caution", fact: "Fixture check refreshed", safeNext: "Inspect project settings" }], runtimes: [],
        } }));
        await page.getByRole("button", { name: "Check again", exact: true }).click();
        await expect(page.getByText("Fixture check refreshed", { exact: true })).toBeVisible();
        await expect(page.getByRole("button", { name: "Evidence history", exact: true })).toBeVisible();
        await page.unroute("**/api/checks");
        await page.getByRole("button", { name: "Check again", exact: true }).click();
      }
      await page.getByRole("button", { name: "Evidence history", exact: true }).click();
      await expect(page.locator("#sdlc-history")).toContainText("No lifecycle records yet");
      await shot("evidence-history");
      await palette(`Start agent on spec ${spec.id}`);
      const launch = page.locator("#session-form");
      await launch.locator('label:has([name="agent"][value="agy"])').click();
      await launch.getByLabel("Plan in Antigravity Talk").check();
      await expect(launch.locator('[name="isolated"]')).toBeDisabled();
      await shot("talk-launch");
      const admission = page.waitForResponse(response => response.url().endsWith(`/api/specs/${spec.id}/talk`) && response.request().method() === "POST");
      await launch.getByRole("button", { name: "Open Antigravity Talk", exact: false }).click();
      expect((await admission).ok()).toBeTruthy();
      const talk = page.locator(".talk-tile");
      await expect(talk.getByRole("button", { name: "Continue in terminal" })).toBeVisible();
      await talk.getByRole("button", { name: "End", exact: true }).click();
      await expect(talk).toHaveCount(0);

      await page.goto(`${origin}/?view=specs#${token}`);
      await expect(page.locator("#view-bench-board")).toBeVisible();
      await palette("Open Tasks");
      await page.getByRole("button", { name: "New task", exact: true }).click();
      await expect(page.locator("#task-form [name=specId]")).toHaveValue("");
      await shot("task");
      await page.keyboard.press("Escape");
      const agent = width === 1440 ? "claude" : "shell";
      const started = await request.post(`${origin}/api/terminals`, { headers, data: { agent } });
      expect(started.ok()).toBeTruthy();
      const { sessionId } = await started.json();
      expect((await request.delete(`${origin}/api/terminals/${sessionId}`, { headers })).ok()).toBeTruthy();
      await palette("Open Session history");
      await page.getByRole("searchbox", { name: "Find a session" }).fill(sessionId);
      await page.locator(`#session-list [data-session-id="${sessionId}"]`).click();
      const detail = page.locator("#session-history-detail");
      await expect(detail.getByRole("heading", { name: new RegExp(sessionId.slice(0, 8)) })).toBeVisible();
      await expect(detail.getByRole("button", { name: /Start a new .* session/ })).toBeVisible();
      await shot("session-history-detail");
      await page.goto(`${origin}/?view=studio#${token}`);
      await expect(page.locator("#view-studio")).toBeHidden();
      await palette("Open Studio");
      await expect(page.locator("#view-studio")).toBeVisible();
    });
  }
}

test("Plan links a legacy spec and creates a spec from its work item", async ({ page }) => {
  await page.goto(`${origin}/#${token}`);
  await page.locator('[data-bench-group="unlinked"] summary').click();
  await page.locator('[data-bench-spec="099"]').click();
  await page.getByRole("button", { name: "Link to work item" }).click();
  await page.locator("#spec-link-work-item").selectOption("001-bench-consolidation");
  await page.getByRole("button", { name: "Link spec", exact: true }).click();
  await expect(page.locator("#spec-link-dialog")).not.toBeVisible();
  await expect(page.locator("#bench-plan-spec")).toContainText("Why (BRD)");
  expect(readFileSync(join(fixture, "Bench workspace/specs/099-legacy.md"), "utf8"))
    .toContain("linked work item 001-bench-consolidation");

  await page.locator('[data-bench-new-spec="001-bench-consolidation"]').click();
  await expect(page.locator("#spec-work-item")).toHaveValue("001-bench-consolidation");
  await page.locator("#spec-title-input").fill("Work item spec from Plan");
  await page.locator('#spec-dialog-form button[type="submit"]').click();
  await expect(page.locator("#spec-dialog")).not.toBeVisible();
  await expect(page.locator("#bench-plan-spec")).toContainText("Why (BRD)");
});

test("planning agent can draft a PRD using the prompt example", async ({ page }) => {
  await page.goto(`${origin}/#${token}`);
  await page.locator('[data-bench-work="001-bench-consolidation"]').click();
  const row = page.locator("#bench-plan-spec .bench-documents > li").nth(1);
  await row.locator("summary").click();
  await row.getByRole("button", { name: "Redraft with agent" }).click();
  await page.locator("#bench-draft-form").getByRole("combobox", { name: "Agent" }).selectOption("codex");
  await page.locator('#bench-draft-form button[type="submit"]').click();
  await expect.poll(() => {
    try {
      return readFileSync(join(fixture, "Bench workspace/work/001-bench-consolidation/prd.draft.md"), "utf8");
    } catch { return ""; }
  }).toContain("- R1 · G1 · A returning customer");
});

test("proof sends uncommitted work to commit review and restores the form", async ({ page }) => {
  await page.goto(`${origin}/#${token}`);
  await page.keyboard.press("ControlOrMeta+k");
  await page.locator("#palette-input").fill("Open spec 097");
  await page.keyboard.press("Enter");
  await page.getByRole("tab", { name: "Review", exact: true }).click();
  await page.locator(".bench-proof-list").getByRole("button", { name: "Record proof", exact: true }).first().click();
  const dialog = page.locator("#sdlc-dialog");
  await expect(dialog.getByText("Commit code changes before recording proof.")).toBeVisible();
  await dialog.locator("#sdlc-text").fill("Evidence details to preserve");
  await dialog.getByRole("button", { name: "Commit changes", exact: true }).click();
  await expect(page.locator("#commit-dialog")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeVisible();
  await expect(dialog.locator("#sdlc-text")).toHaveValue("Evidence details to preserve");
});

test("Plan starts a terminal with the unlinked spec selected", async ({ page, request }) => {
  const headers = { "X-Verb-Token": token };
  const response = await request.post(`${origin}/api/specs`, { headers, data: { title: "Unlinked launch fixture" } });
  expect(response.ok()).toBeTruthy();
  const spec = await response.json();
  await page.goto(`${origin}/#${token}`);
  await page.locator('[data-bench-group="unlinked"] summary').click();
  await page.locator(`[data-bench-spec="${spec.id}"]`).click();
  await page.locator("#bench-plan").getByRole("button", { name: "Start terminal", exact: true }).click();
  await expect(page.locator("#session-dialog")).toBeVisible();
  await expect(page.locator('#session-form [name="specId"]')).toHaveValue(spec.id);
  await page.locator('#session-form button[type="submit"]').click();
  await expect(page.locator("#bench-terminal-context-summary")).toContainText(`Spec ${spec.id}`);
});
