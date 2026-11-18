// The sidebar branch comes from /api/state, which on a node makes more than one boundary crossing.
// /api/git (one crossing) usually answers first and already names the branch, so it fills the
// sidebar while state is still loading. State's own answer replaces it whenever it arrives.

// A branch name as /api/git reports it, or null when it names no branch (detached HEAD reads as
// "HEAD (no branch)"; Git ref names never contain spaces).
function gitBranchName(git) {
  if (!git || git.error) return null;
  const name = typeof git.branch === "string" ? git.branch.trim() : "";
  return name && name !== "HEAD" && !/\s/.test(name) ? name : null;
}

// The project with its branch filled from an /api/git answer, or the same object when there is
// nothing to fill: state has already answered (any status but "loading"), or Git named no branch.
export function projectWithGitBranch(project, git) {
  if (!project || project.gitStatus !== "loading") return project;
  const branch = gitBranchName(git);
  if (!branch) return project;
  return {
    ...project,
    branch,
    gitStatus: "branch",
    changedFiles: Array.isArray(git.changes) ? git.changes.length : null,
  };
}

// After a state read that failed, or that answered without Git facts yet ("loading" or "busy"),
// read again soon rather than waiting for the 30 s poll. Then back to the poll.
export const STATE_RETRY_DELAYS = [1000, 2000, 4000];

export function stateNeedsRetry(state) {
  return !state || ["loading", "busy"].includes(state.project?.gitStatus);
}

// `run` reads state again. `outcome(state)` is called after each read: the state on success,
// undefined on failure. A good answer resets the backoff.
export function createStateRetry(run, {
  delays = STATE_RETRY_DELAYS,
  setTimer = (fn, ms) => setTimeout(fn, ms),
  clearTimer = (id) => clearTimeout(id),
} = {}) {
  let attempt = 0;
  let timer = null;
  return function outcome(state) {
    if (timer !== null) {
      clearTimer(timer);
      timer = null;
    }
    if (!stateNeedsRetry(state)) {
      attempt = 0;
      return null;
    }
    if (attempt >= delays.length) return null;
    const delay = delays[attempt++];
    timer = setTimer(() => {
      timer = null;
      run();
    }, delay);
    return delay;
  };
}
