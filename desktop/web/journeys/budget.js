// The performance budget, from docs/PERFORMANCE_BUDGET.md. Times are milliseconds.
export const BUDGET = {
  readTarget: 1_000,
  readLimit: 2_500,
  firstUseful: 4_000,
  allPanels: 10_000,
  reloadRemembered: 1_500,
  keystrokeEcho: 100,
};

export const READS = ["/api/state", "/api/specs", "/api/hub", "/api/work-items", "/api/git", "/api/files"];
