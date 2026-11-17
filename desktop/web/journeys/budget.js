// The performance budget, from docs/PERFORMANCE_BUDGET.md. Times are milliseconds.
export const BUDGET = {
  // Reads: 10 samples each. slow when the median is over the target; fail when p75 is over the
  // limit or p90 over the tail limit.
  readSamples: 10,
  readTarget: 1_000,
  readLimit: 2_500,
  readTailLimit: 4_000,
  firstUseful: 4_000,
  allPanels: 10_000,
  reloadRemembered: 1_500,
  keystrokeEcho: 100,
};

export const READS = ["/api/state", "/api/specs", "/api/hub", "/api/work-items", "/api/git", "/api/files"];

// Panels timed separately on opening. The sidebar branch and work tree are the first useful page;
// the others are filled when the page's own read for them has answered.
export const PANEL_READS = { Plan: "/api/specs", hub: "/api/hub", Git: "/api/git" };
