import test from "node:test";
import assert from "node:assert/strict";
import { createStateRetry, projectWithGitBranch, stateNeedsRetry } from "../src/sidebar-branch.js";
import { projectGitPresentation } from "../src/project-git.js";

const loading = { id: "p1", name: "verb", branch: null, gitStatus: "loading", changedFiles: null };

test("the sidebar branch fills from /api/git while state is still loading", () => {
  const filled = projectWithGitBranch(loading, { branch: "spec/002-goatee", changes: [{ path: "a" }, { path: "b" }] });
  assert.deepEqual(filled, { ...loading, branch: "spec/002-goatee", gitStatus: "branch", changedFiles: 2 });
  assert.equal(projectGitPresentation(loading).branch, "Reading Git status…");
  assert.equal(projectGitPresentation(filled).branch, "spec/002-goatee");
  assert.equal(projectGitPresentation(filled).changes, "2 changed");
});

test("state's own answer wins: /api/git never replaces a branch state has given", () => {
  for (const gitStatus of ["branch", "unborn", "detached", "busy", "unavailable", "not_repository"]) {
    const project = { ...loading, gitStatus, branch: gitStatus === "branch" ? "main" : null };
    assert.equal(projectWithGitBranch(project, { branch: "other" }), project, gitStatus);
  }
});

test("a Git answer that names no branch leaves the sidebar loading", () => {
  for (const git of [null, undefined, { error: "busy" }, { branch: null }, { branch: "" }, { branch: "HEAD (no branch)" }, { branch: "HEAD" }]) {
    assert.equal(projectWithGitBranch(loading, git), loading, JSON.stringify(git));
  }
  assert.equal(projectWithGitBranch(undefined, { branch: "main" }), undefined);
  assert.equal(projectWithGitBranch(loading, { branch: "main" }).changedFiles, null);
});

test("a failed state read, or one without Git facts yet, needs another read", () => {
  assert.equal(stateNeedsRetry(undefined), true);
  assert.equal(stateNeedsRetry({ project: { gitStatus: "loading" } }), true);
  assert.equal(stateNeedsRetry({ project: { gitStatus: "busy" } }), true);
  assert.equal(stateNeedsRetry({ project: { gitStatus: "branch" } }), false);
  assert.equal(stateNeedsRetry({ project: { gitStatus: "unavailable" } }), false);
});

function fakeTimers() {
  const pending = new Map();
  let next = 1;
  return {
    pending,
    setTimer: (fn, ms) => { pending.set(next, { fn, ms }); return next++; },
    clearTimer: id => pending.delete(id),
    fire() { const [[id, { fn }]] = pending; pending.delete(id); fn(); },
  };
}

test("state is read again after 1 s, 2 s and 4 s, then left to the 30 s poll", () => {
  const timers = fakeTimers();
  let runs = 0;
  const outcome = createStateRetry(() => runs++, timers);
  const seen = [];
  for (let i = 0; i < 4; i++) {
    seen.push(outcome(undefined));
    if (timers.pending.size) timers.fire();
  }
  assert.deepEqual(seen, [1000, 2000, 4000, null]);
  assert.equal(runs, 3);
  assert.equal(timers.pending.size, 0);
});

test("a good state answer resets the backoff and cancels a pending retry", () => {
  const timers = fakeTimers();
  let runs = 0;
  const outcome = createStateRetry(() => runs++, timers);
  assert.equal(outcome({ project: { gitStatus: "busy" } }), 1000);
  assert.equal(outcome({ project: { gitStatus: "loading" } }), 2000);
  assert.equal(timers.pending.size, 1, "one retry pending at a time");
  assert.equal(outcome({ project: { gitStatus: "branch" } }), null);
  assert.equal(timers.pending.size, 0);
  assert.equal(runs, 0);
  assert.equal(outcome(undefined), 1000, "backoff starts over after a good answer");
});
