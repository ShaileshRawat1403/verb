// What a person sees when they open Verb, how fast, and whether the host ever turns them away.
import { BUDGET, PANEL_READS, READS } from "./budget.js";
import { parseServerTiming, readVerdict, summarize } from "./stats.js";
import { expect, open, record, test, verdict } from "./support.js";

const FIRST_USEFUL = ["sidebar branch", "work tree"];
// What the sidebar shows before it knows the branch.
const PLACEHOLDER = /^\s*(—|Reading\b.*)?\s*$/;
// What the sidebar shows when state answered without a branch. Not a branch: counted on its own.
const NOT_BRANCH = /^\s*(Busy\b.*|Git status unavailable)\s*$/;

// Waits for a real branch name in the sidebar. Returns when it appeared (ms from `started`, or
// undefined after `timeout`), the text, and when "busy" or "unavailable" was first shown instead.
async function waitForBranch(locator, started, timeout) {
  const deadline = Date.now() + timeout;
  const shown = {};
  for (;;) {
    const text = ((await locator.textContent().catch(() => null)) ?? "").trim();
    const other = text.match(NOT_BRANCH);
    if (other) shown[/^Busy/.test(text) ? "busy" : "unavailable"] ??= Date.now() - started;
    else if (!PLACEHOLDER.test(text)) return { at: Date.now() - started, text, shown };
    if (Date.now() >= deadline) return { at: undefined, text, shown };
    await new Promise(resolve => setTimeout(resolve, 50));
  }
}

// One opening in a fresh browser (nothing remembered): when each panel filled, and any busy answer.
async function openOnce(page) {
  const busy = [];
  // Each panel's own read, timed from the navigation: the first successful answer fills the panel.
  const panelAt = {};
  let started = 0;
  // When the page's own /api/state read answered: the sidebar's source besides /api/git.
  let stateAt;
  page.on("response", (response) => {
    const path = new URL(response.url()).pathname;
    if (response.status() === 503) busy.push(path);
    if (path === "/api/state" && response.ok() && stateAt === undefined) stateAt = Date.now() - started;
    for (const [panel, read] of Object.entries(PANEL_READS)) {
      if (path === read && response.ok() && panelAt[panel] === undefined) panelAt[panel] = Date.now() - started;
    }
  });
  started = Date.now();
  await open(page);
  let sidebar = { shown: {} };
  const branchWait = waitForBranch(page.locator("#sidebar-branch"), started, BUDGET.allPanels).then((result) => {
    sidebar = result;
    if (result.at !== undefined) panelAt["sidebar branch"] = result.at;
  });
  const timed = async (panel, wait) => {
    try {
      await wait;
      panelAt[panel] = Date.now() - started;
    } catch {
      // Recorded below as not filled within the limit.
    }
  };
  await Promise.all([
    branchWait,
    timed("work tree", expect(page.locator("#bench-work-tree")).not.toContainText("Loading work items", { timeout: BUDGET.allPanels })),
    expect.poll(() => Object.keys(PANEL_READS).every(panel => panelAt[panel] !== undefined), { timeout: BUDGET.allPanels }).toBe(true).catch(() => {}),
  ]);
  await expect(page.getByText("desktop host is busy")).toHaveCount(0);
  return { panelAt, busy, stateAt, sidebarShown: sidebar.shown };
}

// Opening is timed three times, each in a fresh browser, because one opening is one sample. The first
// one may meet a cold host (caches empty after a minute idle); it is reported as "cold" but the
// verdict uses the median of the three, the same rule as the reads.
const OPENINGS = 3;
// How long the host reuses an /api/state answer (web.rs: cached_at.elapsed() < 1500 ms).
const STATE_CACHE_MS = 1_500;

test("opening Verb shows the project within budget and never says the host is busy", async ({ browser }) => {
  test.setTimeout(OPENINGS * (BUDGET.allPanels + 15_000));
  const runs = [];
  for (let i = 0; i < OPENINGS; i++) {
    const context = await browser.newContext();
    try {
      runs.push(await openOnce(await context.newPage()));
    } finally {
      await context.close();
    }
  }
  const panels = ["sidebar branch", "work tree", ...Object.keys(PANEL_READS)];
  const at = (run, list) => (list.every(panel => run.panelAt[panel] !== undefined) ? Math.max(...list.map(panel => run.panelAt[panel])) : Infinity);
  const late = [];
  const check = (name, values, target, limit) => {
    const median = [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
    const v = verdict(median, target, limit);
    const shown = Number.isFinite(median) ? median : limit;
    record("open", name, shown, "ms", v, { median: shown, cold: Number.isFinite(values[0]) ? values[0] : null, runs: values.map(x => (Number.isFinite(x) ? x : null)) });
    if (v === "fail") late.push(`${name}: median ${Number.isFinite(median) ? `${Math.round(median)} ms` : "not filled"} (limit ${limit} ms)`);
  };
  for (const panel of panels) {
    const first = FIRST_USEFUL.includes(panel);
    check(`${panel} filled`, runs.map(run => at(run, [panel])), first ? 2_500 : 5_000, first ? BUDGET.firstUseful : BUDGET.allPanels);
  }
  check("first useful page", runs.map(run => at(run, FIRST_USEFUL)), 2_500, BUDGET.firstUseful);
  check("every panel filled", runs.map(run => at(run, panels)), 5_000, BUDGET.allPanels);
  // When state answered, for reading the sidebar's time against it. Information, not a verdict.
  const stateRuns = runs.map(run => run.stateAt ?? Infinity);
  const stateMedian = [...stateRuns].sort((a, b) => a - b)[Math.floor(stateRuns.length / 2)];
  record("open", "/api/state answered", Number.isFinite(stateMedian) ? stateMedian : BUDGET.allPanels, "ms", "info", {
    median: Number.isFinite(stateMedian) ? stateMedian : null,
    runs: stateRuns.map(x => (Number.isFinite(x) ? x : null)),
  });
  // "Busy—retrying" or "Git status unavailable" in the sidebar: shown, but not a branch.
  const notBranch = runs.filter(run => Object.keys(run.sidebarShown).length).length;
  record("open", "sidebar showed busy or unavailable", notBranch, "count", notBranch ? "info" : "pass", {
    runs: runs.map(run => run.sidebarShown),
  });
  const busy = runs.reduce((sum, run) => sum + run.busy.length, 0);
  record("open", "busy answers seen", busy, "count", busy ? "info" : "pass");
  // A failure names the slow panel.
  expect(late, "slow panels").toEqual([]);
});

test("every background read meets the read budget", async ({ page }) => {
  await open(page);
  await expect(page.locator("#bench-work-tree")).not.toContainText("Loading work items", { timeout: BUDGET.allPanels });
  const failures = [];
  for (const path of READS) {
    const samples = [];
    // The host keeps /api/state for 1.5 s, so the first sample is uncached only after a pause.
    if (path === "/api/state") await page.waitForTimeout(STATE_CACHE_MS + 100);
    for (let round = 0; round < BUDGET.readSamples; round++) {
      // Reads go out the way the page sends them: its own stored key (if any) and its cookie.
      const result = await page.evaluate(async (path) => {
        const key = sessionStorage.getItem("verb-web-token");
        const started = performance.now();
        const response = await fetch(path, { cache: "no-store", credentials: "same-origin", headers: key ? { "X-Verb-Token": key } : {} });
        const ok = response.ok;
        await response.text();
        return { ok, status: response.status, ms: performance.now() - started, timing: response.headers.get("server-timing") };
      }, path);
      if (!result.ok) failures.push(`${path} answered ${result.status}`);
      samples.push(result);
    }
    if (path === "/api/state") {
      // Every later sample is a cache hit; the first is what an opening page waits for.
      const first = samples[0];
      const phases = parseServerTiming(first.timing);
      record("reads", "/api/state first sample (uncached)", first.ms, "ms", "info", {
        phases,
        uncached: (phases?.boundary?.entries ?? 0) > 0,
      });
    }
    const stats = summarize(samples.map(sample => sample.ms));
    const medianSample = [...samples].sort((a, b) => a.ms - b.ms)[Math.ceil(samples.length / 2) - 1];
    const v = readVerdict(stats);
    record("reads", `${path} median of ${BUDGET.readSamples}`, stats.median, "ms", v, {
      samples: samples.length,
      median: Math.round(stats.median),
      p75: Math.round(stats.p75),
      p90: Math.round(stats.p90),
      phases: parseServerTiming(medianSample.timing),
    });
    if (v === "fail") {
      failures.push(`${path} p75 ${Math.round(stats.p75)} ms (limit ${BUDGET.readLimit}), p90 ${Math.round(stats.p90)} ms (limit ${BUDGET.readTailLimit})`);
    }
  }
  expect(failures).toEqual([]);
});

test("a reload shows the last known project at once", async ({ page }) => {
  await open(page);
  const branch = page.locator("#sidebar-branch");
  await expect(page.locator("#bench-work-tree")).not.toContainText("Loading work items", { timeout: BUDGET.allPanels });
  // The sidebar passes through "Reading local state…" and then "Reading Git status…" before the
  // branch; wait for the branch itself, checked last so nothing can replace it in between.
  const { text: known } = await waitForBranch(branch, Date.now(), BUDGET.allPanels);
  expect(known, "a real branch name").not.toMatch(PLACEHOLDER);
  expect(known, "a real branch name").not.toMatch(NOT_BRANCH);
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
