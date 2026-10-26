import test from "node:test";
import assert from "node:assert/strict";
import { projectGitPresentation as view } from "../src/project-git.js";
import { gitRemoteDescription } from "../src/project-git.js";

test("remote guidance distinguishes local commits, incoming work and cached comparisons", () => {
  assert.match(gitRemoteDescription({ upstream: "origin/main", ahead: 2, behind: 1 }), /2 local commits to upload; 1 remote commit to bring in/);
  const matched = gitRemoteDescription({ upstream: "origin/main", ahead: 0, behind: 0 });
  assert.match(matched, /last known remote state/);
  assert.match(matched, /refresh does not check the server/);
  assert.match(gitRemoteDescription({ upstream: null }), /No remote branch is linked/);
  assert.equal(gitRemoteDescription({ error: "failed", upstream: "origin/main" }), "Remote comparison unavailable.");
});
test("completed Git failure never reads as loading or zero changes", () => {
  assert.equal(view({ gitStatus: "loading" }).changes, "Reading changes…");
  const failure = view({ gitStatus: "unavailable", changedFiles: 0 });
  assert.equal(failure.branch, "Git status unavailable");
  assert.equal(failure.changes, "Changes unavailable");
  assert.match(failure.detail, /this account can read/);
});
test("unborn and detached checkouts show distinct facts with observed counts", () => {
  assert.equal(
    view({ gitStatus: "unborn", branch: "main", changedFiles: 1 }).branch,
    "main · no commits",
  );
  const detached = view({
    gitStatus: "detached",
    branch: null,
    changedFiles: 0,
  });
  assert.equal(detached.branch, "Detached HEAD");
  assert.equal(detached.changes, "0 changed");
});
test("non-Git and malformed observations do not become a clean branch", () => {
  assert.equal(
    view({ gitStatus: "not_repository", changedFiles: 0 }).changes,
    "No Git tracking",
  );
  assert.equal(
    view({ gitStatus: "branch", branch: "main", changedFiles: -1 }).changes,
    "Changes unavailable",
  );
  assert.equal(
    view({ gitStatus: "future_state", changedFiles: 0 }).changes,
    "Changes unavailable",
  );
});
test("older hosts remain readable without inventing detached identity", () => {
  assert.equal(
    view({ branch: "legacy", changedFiles: 2 }).changes,
    "2 changed",
  );
  assert.equal(
    view({ branch: null, changedFiles: 0 }).branch,
    "Git status unavailable",
  );
});
test("a full execution slot reads as busy and retrying, never as a Git failure", () => {
  const busy = view({ gitStatus: "busy", changedFiles: null });
  assert.equal(busy.branch, "Busy—retrying");
  assert.match(busy.detail, /retries automatically/);
  assert.notEqual(busy.changes, "0 changed");
});
