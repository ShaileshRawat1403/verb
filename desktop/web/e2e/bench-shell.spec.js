import { test, expect } from "@playwright/test";
import { spawn } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, rmSync } from "node:fs";
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
  child = spawn(resolve(process.env.VERB_BIN ?? "../target/debug/verb"), ["web", "--port", "0"], {
    cwd: project,
    env: { PATH: process.env.PATH, HOME: fixture, VERB_STATE_DIR: join(fixture, "state"), VERB_TOKEN: token },
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
      const settings = page.locator("#bench-settings");
      const shot = async name => {
        if (process.env.VERB_SHELL_SCREEN_DIR) {
          await page.screenshot({ animations: "disabled", path: join(process.env.VERB_SHELL_SCREEN_DIR, `${name}-${width}-${theme}.png`) });
        }
      };
      await expect(app).toHaveClass(/bench-layout/);
      await expect(page.locator("#view-sessions")).toBeVisible();
      if (mobile) await expect(nav).toBeHidden();
      else await expect(panel).toBeVisible();
      await shot("shell");

      if (mobile) await navigation.click();
      const group = nav.locator('details[data-bench-group="001-bench-consolidation"]');
      await group.locator("summary").click();
      await expect(nav.locator('[data-bench-group="unlinked"]')).not.toHaveAttribute("open", "");
      await shot("navigation");
      await group.getByRole("button", { name: /006 · Terminal-first Shell/ }).click();
      await expect(panel.getByRole("heading", { name: /Terminal-first Shell/ })).toBeVisible();
      await shot("plan");
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
      const tile = page.locator(".terminal-tile").first();
      await expect(tile).toBeVisible();
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
      await settings.click();
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
      await settings.click();
      await legacy.uncheck();
      await expect(app).toHaveClass(/bench-layout/);
      await viewLinks.getByRole("button", { name: "Studio" }).click();
      await expect(draft).toHaveValue("Keep this draft through the layout switch");
      await settings.click();
      await viewLinks.getByRole("button", { name: "Sessions" }).click();
      await expect(tile).toHaveAttribute("id", id);
      expect(await tile.evaluate((node, previous) => node === previous, domTile)).toBeTruthy();
      await command("after-layout-switch");
      const ended = await page.request.delete(`${origin}/api/terminals/${id.slice("terminal-".length)}`, { headers: { "X-Verb-Token": token } });
      expect(ended.ok()).toBeTruthy();
    });
  }
}
