import { test } from "node:test";
import assert from "node:assert/strict";
import {
  STAGES,
  STAGE_GUIDE,
  changeLabel,
  commitPrefix,
  filterCommands,
  groupByStage,
  matchScore,
  progressLabel,
  skippedStages,
  stageWarnings,
} from "../src/workbench.js";

const spec = (stage, done = 0, total = 3) => ({
  id: "012",
  stage,
  criteriaDone: done,
  criteria: Array.from({ length: total }, (_, i) => ({ index: i, done: i < done })),
});

test("every stage has guidance", () => {
  for (const stage of STAGES) assert.ok(STAGE_GUIDE[stage].next.length > 10, stage);
});

test("stage warnings match the server's rules", () => {
  assert.deepEqual(stageWarnings(spec("spec"), "plan"), []);
  assert.deepEqual(stageWarnings(spec("spec"), "build"), ["skipped plan"]);
  assert.deepEqual(stageWarnings(spec("verify", 1), "review"), [
    "2 of 3 acceptance criteria not yet proven",
  ]);
  assert.deepEqual(stageWarnings(spec("verify", 3), "review"), []);
  assert.deepEqual(stageWarnings(spec("build"), "build"), []);
  assert.deepEqual(stageWarnings(spec("review"), "plan"), [], "moving back never warns");
});

test("progress and grouping", () => {
  assert.equal(progressLabel(spec("build", 1)), "1/3 proven");
  assert.equal(progressLabel(spec("build", 0, 0)), "no criteria yet");
  const groups = groupByStage([spec("build"), spec("spec"), spec("build")]);
  assert.deepEqual(groups.map((g) => [g.stage, g.specs.length]), [["spec", 1], ["build", 2]]);
});

test("git codes read as words", () => {
  assert.equal(changeLabel("M"), "Modified");
  assert.equal(changeLabel("??"), "New file");
  assert.equal(changeLabel("AM"), "Added");
  assert.equal(changeLabel(" D"), "Deleted");
});

test("palette search matches in order and prefers word starts", () => {
  assert.equal(matchScore("New terminal", "xyz"), -1);
  assert.ok(matchScore("New terminal", "nt") > matchScore("Commit changes", "nt"));
  const commands = [
    { title: "Commit changes" },
    { title: "New spec" },
    { title: "New terminal" },
  ];
  assert.equal(filterCommands(commands, "ns")[0].title, "New spec");
  assert.equal(filterCommands(commands, "").length, 3);
});

test("commit prefix", () => {
  assert.equal(commitPrefix({ id: "007" }), "spec:007 ");
  assert.equal(commitPrefix(null), "");
});

test("skipped stages come from the audit trail until they are entered", () => {
  const audit = (actions) => ({ audit: actions.map((action) => ({ action })) });
  assert.deepEqual([...skippedStages(audit(["created spec", "stage spec → build (warning: skipped plan)"]))], ["plan"]);
  assert.deepEqual(
    [...skippedStages(audit(["stage spec → verify (warning: skipped plan, build)", "stage verify → plan"]))],
    ["build"],
  );
  assert.equal(skippedStages(audit(["stage spec → plan"])).size, 0);
});
