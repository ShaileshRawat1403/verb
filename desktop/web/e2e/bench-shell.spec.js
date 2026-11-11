import { test, expect } from "@playwright/test";
import { execFileSync, spawn } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
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
  execFileSync("git", ["-C", project, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
    "commit", "--allow-empty", "-qm", "Start"]);
  const bin = join(fixture, "bin");
  mkdirSync(bin);
  writeFileSync(join(bin, "claude"), "#!/bin/sh\nprintf '\\033[?2004hAgent ready\\n'\nexec cat\n", { mode: 0o755 });
  writeFileSync(join(project, "specs/099-legacy.md"),
    "---\nid: 099\ntitle: Legacy sample\nstage: plan\n---\n## Acceptance criteria\n- [ ] A legacy criterion\n");
  writeFileSync(join(bin, "codex"), String.raw`#!/bin/sh
for prompt do :; done
draft=$(printf '%s\n' "$prompt" | sed -n 's/^Draft path: //p' | head -n 1)
case "$draft" in
  *.brd.draft.md|*/brd.draft.md)
    printf '# Small planning example\n\nSize: small\n\nProblem: Show work planning\nG1: Trace the outcome\nDone when: The owner publishes\n' > "$draft"
    printf 'Review this scope change\n' > plan-fixture-change.txt
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
      await group.locator("summary").click();
      await expect(group.locator("summary")).toHaveText(/Bench consolidation\s+9/);
      await expect(nav.locator('[data-bench-group="unlinked"]')).not.toHaveAttribute("open", "");
      await shot("navigation");
      await group.getByRole("button", { name: /006 · Terminal-first Shell/ }).click();
      await expect(panel.getByRole("heading", { name: /Terminal-first Shell/ })).toBeVisible();
      await expect(panel.locator(".bench-documents > li > div")).toHaveText(["Why (BRD)Published", "What (PRD)Published"]);
      await shot("plan");
      for (const [label, file] of [["Why (BRD)", "brd.md"], ["What (PRD)", "prd.md"]]) {
        await panel.getByRole("button", { name: label }).click();
        await expect(page.locator("#preview-path")).toHaveText(`work/001-bench-consolidation/${file}`);
        await run("Open Sessions");
        if (mobile) await details.click();
      }
      await panel.getByRole("tab", { name: "Plan" }).press("ArrowRight");
      await expect(panel.getByRole("tab", { name: "Review", exact: true })).toHaveAttribute("aria-selected", "true");
      await shot("review");
      await panel.getByRole("tab", { name: "Preview" }).click();
      await expect(panel.getByRole("button", { name: "Open project files" })).toBeVisible();
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
      await nav.getByRole("button", { name: "Board" }).click();
      await expect(page.locator("#view-specs")).toBeVisible();
      await expect(page.locator("#spec-view-board")).toBeVisible();
      await expect(page.locator("#spec-view-board")).toHaveAttribute("aria-pressed", "true");
      await expect(panel).toBeHidden();
      await shot("board");
      if (mobile) await navigation.click();
      await nav.getByRole("button", { name: /Needs you/ }).click();
      await expect(page.locator("#view-needs-you")).toBeVisible();
      await shot("needs-you");
      await openSettings();
      await shot("settings");
      const legacy = page.getByRole("checkbox", { name: "Use previous navigation" });
      await legacy.check();
      await expect(app).not.toHaveClass(/bench-layout/);
      const viewLinks = page.getByRole("navigation", { name: "Workspace views" });
      await viewLinks.getByRole("button", { name: "Studio" }).click();
      if (mobile) await page.locator("#studio-focus-work").click();
      const draft = page.locator("#studio-intent");
      const newChange = page.locator("#studio-new-change-toggle");
      if (await newChange.isVisible()) await newChange.click();
      await draft.fill("Keep this draft through the layout switch");
      await openSettings();
      await legacy.uncheck();
      await expect(app).toHaveClass(/bench-layout/);
      await viewLinks.getByRole("button", { name: "Studio" }).click();
      await expect(draft).toHaveValue("Keep this draft through the layout switch");
      await openSettings();
      await viewLinks.getByRole("button", { name: "Sessions" }).click();
      await expect(tile).toHaveAttribute("id", id);
      expect(await tile.evaluate((node, previous) => node === previous, domTile)).toBeTruthy();
      await command("after-layout-switch");
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
  await page.locator('[data-bench-group="001-bench-consolidation"] summary').click();
  await page.locator('[data-bench-spec="006"]').click();
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
      await nav.locator('[data-bench-group="001-bench-consolidation"] summary').click();
      await nav.locator('[data-bench-spec="001"]').click();
      if (mobile) await page.keyboard.press("Escape");
      const selected = page.getByRole("button", { name: /Start a terminal for 001/ });
      await expect(selected).toBeVisible();
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
    test(`Plan traces work and publishes drafts at ${width} in ${theme}`, async ({ page, request }) => {
      const mobile = width === 390;
      test.setTimeout(90_000);
      await page.setViewportSize({ width, height: mobile ? 844 : 900 });
      await page.emulateMedia({ colorScheme: theme });
      const shot = name => process.env.VERB_SLICE3_SCREEN_DIR
        ? page.screenshot({ animations: "disabled", path: join(process.env.VERB_SLICE3_SCREEN_DIR, `${name}-${width}-${theme}.png`) })
        : Promise.resolve();
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
      await group.locator("summary").click();
      await group.locator('[data-bench-spec="006"]').click();
      await expect(panel.getByText("How (Spec)")).toBeVisible();
      await expect(panel.getByText("R6", { exact: true }).first()).toBeVisible();
      await expect(panel.getByText("G2", { exact: true }).first()).toBeVisible();
      await shot("plan");
      await panel.getByRole("button", { name: "New spec task" }).click();
      const task = page.locator("#bench-plan-task-form");
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
      await nav.getByRole("button", { name: "Draft a work item" }).click();
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
      await item.locator("summary").click();
      await item.getByRole("button", { name: "Open work item" }).click();
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
      await shot("published-small-item");

      if (mobile) await page.keyboard.press("Escape");
      await openNav();
      await nav.getByRole("button", { name: /Needs you/ }).click();
      const needs = page.locator("#bench-needs-list");
      await expect(needs.getByText("plan-fixture-change.txt").first()).toBeVisible({ timeout: 40_000 });
      await shot("needs-you");
      rmSync(join(fixture, "Bench workspace", "plan-fixture-change.txt"), { force: true });
    });
  }
}
