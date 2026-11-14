// Git status compares local records; it does not contact the remote server.
export function gitRemoteDescription(git) {
  if (!git || git.error) return "Remote comparison unavailable.";
  if (!git.upstream)
    return "No remote branch is linked to this branch. A local commit does not upload your work.";
  const parts = [];
  if (git.ahead) parts.push(`${git.ahead} local commit${git.ahead === 1 ? "" : "s"} to upload`);
  if (git.behind) parts.push(`${git.behind} remote commit${git.behind === 1 ? "" : "s"} to bring in`);
  return `${git.upstream}: ${parts.join("; ") || "matches the last known remote state"}. Based on local records; refresh does not check the server.`;
}

// Completed failures and initial loading are different observations.
export function projectGitPresentation(project) {
  const states = {
    loading: ["Reading Git status…", "Reading the current checkout."],
    unborn: [
      `${project.branch || "New repository"} · no commits`,
      "This repository has no commits yet. Create its first commit when ready.",
    ],
    detached: [
      "Detached HEAD",
      "No branch is checked out. Switch to a branch in your terminal before adding work you want to keep on a branch.",
    ],
    not_repository: [
      "Not a Git repository",
      "Open a Git checkout, or initialize this folder from your terminal.",
    ],
    busy: [
      "Busy—retrying",
      "Background checks are waiting for a free execution slot. Verb retries automatically; terminals are unaffected.",
    ],
    unavailable: [
      "Git status unavailable",
      "Check that Git is installed and this account can read the checkout. Terminal work remains available.",
    ],
  };
  const state =
    project.gitStatus ||
    (project.branch
      ? "branch"
      : project.changedFiles == null
        ? "loading"
        : "unavailable");
  const [branch, detail] =
    states[state] ||
    (state === "branch" && project.branch
      ? [project.branch, "Current Git branch."]
      : states.unavailable);
  const count =
    ["branch", "unborn", "detached"].includes(state) &&
    Number.isInteger(project.changedFiles) &&
    project.changedFiles >= 0
      ? project.changedFiles
      : null;
  const changes =
    state === "loading"
      ? "Reading changes…"
      : count != null
        ? `${count} changed`
        : state === "not_repository"
          ? "No Git tracking"
          : "Changes unavailable";
  return { branch, detail, changes };
}
