import { test, expect } from "@playwright/test";
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

test("shared list and board support keyboard selection, attention filters and reviewed stage moves", async ({ page }) => {
  await page.goto(url); await page.keyboard.press("Alt+Digit1");
  await expect(page.locator("#spec-work-items article")).toHaveCount(2);
  await expect(page.locator("#spec-attention-list")).toContainText("Review requested");
  const board = page.getByRole("button", { name: "Board", exact: true });
  await board.focus(); await page.keyboard.press("Space");
  await expect(board).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator(".spec-board-column")).toHaveCount(6);
  await expect(page.locator("#spec-work-items label")).toHaveCount(0);
  await page.screenshot({ path: test.info().outputPath("spec-board-desktop.png") });
  await page.locator(`#spec-work-items [data-spec-id="${first.id}"]`).dragTo(page.locator('[data-board-stage="plan"]'));
  await expect(page.locator("#stage-dialog")).toBeVisible();
  expect(readFileSync(join(repo, first.file), "utf8")).toContain("stage: spec");
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Back to work overview", exact: true }).click();
  await page.locator(`#spec-work-items [data-spec-id="${first.id}"]`).click();
  await expect(page.locator("#spec-title")).toBeFocused();
  await expect(page.locator("#spec-facts")).toContainText("1/2 criteria recorded complete");
  await expect(page.locator("#spec-facts")).toContainText("Independent test results and evidence freshness have not been established");
  await expect(page.locator("#spec-facts")).toContainText("Release not observed");
  await page.getByLabel("Target stage", { exact: true }).selectOption("build");
  await page.getByRole("button", { name: "Review move…", exact: true }).click();
  await expect(page.locator("#stage-warnings")).toContainText("skipped plan");
  await page.locator("#stage-dialog-submit").click();
  await expect(page.locator(`#spec-work-items .spec-board-column[aria-label="Build"] [data-spec-id="${first.id}"]`)).toHaveCount(1);
  expect(readFileSync(join(repo, first.file), "utf8")).toContain("stage spec → build");
  await page.getByRole("button", { name: "Back to work overview", exact: true }).click();
  await expect(page.getByLabel("Find work", { exact: true })).toBeFocused();
  await page.getByLabel("Find work", { exact: true }).fill("contrast");
  await page.getByLabel("Needs you only").check();
  await expect(page.locator("#spec-work-items article")).toHaveCount(1);
  await page.getByLabel("Find work", { exact: true }).fill("recovery-does-not-match");
  await expect(page.locator("#spec-work-items")).toContainText("No specs match these filters");
  await page.getByLabel("Find work", { exact: true }).fill(""); await page.getByLabel("Needs you only").uncheck();
  const card = page.locator(`#spec-work-items [data-spec-id="${first.id}"]`);
  await card.focus(); await page.waitForTimeout(4500); // A workspace poll must not steal keyboard focus.
  await expect(card).toBeFocused();
  await page.keyboard.press("Enter");
  await page.getByLabel("Target stage", { exact: true }).selectOption("plan");
  await page.getByRole("button", { name: "Review move…", exact: true }).click();
  await expect(page.locator("#stage-dialog")).toBeVisible();
  appendFileSync(join(repo, first.file), "\n<!-- owner edit while reviewing -->\n");
  await page.locator("#stage-dialog-submit").click();
  await expect(page.locator("#toast")).toContainText("spec changed");
  await expect(page.locator("#stage-dialog")).toBeVisible();
  const raw = readFileSync(join(repo, first.file), "utf8");
  expect(raw).toContain("stage: build"); expect(raw).toContain("owner edit while reviewing");
  expect(raw).not.toContain("stage build → plan");
});

test("phone layout keeps named controls usable without dragging or motion", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto(url); await page.keyboard.press("Alt+Digit1");
  await page.getByRole("button", { name: "Board", exact: true }).click();
  await expect(page.locator("#spec-work-items article")).toHaveCount(2);
  const overflow = await page.evaluate(() => [...document.querySelectorAll("body *")]
    .filter(node => node.getBoundingClientRect().right > innerWidth && getComputedStyle(node).position !== "fixed")
    .map(node => ({ tag: node.tagName, id: node.id, className: node.className, right: node.getBoundingClientRect().right })));
  expect(await page.evaluate(() => document.documentElement.scrollWidth), JSON.stringify(overflow)).toBeLessThanOrEqual(390);
  for (const control of await page.locator(".spec-overview button, #spec-search, #spec-stage-filter").all()) {
    const bounds = await control.boundingBox();
    expect(bounds.height).toBeGreaterThanOrEqual(44);
    expect(bounds.x).toBeGreaterThanOrEqual(0);
    expect(bounds.x + bounds.width).toBeLessThanOrEqual(390);
  }
  await expect(page.getByLabel("Find work", { exact: true })).toBeVisible();
  await page.getByLabel("Show stage", { exact: true }).selectOption("review");
  await expect(page.locator("#spec-work-items article")).toHaveCount(1);
  await expect(page.locator("#spec-work-items")).toContainText("Improve contrast");
  await page.getByLabel("Show stage", { exact: true }).selectOption("all");
  for (const theme of ["dark", "light"]) {
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    await page.getByRole("button", { name: "Board", exact: true }).hover();
    const contrasts = await page.evaluate(() => {
      const luminance = color => {
        const values = color.match(/[\d.]+/g).slice(0, 3).map(Number).map(n => n / 255).map(n => n <= 0.04045 ? n / 12.92 : ((n + 0.055) / 1.055) ** 2.4);
        return values[0] * 0.2126 + values[1] * 0.7152 + values[2] * 0.0722;
      };
      return [...document.querySelectorAll('.work-card-note, .work-card-fact b, .spec-view-switch [aria-pressed="true"]')].map(node => {
        const foreground = luminance(getComputedStyle(node).color);
        const background = luminance(getComputedStyle(node.matches("button") ? node : node.closest(".spec-work-card")).backgroundColor);
        return (Math.max(foreground, background) + 0.05) / (Math.min(foreground, background) + 0.05);
      });
    });
    for (const contrast of contrasts) expect(contrast, `${theme} card text contrast`).toBeGreaterThanOrEqual(4.5);
  }
  await page.screenshot({ path: test.info().outputPath("spec-board-phone.png") });
  await page.locator(`#spec-work-items [data-spec-id="${first.id}"]`).click();
  await expect(page.getByLabel("Target stage", { exact: true })).toBeVisible();
  await expect(page.locator(".spec-overview")).toBeHidden();
  const terminal = page.getByRole("button", { name: "Terminal", exact: true });
  await expect(terminal).toBeVisible();
  expect((await terminal.boundingBox()).height).toBeGreaterThanOrEqual(44);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  const titleBounds = await page.locator("#spec-title").boundingBox();
  expect(titleBounds.y).toBeGreaterThanOrEqual(130);
  expect(titleBounds.y).toBeLessThan(350);
  await page.screenshot({ path: test.info().outputPath("spec-detail-phone.png") });
});

test("a failed spec read exposes an error instead of a false empty project", async ({ page }) => {
  await page.route("**/api/specs", route => route.fulfill({ status: 500, json: { error: "Could not read project specs" } }));
  await page.goto(url); await page.keyboard.press("Alt+Digit1");
  await expect(page.locator("#spec-workbench-error")).toContainText("Could not read project specs");
  await expect(page.locator("#spec-work-items")).toContainText("work list is unavailable");
  await expect(page.locator("#spec-empty")).toBeHidden();
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
    await page.goto(url); await page.keyboard.press("Alt+Digit1");
    await expect(page.locator("#spec-workbench-error")).toContainText("every spec file");
    await expect(page.locator("#spec-work-items")).toContainText("work list is unavailable");
    await expect(page.locator("#spec-empty")).toBeHidden();
  } finally { writeFileSync(path, original); }
  // The shared host may retain the read failure in its brief state cache. Prove recovery
  // before another journey starts instead of leaking that deliberate failure into it.
  await expect.poll(async () => {
    const restored = await request.get(`${origin}/api/state`, { headers });
    expect(restored.ok()).toBeTruthy();
    return Boolean((await restored.json()).specError);
  }).toBe(false);
  await page.getByRole("button", { name: "Refresh observations", exact: true }).click();
  await expect(page.locator("#spec-workbench-error")).toBeHidden();
  await expect(page.locator("#spec-work-items article")).toHaveCount(2);
});

test("desktop board preserves stage order, scrolls by keyboard and remembers focused filters", async ({ page }) => {
  await page.goto(url); await page.keyboard.press("Alt+Digit1");
  await page.getByRole("button", { name: "Board", exact: true }).click();
  await expect(page.locator(".spec-board-column")).toHaveCount(6);
  const rows = await page.locator(".spec-board-column").evaluateAll(nodes => nodes.map(node => Math.round(node.getBoundingClientRect().top)));
  expect(new Set(rows).size).toBe(1);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(1440);
  const region = page.getByRole("region", { name: "Spec board", exact: true });
  await region.focus(); await page.keyboard.press("ArrowRight");
  await expect.poll(() => region.evaluate(node => node.scrollLeft)).toBeGreaterThan(0);
  await page.getByLabel("Show stage", { exact: true }).selectOption("review");
  await expect(page.locator(".spec-board-column")).toHaveCount(1);
  await expect(page.locator("#spec-work-items article")).toHaveCount(1);
  await page.getByLabel("Needs you only").check();
  await page.getByLabel("Find work", { exact: true }).fill("missing");
  await expect(page.locator("#spec-work-items")).toContainText("No specs match these filters");
  await page.getByRole("button", { name: "Clear filters", exact: true }).click();
  await expect(page.getByLabel("Find work", { exact: true })).toBeFocused();
  await expect(page.getByLabel("Show stage", { exact: true })).toHaveValue("all");
  await expect(page.getByLabel("Needs you only")).not.toBeChecked();
  await expect(page.locator("#spec-work-items article")).toHaveCount(2);
  await page.getByLabel("Show stage", { exact: true }).selectOption("review");
  await page.reload(); await page.keyboard.press("Alt+Digit1");
  await expect(page.getByRole("button", { name: "Board", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByLabel("Show stage", { exact: true })).toHaveValue("review");
  await expect(page.locator("#spec-work-items article")).toHaveCount(1);
  await page.screenshot({ path: test.info().outputPath("stage-focused-board.png") });
});

test("overview controls and compact facts remain readable in both themes and layouts", async ({ page, request }) => {
  for (const [title, stage] of [["Clarify the welcome screen", "plan"], ["Keep the draft when switching files", "build"], ["Check the phone reading flow", "verify"], ["Review the wording before release", "ship"]]) {
    const response = await request.post(`${origin}/api/specs`, { headers, data: { title, criteria: ["A person can check the result"] } });
    expect(response.ok()).toBeTruthy();
    const spec = await response.json();
    const moved = await request.post(`${origin}/api/specs/${spec.id}/stage`, { headers, data: { stage, expectedRevision: spec.revision, note: "Visual fixture stage only; no execution, verification or release claimed." } });
    expect(moved.ok()).toBeTruthy();
  }
  await page.emulateMedia({ reducedMotion: "reduce" });
  for (const width of [1440, 390]) {
    await page.setViewportSize({ width, height: 900 });
    await page.goto(url); await page.keyboard.press("Alt+Digit1");
    const clear = page.getByRole("button", { name: "Clear filters", exact: true });
    if (await clear.isEnabled()) await clear.click();
    for (const theme of ["dark", "light"]) {
      for (let n = 0; n < 3 && await page.locator("html").getAttribute("data-resolved-theme") !== theme; n++)
        await page.getByRole("button", { name: "Change theme", exact: true }).click();
      await expect(page.locator("html")).toHaveAttribute("data-resolved-theme", theme);
      await page.getByRole("button", { name: "List", exact: true }).click();
      await expect(page.locator("#spec-work-items article")).toHaveCount(6);
      await expect(page.locator("#spec-work-items .work-card-fact")).toHaveCount(18);
      if (width === 390) await expect(page.locator(".sidebar-specs")).toBeHidden();
      const firstCard = page.locator("#spec-work-items article").first();
      expect((await firstCard.boundingBox()).height).toBeLessThan(width === 1440 ? 220 : 320);
      const samples = await page.locator('.work-card-note, .work-card-fact b, .spec-view-switch [aria-pressed="true"], .spec-overview-heading > button').evaluateAll(nodes => {
        const lum = color => {
          const c = color.match(/[\d.]+/g).slice(0,3).map(Number).map(x=>x/255).map(x=>x<=.04045?x/12.92:((x+.055)/1.055)**2.4);
          return .2126*c[0]+.7152*c[1]+.0722*c[2];
        };
        return nodes.map(node=>{
          let parent=node,bg;
          while(parent){bg=getComputedStyle(parent).backgroundColor;if(bg!=="rgba(0, 0, 0, 0)"&&bg!=="transparent")break;parent=parent.parentElement;}
          const a=lum(getComputedStyle(node).color),b=lum(bg);
          return {ratio:(Math.max(a,b)+.05)/(Math.min(a,b)+.05),text:node.textContent};
        });
      });
      samples.forEach(sample=>expect(sample.ratio,`${theme}: ${sample.text}`).toBeGreaterThanOrEqual(4.5));
      expect(await page.evaluate(()=>document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
      await page.screenshot({path:test.info().outputPath(`work-list-${theme}-${width}.png`)});
      await page.getByRole("button", { name: "Board", exact: true }).click();
      await expect(page.locator("#spec-work-items article")).toHaveCount(6);
      expect(await page.evaluate(()=>document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
      await page.screenshot({path:test.info().outputPath(`work-board-${theme}-${width}.png`)});
    }
  }
});
