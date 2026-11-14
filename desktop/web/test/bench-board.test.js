import test from "node:test";
import assert from "node:assert/strict";
import { boardRows, specSignals, stageAudit, stageSummary } from "../src/bench-board.js";

test("Board prioritizes observed attention and orders other rows by audit or evidence", () => {
  const items = ["recent-audit", "recent-proof", "attention", "empty"].map(id => ({ id }));
  const specs = [
    { id: "001", work_item: "recent-audit", stage: "ship", audit: [{ at: "2026-11-11" }] },
    { id: "002", work_item: "recent-proof", stage: "build" },
    { id: "003", work_item: "attention", stage: "plan" },
    { id: "004", stage: "review" },
  ];
  const summaryFor = spec => spec.id === "002" ? { sdlc: { activityAt: "2026-11-12" } } : null;
  const { rows, unlinked } = boardRows(items, specs, [{ workItemId: "attention" }], summaryFor);
  assert.deepEqual(rows.map(row => [row.item.id, row.state]), [
    ["attention", "In progress"], ["recent-proof", "In progress"],
    ["recent-audit", "Complete"], ["empty", "Not started"],
  ]);
  assert.equal(rows[1].counts.build, 1);
  assert.deepEqual(unlinked.map(spec => spec.id), ["004"]);
});

test("Needs you uses current observed failures, stale recorded proof and the latest gate", () => {
  const spec = { id: "001", title: "Sample", audit: [{ action: "stage review → ship (warning: missing proof)" }] };
  const summary = { sdlc: { criteria: [
    { index: 0, observedCheck: { result: "failed", freshness: "current" } },
    { index: 1, recorded: true, freshness: "stale" },
    { index: 2, observedCheck: { result: "failed", freshness: "stale" } },
    { index: 3, freshness: "stale" },
  ] } };
  assert.deepEqual(specSignals(spec, summary).map(signal => signal.label), [
    "Failed check", "Stale proof", "Recorded gate warning",
  ]);
  spec.audit.push({ action: "stage ship → spec" });
  summary.sdlc.criteria = [{ observedCheck: { result: "passed", freshness: "current" } }];
  assert.deepEqual(specSignals(spec, summary), []);
});

test("stage audit preserves structured warnings and formats old joined warnings", () => {
  const move = stageAudit('stage review → ship (warnings: ["A warning; with detail.","Second warning."]) — Owner note');
  assert.deepEqual(move, { stage: "ship", warnings: ["A warning; with detail.", "Second warning."], note: "Owner note" });
  assert.equal(stageSummary(move.stage, move.warnings.length), "Moved to Ship with 2 warnings");
  const legacy = stageAudit("stage review → ship (warning: One warning.) — owner note; SDLC readiness warnings: Second warning.; Third warning.");
  assert.deepEqual(legacy, { stage: "ship", warnings: ["One warning.", "Second warning.", "Third warning."], note: "owner note" });
  const olderGitWarning = stageAudit("stage review → ship (warning: Git readiness could not be established; release status is unknown; 2 uncommitted changes in the project)");
  assert.equal(olderGitWarning.warnings.length, 2);
});
