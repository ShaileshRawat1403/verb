import { test, expect } from "@playwright/test";
import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const bin = resolve(process.env.VERB_BIN ?? "../target/debug/verb");

async function workspace(name, token) {
  const fixture = mkdtempSync(join(tmpdir(), "verb-project-switch-"));
  const project = join(fixture, name);
  mkdirSync(project);
  const child = spawn(bin, ["web", "--port", "0"], {
    cwd: project,
    env: {
      PATH: process.env.PATH,
      HOME: fixture,
      VERB_STATE_DIR: join(fixture, "state"),
      VERB_TOKEN: token,
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  const close = async () => {
    if (child.exitCode === null && child.signalCode === null) {
      await new Promise((done) => {
        child.once("exit", done);
        child.kill();
      });
    }
    rmSync(fixture, { recursive: true, force: true });
  };
  try {
    const origin = await new Promise((done, fail) => {
      let output = "";
      const timeout = setTimeout(() => fail(new Error("Verb did not start")), 10000);
      child.stdout.on("data", (chunk) => {
        output += chunk;
        const origin = /^Verb web UI: (http:\/\/127\.0\.0\.1:\d+)/m.exec(output)?.[1];
        if (origin) {
          clearTimeout(timeout);
          done(origin);
        }
      });
      child.once("exit", (code) => {
        clearTimeout(timeout);
        fail(new Error(`Verb exited with ${code}`));
      });
    });
    return { origin, url: `${origin}/#${token}`, close };
  } catch (error) {
    await close();
    throw error;
  }
}

const sourceToken = "source-fixture-token-00000000000000000000";
const targetToken = "target-fixture-token-00000000000000000000";
let source;
let target;

test.beforeAll(async () => {
  source = await workspace("Source workspace", sourceToken);
  target = await workspace("Destination workspace", targetToken);
});

test.afterAll(async () => {
  await Promise.all([source?.close(), target?.close()]);
});

for (const width of [1440, 390]) {
  test(`opens an independent project and preserves the source terminal and draft at ${width}`, async ({ page, context }) => {
    await page.setViewportSize({ width, height: width === 390 ? 844 : 900 });
    await page.goto(source.url);
    await page.locator("#quick-terminal-empty").click();
    const input = page.getByRole("textbox", { name: "Terminal input" });
    await expect(input).toBeVisible();
    await input.pressSequentially("printf 'original-terminal\\n'");
    await input.press("Enter");
    const terminal = page.locator(".terminal-tile");
    const originalId = await terminal.getAttribute("id");
    const output = () => page.request.get(
      `${source.origin}/api/terminals/${originalId.slice("terminal-".length)}/output`,
      { headers: { "X-Verb-Token": sourceToken } },
    ).then((response) => response.json()).then((response) => Buffer.from(response.data, "base64").toString());
    await expect.poll(output).toContain("original-terminal");

    await page.getByRole("button", { name: "Studio", exact: true }).click();
    if (width === 390) await page.locator("#studio-focus-work").click();
    const draft = page.locator("#studio-intent");
    await draft.fill("Keep this draft in the source project");

    await page.getByRole("button", { name: "Open another project" }).click();
    const dialog = page.getByRole("dialog", { name: "Open another project" });
    const url = dialog.getByLabel("Project URL");
    await url.fill("javascript:alert(1)");
    await dialog.getByRole("button", { name: "Open project", exact: true }).click();
    await expect(dialog.getByRole("alert")).toBeVisible();
    await expect(url).toHaveAttribute("aria-invalid", "true");
    await url.fill(target.url);

    const authorization = [];
    context.on("request", (request) => {
      if (request.url().startsWith(`${target.origin}/api/`)) {
        authorization.push(request.headers()["x-verb-token"]);
      }
    });
    const [other] = await Promise.all([
      context.waitForEvent("page"),
      dialog.getByRole("button", { name: "Open project", exact: true }).click(),
    ]);
    await expect(other.locator("#sidebar-project")).toHaveText("Destination workspace");
    expect(await other.evaluate(() => window.opener)).toBeNull();
    expect(authorization).toContain(targetToken);
    expect(authorization).not.toContain(sourceToken);
    await expect(dialog).toBeHidden();
    await expect(page.locator("#project-switch-url")).toHaveValue("");
    await expect(draft).toHaveValue("Keep this draft in the source project");

    await page.getByRole("button", { name: "Sessions", exact: true }).click();
    await expect(terminal).toHaveAttribute("id", originalId);
    await expect(input).toBeVisible();
    await input.pressSequentially("printf 'still-running\\n'");
    await input.press("Enter");
    await expect.poll(output).toContain("still-running");
    const stopped = await page.request.delete(
      `${source.origin}/api/terminals/${originalId.slice("terminal-".length)}`,
      { headers: { "X-Verb-Token": sourceToken } },
    );
    expect(stopped.ok()).toBeTruthy();
  });
}

test("hides the switcher for read-scoped access", async ({ page }) => {
  await page.route("**/api/whoami", (route) => route.fulfill({
    json: { token: "Read fixture", scope: "read", expires: 4000000000 },
  }));
  await page.goto(source.url);
  await expect(page.locator("#access-banner")).toContainText("read scope");
  await expect(page.locator("#open-project-switch")).toBeHidden();
});

for (const width of [1440, 390]) {
  for (const theme of ["light", "dark"]) {
    test(`project switcher at ${width} in ${theme}`, async ({ page }) => {
      await page.setViewportSize({ width, height: width === 390 ? 844 : 900 });
      await page.emulateMedia({ colorScheme: theme });
      await page.goto(source.url);
      const open = page.getByRole("button", { name: "Open another project" });
      await expect(open).toBeVisible();
      const directory = process.env.VERB_SWITCH_SCREEN_DIR;
      if (directory) await page.screenshot({ animations: "disabled", path: join(directory, `workspace-${width}-${theme}.png`) });
      await open.click();
      const dialog = page.getByRole("dialog", { name: "Open another project" });
      await expect(dialog).toBeVisible();
      await expect(dialog.getByLabel("Project URL")).toBeFocused();
      if (directory) await page.screenshot({ animations: "disabled", path: join(directory, `switcher-${width}-${theme}.png`) });
      await page.keyboard.press("Escape");
      await expect(dialog).toBeHidden();
    });
  }
}
