// What a person sees when they open Verb, how fast, and whether the host ever turns them away.
import { BUDGET, READS } from "./budget.js";
import { expect, open, record, test, verdict } from "./support.js";

test("opening Verb shows the project within budget and never says the host is busy", async ({ page }) => {
  const busy = [];
  page.on("response", (response) => { if (response.status() === 503) busy.push(new URL(response.url()).pathname); });
  const started = await open(page);
  const branch = page.locator("#sidebar-branch");
  await expect(branch).not.toHaveText(/^(—|Reading Git status…)?$/, { timeout: BUDGET.allPanels });
  const firstUseful = Date.now() - started;
  record("open", "first useful page", firstUseful, "ms", verdict(firstUseful, 2_500, BUDGET.firstUseful));
  await expect(page.locator("#bench-work-tree")).not.toContainText("Loading work items", { timeout: BUDGET.allPanels });
  const allPanels = Date.now() - started;
  record("open", "every panel filled", allPanels, "ms", verdict(allPanels, 5_000, BUDGET.allPanels));
  await expect(page.getByText("desktop host is busy")).toHaveCount(0);
  record("open", "busy answers seen", busy.length, "count", busy.length ? "info" : "pass");
  expect(firstUseful, "first useful page").toBeLessThanOrEqual(BUDGET.firstUseful);
  expect(allPanels, "every panel filled").toBeLessThanOrEqual(BUDGET.allPanels);
});

test("every background read meets the read budget", async ({ page }) => {
  await open(page);
  await expect(page.locator("#bench-work-tree")).not.toContainText("Loading work items", { timeout: BUDGET.allPanels });
  const failures = [];
  for (const path of READS) {
    const times = [];
    for (let round = 0; round < 3; round++) {
      // Reads go out the way the page sends them: its own stored key (if any) and its cookie.
      const result = await page.evaluate(async (path) => {
        const key = sessionStorage.getItem("verb-web-token");
        const started = performance.now();
        const response = await fetch(path, { cache: "no-store", credentials: "same-origin", headers: key ? { "X-Verb-Token": key } : {} });
        const ok = response.ok;
        await response.text();
        return { ok, status: response.status, ms: performance.now() - started };
      }, path);
      if (!result.ok) failures.push(`${path} answered ${result.status}`);
      times.push(result.ms);
    }
    const median = times.sort((a, b) => a - b)[1];
    const v = verdict(median, BUDGET.readTarget, BUDGET.readLimit);
    record("reads", `${path} median of 3`, median, "ms", v);
    if (v === "fail") failures.push(`${path} median ${Math.round(median)} ms is over ${BUDGET.readLimit} ms`);
  }
  expect(failures).toEqual([]);
});

test("a reload shows the last known project at once", async ({ page }) => {
  await open(page);
  const branch = page.locator("#sidebar-branch");
  await expect(branch).not.toHaveText(/^(—|Reading Git status…)?$/, { timeout: BUDGET.allPanels });
  await expect(page.locator("#bench-work-tree")).not.toContainText("Loading work items", { timeout: BUDGET.allPanels });
  const known = await branch.textContent();
  await page.reload({ waitUntil: "domcontentloaded" });
  const started = Date.now();
  await expect(branch).toHaveText(known, { timeout: BUDGET.reloadRemembered });
  record("reload", "last known branch shown", Date.now() - started, "ms", "pass");
  await expect(page.getByText("Reading Git status")).toHaveCount(0);
});

test("Plan, Review and Preview open without an error", async ({ page }) => {
  await open(page);
  await expect(page.locator("#bench-work-tree")).not.toContainText("Loading work items", { timeout: BUDGET.allPanels });
  for (const tab of ["plan", "review", "preview"]) {
    const button = page.locator(`#bench-tab-${tab}`);
    if (!(await button.isVisible())) continue;
    await button.click();
    const panel = page.locator(`#bench-${tab}`);
    await expect(panel).toBeVisible();
    await expect(panel.locator('[role="alert"]:visible')).toHaveCount(0, { timeout: 15_000 });
    await expect(panel).not.toContainText(/host is busy|Could not read/i, { timeout: 15_000 });
  }
});
