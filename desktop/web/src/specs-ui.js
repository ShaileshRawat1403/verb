import { stageSummary } from "./bench-board.js";
// The spec-driven workbench: specs list, stage bar, acceptance criteria with evidence, Git, audit
// trail, command palette and keyboard shortcuts. Rules live in workbench.js (tested); this file
// only talks to the DOM and the API.

import {
  STAGES,
  STAGE_GUIDE,
  suggestCommitMessage,
  commitPrefix,
  filterCommands,
  stageWarnings,
  specSessions,
  lastAgent,
} from "./workbench.js";
import { agentDisplayName } from "./view.js";
import { icon } from "./icons.js";
import { gitRemoteDescription } from "./project-git.js";
import { specPresentation } from "./spec-summary.js";
import { initSdlc } from "./sdlc-ui.js";
import { readWithBusyRetry } from "./read-retry.js";

const SHORTCUTS = [
  ["⌘K / Ctrl+K", "Command palette: search specs and run any action"],
  ["Alt+N", "New spec"],
  ["Alt+T", "New terminal"],
  ["Alt+C", "Commit changes"],
  ["Alt+1 … Alt+7", "Board, Files, Terminals, Review, Tasks, Context, Host"],
  ["Alt+A", "Ask Verb about this project"],
  ["?", "This list"],
  ["Esc", "Close a dialog"],
];
const VIEWS = ["bench-board", "project", "sessions", "overview", "tasks", "context", "host"];

export function initWorkbench(deps) {
  const { api, toast, showDialog, closeDialog, escapeHtml, $ } = deps;
  const state = {
    specs: [],
    git: null,
    selectedId: readSelected(),
    pending: null, // what an open stage/evidence dialog will do on submit
    meters: {}, // session id -> context meter, for running Claude/Codex sessions
    summaries: {},
    specsError: null,
    specsLoaded: false,
    palette: [],
    paletteIndex: 0,
  };

  function readSelected() {
    try {
      return localStorage.getItem("verb.selectedSpec");
    } catch {
      return null;
    }
  }
  function remember(id) {
    try {
      localStorage.setItem("verb.selectedSpec", id ?? "");
    } catch {
      // Selection is a convenience; nothing depends on it surviving.
    }
  }
  const selected = () => state.specs.find((spec) => spec.id === state.selectedId) ?? null;
  const lifecycle = initSdlc({ api, $, escapeHtml, toast, showDialog, closeDialog, current: selected,
    replaceSpec, openCommit, changed: () => Promise.all([refreshSpecs(), deps.refreshState(true)]) });

  // ------------------------------------------------------------------------------- data

  let specsRequest = 0;
  async function refreshSpecs() {
    const request = ++specsRequest;
    try {
      const result = await readWithBusyRetry(api, "/api/specs");
      if (request !== specsRequest) return;
      state.specs = (result.specs || []).map(normalize);
      state.summaries = result.summaries || {};
      state.specsError = null;
      state.specsLoaded = true;
      if (!selected() && state.specs.length) state.selectedId = state.specs.at(-1).id;
      if ($("#palette-dialog").open) renderPalette();
      render();
    } catch (error) {
      if (request !== specsRequest) return;
      state.specsError = error.message;
      deps.extras?.onWorkbenchChange?.();
      toast(error.message, "error");
    }
  }
  // Execution-boundary congestion is retried a bounded number of times; genuine Git errors are
  // shown as they are.
  const GIT_RETRY_DELAYS = [1500, 3000, 6000];
  let gitRetry = null;
  async function refreshGit(attempt = 0) {
    clearTimeout(gitRetry);
    try {
      state.git = await api("GET", "/api/git");
    } catch (error) {
      const busy = error.code === "capacity" && attempt < GIT_RETRY_DELAYS.length;
      state.git = { error: busy ? "Busy—retrying" : error.message, busy, changes: [], recent: [] };
      if (busy) gitRetry = setTimeout(() => refreshGit(attempt + 1), GIT_RETRY_DELAYS[attempt]);
    }
    deps.extras.onWorkbenchChange?.();
  }
  const normalize = (spec) => ({ ...spec, criteriaDone: spec.criteria_done ?? spec.criteriaDone ?? 0 });
  function replaceSpec(spec) {
    const next = normalize(spec);
    state.specs = state.specs.map((s) => (s.id === next.id ? next : s));
    if (!state.specs.some((s) => s.id === next.id)) state.specs.push(next);
    render();
    refreshSpecs();
    deps.refreshState(true);
  }

  // ----------------------------------------------------------------------------- render

  function render() { deps.extras.onWorkbenchChange?.(); }

  function summaryFor(spec) {
    const fromState = deps.ui.state?.specSummaries?.[spec.id];
    return fromState?.revision === spec.revision ? fromState : state.summaries[spec.id];
  }

  function presentation(spec) {
    return specPresentation(spec, summaryFor(spec), {
      waitingTerminals: deps.ui.waitingTerminals,
      waitingTalkSpecs: deps.ui.waitingTalkSpecs,
      liveIds: new Set((deps.ui.state?.sessions || []).filter(s => s.state === "live").map(s => s.id)),
    });
  }

  async function refreshMeters() {
    const spec = selected();
    const anyAgentLive = spec && specSessions(spec, deps.ui.state?.sessions ?? []).some((x) => x.live && /^(claude|codex)$/.test(x.agent));
    if (!anyAgentLive) return;
    try {
      state.meters = (await api("GET", "/api/meters")).meters ?? {};
    } catch {
      // A missing meter is shown as nothing, never as a wrong number.
    }
  }

  function selectSpec(id, { navigate = true } = {}) {
    if (!state.specs.some(spec => spec.id === id)) return;
    state.selectedId = id;
    remember(id);
    if (navigate) deps.extras.openBenchSpec?.(id);
    render();
  }

  function workItemOptions(select, first, defaultId) {
    select.replaceChildren(new Option(first, ""), ...(deps.extras.linkableWorkItems?.() ?? [])
      .map(item => new Option(item.title, item.id)));
    select.value = [...select.options].some(option => option.value === defaultId) ? defaultId : "";
  }

  // Every criterion of a new spec meets the requirements chosen for it; the host writes the
  // Traceability rows, so the agent is told which requirement IDs its work serves.
  async function linkSpec(spec, workItem, requirements = []) {
    const criteria = requirements.length
      ? Object.fromEntries((spec.criteria ?? []).map(c => [String(c.index + 1), requirements]))
      : undefined;
    const result = await api("POST", "/api/work-items/link", {
      specId: spec.id, expectedRevision: spec.revision, workItem, ...(criteria ? { criteria } : {}),
    });
    await Promise.all([refreshSpecs(), deps.extras.refreshBenchWork?.()]);
    return result;
  }

  let linkTarget;
  function openLinkSpec(spec) {
    linkTarget = spec;
    workItemOptions($("#spec-link-work-item"), "Choose a published work item");
    showDialog("spec-link-dialog");
  }

  $("#spec-link-form").addEventListener("submit", async event => {
    event.preventDefault();
    const form = event.currentTarget;
    const submit = form.querySelector('[type="submit"]');
    submit.disabled = true;
    try {
      const result = await linkSpec(linkTarget, form.elements.workItem.value);
      closeDialog("spec-link-dialog");
      toast(result.message ?? "Spec linked to work item.");
    } catch (error) {
      toast(error.message, "error");
    } finally {
      submit.disabled = false;
    }
  });

  function renderRequirementOptions() {
    const id = $("#spec-work-item").value;
    const item = (deps.extras.linkableWorkItems?.() ?? []).find(row => row.id === id);
    const requirements = item?.requirements ?? [];
    const box = $("#spec-requirements");
    box.hidden = !requirements.length;
    box.querySelector("[data-requirement-options]").replaceChildren(...requirements.map(r => {
      const label = document.createElement("label");
      label.className = "spec-requirement";
      const input = Object.assign(document.createElement("input"), { type: "checkbox", name: "requirement", value: r.id });
      // One requirement is the common small work item; it is chosen for you and can be cleared.
      input.checked = requirements.length === 1;
      const text = document.createElement("span");
      text.textContent = `${r.id} · ${r.text}`;
      label.append(input, text);
      return label;
    }));
  }
  $("#spec-work-item").addEventListener("change", renderRequirementOptions);

  function openNewSpec(draft = {}) {
    state.newSpecView = deps.ui.view;
    $("#spec-dialog-form").reset();
    workItemOptions($("#spec-work-item"), "Unlinked", draft.workItem ?? deps.extras.selectedWorkItem?.());
    renderRequirementOptions();
    $("#spec-title-input").value = draft.title ?? "";
    $("#spec-dialog-form").elements.problem.value = draft.problem ?? "";
    showDialog("spec-dialog");
    $("#spec-title-input").focus();
  }

  async function openStage(stage) {
    const spec = selected();
    if (!spec || stage === spec.stage) return;
    // The server's list is the one the move records (it also knows git: uncommitted work, an
    // unmerged branch); the local rules are the fallback if it cannot be asked.
    let warnings = stageWarnings(spec, stage);
    let context = null;
    try {
      const result = await api("GET", `/api/specs/${spec.id}/stage-check?to=${encodeURIComponent(stage)}`);
      warnings = result.warnings; context = result.context;
    } catch {
      warnings.push("Readiness checks are unavailable. Changing stage does not confirm verification or release.");
    }
    if (selected()?.id !== spec.id || selected()?.revision !== spec.revision) {
      toast("The selected spec changed. Review its current state before moving it.", "error");
      return;
    }
    state.pending = { kind: "stage", stage, specId: spec.id, revision: spec.revision, context };
    $("#stage-dialog-title").textContent = `Move to ${STAGE_GUIDE[stage].label}`;
    $("#stage-dialog-sub").textContent = `${STAGE_GUIDE[spec.stage].label} → ${STAGE_GUIDE[stage].label}. ${STAGE_GUIDE[stage].next}`;
    $("#stage-warnings").hidden = !warnings.length;
    $("#stage-warnings").innerHTML = warnings.length
      ? `<b>You can continue, and this will be recorded:</b><ul>${warnings.map((w) => `<li>${escapeHtml(w)}</li>`).join("")}</ul>`
      : "";
    $("#stage-note").value = "";
    $("#stage-dialog-submit").textContent = `Move to ${STAGE_GUIDE[stage].label}`;
    showDialog("stage-dialog");
  }

  let commitReview = null, commitGeneration = 0, commitRunning = false, commitSpec = null, commitDiffGeneration = 0;
  function updateCommitSelection() {
    const count = $("#commit-files").querySelectorAll("input:checked").length;
    if (commitMessageAuto && commitReview && !commitReview.recovery) {
      // The suggestion names what will actually be committed: the selected files of the fresh
      // snapshot. The cached listing only supplies whether a file is new, changed or removed.
      const known = new Map((state.git?.changes ?? []).map(c => [c.path, c]));
      const chosen = [...$("#commit-files").querySelectorAll("input:checked")]
        .map(input => known.get(input.dataset.commitPath) ?? { path: input.dataset.commitPath, code: "M" });
      $("#commit-message").value = suggestCommitMessage(commitSpec, chosen);
    }
    $("#commit-selection-count").textContent = `${count} of ${commitReview?.files.length || 0} files selected`;
    $("#commit-dialog-submit").disabled = commitRunning || !commitReview || Boolean(commitReview.blocked) || count === 0;
    $("#commit-dialog-submit").textContent = commitRunning ? (commitReview?.recovery ? "Repairing…" : "Committing…") : "Commit";
    for (const input of $("#commit-files").querySelectorAll("input"))
      input.disabled = commitRunning || !commitReview || Boolean(commitReview.blocked) || Boolean(commitReview.unsupported?.[input.dataset.commitPath]);
    for (const id of ["commit-select-all", "commit-select-none", "commit-message"])
      $("#" + id).disabled = commitRunning || !commitReview || Boolean(commitReview.blocked);
    const recovering = Boolean(commitReview?.recovery);
    $("#commit-record").disabled = commitRunning || !commitSpec || Boolean(commitReview?.blocked);
    $("#commit-record").closest("label").hidden = recovering;
    $("#commit-message").hidden = recovering;
    $("label[for='commit-message']").hidden = recovering;
    $("#commit-dialog-submit").hidden = recovering;
    $(".commit-selection-actions").hidden = recovering;
    $("#commit-files").hidden = recovering;
    $("#commit-dialog-title").textContent = recovering ? "Review interrupted commit" : "Save these changes to history";
    $("#commit-dialog-sub").textContent = recovering
      ? "Repair reconciles Git state only. It does not run the commit again, upload work or deploy anything."
      : "A commit records a snapshot in this project's local Git history. It does not upload your work or create a backup on another server.";
    for (const button of $("#commit-dialog").querySelectorAll("[data-close]")) button.disabled = commitRunning;
  }
  for (const [id, checked] of [["all", true], ["none", false]]) $("#commit-select-" + id).addEventListener("click", () => {
    if (!commitReview || commitReview.blocked || commitRunning) return;
    for (const input of $("#commit-files").querySelectorAll("input:not(:disabled)")) input.checked = checked;
    updateCommitSelection();
  });
  $("#commit-dialog").addEventListener("cancel", event => { if (commitRunning) event.preventDefault(); });
  $("#commit-files").addEventListener("change", updateCommitSelection);
  $("#commit-files").addEventListener("click", async (event) => {
    const button = event.target.closest("[data-commit-diff]");
    if (!button) return;
    const generation = commitGeneration, diffGeneration = ++commitDiffGeneration;
    $("#commit-diff-panel").hidden = false;
    $("#commit-diff-title").textContent = button.dataset.commitDiff;
    $("#commit-diff-text").textContent = "Reading changes…";
    try {
      const result = await api("GET", `/api/git/diff?path=${encodeURIComponent(button.dataset.commitDiff)}`);
      if (generation !== commitGeneration || diffGeneration !== commitDiffGeneration) return;
      $("#commit-diff-text").textContent = result.reason || result.hunks?.map(h => `${h.header}\n${h.lines.map(l => `${l.kind === "add" ? "+" : l.kind === "del" ? "-" : " "}${l.text}`).join("\n")}`).join("\n") || "No text diff available; open binary files in Preview.";
    } catch (error) { if (generation === commitGeneration && diffGeneration === commitDiffGeneration) $("#commit-diff-text").textContent = error.message; }
  });
  // True until the person types in the message box; until then the suggestion follows the
  // selection, and after that their words are never replaced.
  let commitMessageAuto = true;
  $("#commit-message").addEventListener("input", () => { commitMessageAuto = false; });
  async function openCommit() {
    if (commitRunning) return;
    commitMessageAuto = true;
    const spec = selected();
    commitSpec = spec;
    const changes = state.git?.changes ?? [];
    const generation = ++commitGeneration;
    commitReview = null;
    $("#commit-files").replaceChildren();
    $("#commit-diff-panel").hidden = true;
    $("#commit-repair").hidden = true;
    $("#commit-repair").textContent = "Repair interrupted Git operation";
    $("#commit-scope").textContent = "Reading a fresh snapshot. No files are staged by opening this review.";
    updateCommitSelection();
    // A suggestion from what changed, selected so typing replaces it; the prefix stays.
    const prefix = commitPrefix(spec);
    $("#commit-message").value = suggestCommitMessage(spec, changes);
    const onOther = spec?.branch && state.git?.branch && state.git.branch !== spec.branch;
    const note = $("#commit-branch-note");
    note.hidden = !onOther;
    if (onOther)
      note.innerHTML = `You are on <code>${escapeHtml(state.git.branch)}</code>, but spec ${escapeHtml(spec.id)}'s work belongs on <code>${escapeHtml(spec.branch)}</code>. Committing here is allowed; use <b>Switch to this branch</b> first if you meant to keep it separate.`;
    $("#commit-record").checked = Boolean(spec);
    $("#commit-record").disabled = !spec;
    $("#commit-record-label").textContent = spec
      ? `Include spec ${spec.id}'s file and record this commit in its audit trail`
      : "Select a spec to record this commit in its audit trail";
    showDialog("commit-dialog");
    const box = $("#commit-message");
    box.focus();
    box.setSelectionRange(prefix.length, box.value.length);
    try {
      const review = await api("GET", "/api/git/commit-review");
      if (generation !== commitGeneration || !$("#commit-dialog").open) return;
      commitReview = review;
      $("#commit-repair").hidden = !review.recovery?.canRepair;
      $("#commit-repair").disabled = false;
      $("#commit-files").innerHTML = review.files.map(path => `<li><label class="commit-file-choice"><input type="checkbox" data-commit-path="${escapeHtml(path)}" ${review.unsupported?.[path] ? "" : "checked"} ${review.blocked || review.unsupported?.[path] ? "disabled" : ""} /><code>${escapeHtml(path)}</code>${review.unsupported?.[path] ? `<small>${escapeHtml(review.unsupported[path])}</small>` : ""}</label><button type="button" class="text-button" data-commit-diff="${escapeHtml(path)}">View diff</button></li>`).join("");
      $("#commit-scope").textContent = review.blocked || "Only selected files enter this commit; they may include work outside the selected spec. Unselected files stay as they are. Any change since this snapshot requires a new review. Existing staged work is never taken over.";
      updateCommitSelection();
    } catch (error) {
      if (generation === commitGeneration) $("#commit-scope").textContent = `Commit review unavailable: ${error.message}`;
    }
  }

  let transferReview = null;
  let transferRunning = false;
  let transferGeneration = 0;
  function paintTransfer(message, checked = false) {
    $("#git-transfer-status").textContent = message;
    const review = transferReview;
    $("#git-transfer-commits").innerHTML = review
      ? [["Upload", review.outgoing], ["Download", review.incoming]]
          .flatMap(([direction, commits]) => commits.map(commit => `<li><span class="change-kind">${direction}</span><span>${escapeHtml(commit)}</span></li>`)).join("")
      : "";
    $("#git-transfer-check").disabled = transferRunning || !review;
    $("#git-transfer-upload").disabled = transferRunning || !checked || !review || Boolean(review.blocked) || !review.ahead || Boolean(review.behind);
    $("#git-transfer-download").disabled = transferRunning || !checked || !review || Boolean(review.blocked) || !review.behind || Boolean(review.ahead);
  }
  function transferDescription(review) {
    return `${review.branch.replace(/^refs\/heads\//, "")} → ${review.remote}/${review.remote_ref.replace(/^refs\/heads\//, "")} (server: ${review.server}): ${review.ahead} local commit${review.ahead === 1 ? "" : "s"} to upload; ${review.behind} remote commit${review.behind === 1 ? "" : "s"} to download. ${review.blocked || "Review this destination before choosing a transfer."}`;
  }
  async function openTransfer() {
    if (transferRunning) { showDialog("git-transfer-dialog"); return; }
    const generation = ++transferGeneration;
    transferReview = null;
    paintTransfer("Reading local branch information…");
    showDialog("git-transfer-dialog");
    try {
      const review = await api("GET", "/api/git/transfer");
      if (generation !== transferGeneration) return;
      transferReview = review;
      paintTransfer(`${transferDescription(review)} This comparison uses local records. Choose Check remote before uploading or downloading.`);
    } catch (error) {
      if (generation === transferGeneration) paintTransfer(error.message);
    }
  }
  for (const action of ["check", "upload", "download"]) {
    $(`#git-transfer-${action}`).addEventListener("click", async () => {
      if (transferRunning || !transferReview) return;
      transferRunning = true;
      paintTransfer(action === "check" ? "Checking the remote…" : `${action === "upload" ? "Uploading" : "Downloading"} reviewed commits…`);
      try {
        transferReview = await api("POST", "/api/git/transfer", { action, review: transferReview });
        transferRunning = false;
        paintTransfer(`${action === "check" ? "Remote checked." : "Git transfer completed."} ${transferDescription(transferReview)}`, true);
        if (action === "download") document.dispatchEvent(new Event("verb:git-changed"));
        else await refreshGit();
      } catch (error) {
        if (action === "download") document.dispatchEvent(new Event("verb:git-changed"));
        transferRunning = false;
        // A failure can mean the remote changed or completion is uncertain. Never leave a
        // previously enabled upload button available against that obsolete observation.
        transferReview = null;
        paintTransfer(`${error.message} Close and reopen this review to check again.`);
        await refreshGit();
      }
    });
  }

  /** Starts an agent, or a plain terminal ("shell"), on the selected spec. Recorded server-side. */
  let handoffPending = false;
  function handoffError(message) {
    $("#handoff-error").textContent = message;
    $("#handoff-error").hidden = !message;
  }

  function openHandoff({ reviewed = false, role = "review" } = {}) {
    if (handoffPending) return;
    const spec = selected();
    if (!spec) return;
    state.handoffReview = reviewed ? { id: spec.id, revision: spec.revision } : null;
    $("#handoff-reviewed").hidden = !reviewed;
    $("#handoff-revision").textContent = reviewed ? spec.revision.slice(0, 10) : "";
    $("#handoff-refresh").hidden = true;
    $("#handoff-spec-preview").hidden = true;
    $("#handoff-submit").disabled = false;
    handoffError("");
    const form = $("#handoff-dialog-form");
    form.elements.scope.required = reviewed;
    form.elements.role.value = role;
    form.elements.scope.value = "";
    form.elements.checks.value = "";
    for (const key of ["outcome", "files", "reportedChecks"]) form.elements[key].value = "";
    function showSource(sessions) {
      const agents = sessions.filter(session => session.specId === spec.id && session.terminal?.running
        && !["shell", "external"].includes(session.agent));
      const current = agents.find(session => session.id === deps.ui.focusedTerminal) ?? agents[0];
      const agent = reviewed ? current?.agent : lastAgent(spec);
      $("#handoff-from").textContent = reviewed && current
        ? agentDisplayName(agent) + " · " + current.id.slice(0, 8)
        : agent ? agentDisplayName(agent) : "No agent running";
      $("#handoff-running-note").hidden = !current;
      return agent;
    }
    const from = showSource(deps.ui.state?.sessions ?? []);
    $("#handoff-title").textContent = `Hand off spec ${spec.id}`;
    const to = $("#handoff-to");
    for (const option of to.options) {
      option.disabled = reviewed && (option.value === "shell" || deps.ui.installed?.[option.value] === false);
    }
    if (to.selectedOptions[0]?.disabled || (from && to.value === from)) {
      to.value = [...to.options].find(option => !option.disabled && option.value !== from)?.value
        ?? [...to.options].find(option => !option.disabled)?.value ?? "";
    }
    $("#handoff-note").value = "";
    showDialog("handoff-dialog");
    $("#handoff-note").focus();
    if (reviewed) {
      const review = state.handoffReview;
      readWithBusyRetry(api, "/api/workspace").then(workspace => {
        if (state.handoffReview === review && $("#handoff-dialog").open) showSource(workspace.sessions ?? []);
      }).catch(() => { /* Keep the last observed session state if the host is busy. */ });
    }
  }

  async function switchBranch() {
    const spec = selected();
    if (!spec) return;
    try {
      const result = await api("POST", `/api/specs/${spec.id}/branch`);
      toast(`${result.message}. Recorded in the audit trail.`);
      document.dispatchEvent(new Event("verb:git-changed"));
      await refreshSpecs();
    } catch (error) {
      toast(error.message, "error");
    }
  }

  async function copySpecPath() {
    const spec = selected();
    if (!spec) return;
    try {
      await navigator.clipboard.writeText(spec.file);
      toast(`Copied ${spec.file}`);
    } catch {
      toast(spec.file);
    }
  }

  // ---------------------------------------------------------------------------- dialogs

  $("#spec-dialog-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const form = new FormData(event.target);
    const fromStudio = state.newSpecView === "studio";
    const submit = event.currentTarget.querySelector('[type="submit"]');
    if (submit.disabled) return;
    submit.disabled = true;
    try {
      const spec = await api("POST", "/api/specs", {
        title: form.get("title"),
        problem: form.get("problem"),
        users: form.get("users"),
        criteria: String(form.get("criteria") || "").split("\n"),
        out_of_scope: form.get("outOfScope"),
      });
      closeDialog("spec-dialog");
      replaceSpec(spec);
      selectSpec(spec.id, { navigate: !fromStudio });
      if (form.get("workItem")) {
        try {
          const result = await linkSpec(spec, form.get("workItem"), form.getAll("requirement").map(String));
          if (result.message) toast(result.message);
        } catch (error) {
          toast(`Spec ${spec.id} was created. Linking failed: ${error.message}`, "error");
          refreshGit();
          return;
        }
      }
      if (fromStudio) {
        deps.extras.onStudioSpecCreated?.(String(form.get("problem") || ""));
      }
      toast(`Spec ${spec.id} created in ${spec.file}`);
      refreshGit();
    } catch (error) {
      toast(error.message, "error");
    } finally {
      submit.disabled = false;
    }
  });

  $("#stage-dialog-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const spec = selected();
    if (!spec || state.pending?.kind !== "stage") return;
    try {
      const result = await api("POST", `/api/specs/${state.pending.specId}/stage`, {
        stage: state.pending.stage,
        note: $("#stage-note").value,
        expectedRevision: state.pending.revision,
        expectedContext: state.pending.context,
      });
      closeDialog("stage-dialog");
      replaceSpec(result.spec);
      toast(
        stageSummary(result.spec.stage, result.warnings.length),
        result.warnings.length ? "error" : "info",
      );
      refreshGit();
      refreshSpecs();
      deps.refreshState(true);
    } catch (error) {
      toast(error.message, "error");
      refreshSpecs();
    }
  });

  $("#commit-repair").addEventListener("click", async () => {
    const recovery = commitReview?.recovery;
    if (!recovery?.canRepair || commitRunning) return;
    commitRunning = true;
    $("#commit-repair").disabled = true;
    $("#commit-repair").textContent = "Repairing…";
    updateCommitSelection();
    try {
      const result = await api("POST", "/api/git/commit-recovery", { version: recovery.version });
      closeDialog("commit-dialog");
      toast(result.message);
      document.dispatchEvent(new Event("verb:git-changed"));
      await refreshSpecs();
    } catch (error) {
      $("#commit-scope").textContent = `Repair not confirmed: ${error.message}. Reopen review before trying again.`;
      commitReview = { ...commitReview, recovery: { ...recovery, canRepair: false } };
    } finally {
      commitRunning = false;
      updateCommitSelection();
    }
  });

  $("#commit-dialog-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const spec = commitSpec;
    if (commitRunning || !commitReview || commitReview.blocked) return;
    const paths = [...$("#commit-files").querySelectorAll("input:checked")].map(input => input.dataset.commitPath);
    if (!paths.length) return;
    commitRunning = true;
    updateCommitSelection();
    try {
      const result = await api("POST", "/api/git/commit", {
        selection: { version: commitReview.version, paths },
        message: $("#commit-message").value,
        specId: $("#commit-record").checked && spec ? spec.id : null,
      });
      closeDialog("commit-dialog");
      toast(`${result.message}.`);
      document.dispatchEvent(new Event("verb:git-changed"));
      await refreshSpecs();
    } catch (error) {
      toast(error.message, "error");
      $("#commit-scope").textContent = `Commit not confirmed: ${error.message} Close and reopen this review before trying again. Check Git history if the connection was interrupted; do not resend blindly.`;
      commitReview = null;
      document.dispatchEvent(new Event("verb:git-changed"));
    } finally {
      commitRunning = false;
      updateCommitSelection();
    }
  });

  // ------------------------------------------------------------------------------ theme

  const THEME_UI = {
    system: ["monitor", "System", "Theme follows your system setting. Click for light."],
    light: ["sun", "Light", "Light theme. Click for dark."],
    dark: ["moon", "Dark", "Dark theme. Click to follow your system."],
  };
  function renderThemeToggle() {
    const [glyph, label, title] = THEME_UI[deps.theme.preference] ?? THEME_UI.system;
    document.querySelector("#theme-toggle .theme-icon").innerHTML = icon(glyph, { size: 15 });
    $("#theme-label").textContent = label;
    $("#theme-toggle").title = title;
  }
  renderThemeToggle();

  // ---------------------------------------------------------------------------- palette

  function commands(query = "") {
    const spec = selected();
    const list = [
      { title: "New spec", hint: "Alt+N", run: openNewSpec },
      { title: "New terminal", hint: "Alt+T", run: deps.launchQuickTerminal },
      { title: "Commit changes", hint: "Alt+C", run: openCommit },
      { title: "Refresh Git status", run: refreshGit },
      { title: "Review Git upload / download", run: openTransfer },
      { title: "Keyboard shortcuts", hint: "?", run: () => showDialog("shortcuts-dialog") },
      ...["system", "light", "dark"].map((choice) => ({
        title: `Theme: ${choice === "system" ? "follow system" : choice}`,
        hint: deps.theme.preference === choice ? "current" : "",
        run: () => {
          deps.theme.set(choice);
          renderThemeToggle();
        },
      })),
      ...VIEWS.map((view, i) => ({
        title: `Go to ${view.charAt(0).toUpperCase()}${view.slice(1)}`,
        hint: `Alt+${i + 1}`,
        run: () => deps.showView(view),
      })),
    ];
    if (spec) {
      list.splice(
        2,
        0,
        { title: `Start agent on spec ${spec.id}`, hint: spec.title, run: () => deps.extras.openBenchLaunch?.(spec.id) },
        { title: `Hand off spec ${spec.id} to another agent`, run: openHandoff },
        { title: `Switch to branch ${spec.branch}`, run: switchBranch },
        { title: `Copy spec file path`, hint: spec.file, run: copySpecPath },
        ...STAGES.filter((s) => s !== spec.stage).map((stage) => ({
          title: `Move spec ${spec.id} to ${STAGE_GUIDE[stage].label}`,
          run: () => openStage(stage),
        })),
      );
    }
    for (const s of state.specs) {
      list.push({ title: `Open spec ${s.id} · ${s.title}`, hint: STAGE_GUIDE[s.stage].label, run: () => selectSpec(s.id) });
    }
    list.push(...(deps.extras.shellCommands?.() ?? []));
    list.push(...(deps.project?.commands(query) ?? []));
    // A typed question: offer Ask Verb, first when it reads like a question.
    const askEntries = deps.extras?.ask?.commands(query) ?? [];
    if (/\?$|^(what|where|who|why|how|which|when)\b/i.test(query.trim())) list.unshift(...askEntries);
    else list.push(...askEntries);
    return list;
  }

  function openPalette() {
    $("#palette-input").value = "";
    renderPalette();
    showDialog("palette-dialog");
    $("#palette-input").focus();
  }
  function renderPalette() {
    state.palette = filterCommands(commands($("#palette-input").value), $("#palette-input").value).slice(0, 12);
    state.paletteIndex = Math.min(state.paletteIndex, Math.max(0, state.palette.length - 1));
    $("#palette-list").innerHTML =
      state.palette
        .map(
          (c, i) =>
            `<li role="option" data-palette="${i}" aria-selected="${i === state.paletteIndex}" class="${i === state.paletteIndex ? "active" : ""}"><span>${escapeHtml(c.title)}</span>${c.hint ? `<small>${escapeHtml(c.hint)}</small>` : ""}</li>`,
        )
        .join("") || '<li class="muted">No matching command</li>';
  }
  function runPalette(index) {
    const command = state.palette[index];
    if (!command) return;
    closeDialog("palette-dialog");
    command.run();
  }
  $("#palette-input").addEventListener("input", () => {
    state.paletteIndex = 0;
    renderPalette();
  });
  $("#palette-input").addEventListener("keydown", (event) => {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const n = state.palette.length || 1;
      state.paletteIndex = (state.paletteIndex + (event.key === "ArrowDown" ? 1 : n - 1)) % n;
      renderPalette();
    } else if (event.key === "Enter") {
      event.preventDefault();
      runPalette(state.paletteIndex);
    }
  });
  $("#palette-dialog").addEventListener("click", (event) => {
    const item = event.target.closest("[data-palette]");
    if (item) runPalette(Number(item.dataset.palette));
    else if (event.target.id === "palette-dialog") closeDialog("palette-dialog");
  });
  $("#shortcut-list").innerHTML = SHORTCUTS.map(([keys, what]) => `<dt><kbd>${escapeHtml(keys)}</kbd></dt><dd>${escapeHtml(what)}</dd>`).join("");

  document.addEventListener("click", event => {
    const action = event.target.closest("[data-action]")?.dataset.action;
    const handlers = {
      "new-spec": openNewSpec, commit: openCommit, "git-transfer": openTransfer,
      "switch-branch": switchBranch, palette: openPalette,
      theme: () => { deps.theme.cycle(); renderThemeToggle(); },
    };
    handlers[action]?.();
  });
  document.addEventListener("verb:git-changed", () => refreshGit());

  $("#handoff-dialog").addEventListener("cancel", event => {
    if (handoffPending) event.preventDefault();
  });
  $("#handoff-refresh").addEventListener("click", async event => {
    const button = event.currentTarget;
    const review = state.handoffReview;
    if (!review || handoffPending) return;
    button.disabled = true;
    try {
      const result = await readWithBusyRetry(api, `/api/specs/${review.id}`);
      if (state.handoffReview !== review || !$("#handoff-dialog").open) return;
      replaceSpec(result);
      state.handoffReview = { id: result.id, revision: result.revision };
      $("#handoff-revision").textContent = result.revision.slice(0, 10);
      const preview = $("#handoff-spec-preview");
      preview.querySelector("pre").textContent = result.body;
      preview.hidden = false;
      preview.open = true;
      button.hidden = true;
      handoffError("Review the refreshed spec, scope, and checks before handing off.");
      $("#handoff-submit").disabled = false;
    } catch (error) {
      handoffError(error.message);
    } finally {
      button.disabled = false;
    }
  });

  $("#handoff-dialog-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const spec = selected();
    if (!spec) return;
    const form = event.currentTarget;
    const submit = $("#handoff-submit");
    if (handoffPending || submit.disabled) return;
    const to = $("#handoff-to").value;
    if (!to) return handoffError("Choose an available agent.");
    const review = state.handoffReview;
    if (review && review.id !== spec.id) {
      toast("The selected spec changed. Review the handoff again.", "error");
      return;
    }
    const approved = review ? {
      revision: review.revision, role: form.elements.role.value,
      scope: form.elements.scope.value, checks: form.elements.checks.value,
      result: { outcome: form.elements.outcome.value, files: form.elements.files.value, checks: form.elements.reportedChecks.value },
    } : undefined;
    handoffError("");
    handoffPending = true;
    let stale = false;
    submit.disabled = true;
    const closeButtons = [...form.querySelectorAll("[data-close]")];
    closeButtons.forEach(button => { button.disabled = true; });
    const from = lastAgent(spec);
    // Only say the previous agent is still running when one actually is.
    const fromLive = specSessions(spec, deps.ui.state?.sessions ?? []).some((x) => x.live && x.agent === from);
    try {
      const result = await api("POST", `/api/specs/${spec.id}/handoff`, { to, note: $("#handoff-note").value, approved });
      closeDialog("handoff-dialog");
      deps.mountLaunchedSession({ id: result.sessionId, agent: to, specId: spec.id, role: approved?.role ?? null, isolated: false, state: "live", hasTerminal: true, terminal: { running: true, replayAvailable: true }, hostedHere: true });
      if (result.warning) toast(result.warning, "error");
      else
        toast(
          `Handed off to ${agentDisplayName(to)}. The note is in the spec's Handoff notes${fromLive ? `; ${agentDisplayName(from)} is still running, end it when you're ready` : ""}.`,
        );
      await Promise.all([refreshSpecs(), refreshGit()]);
    } catch (error) {
      if (review) {
        stale = error.code === "stale_handoff";
        $("#handoff-refresh").hidden = !stale;
        handoffError(error.message);
      } else {
        toast(error.message, "error");
      }
    } finally {
      handoffPending = false;
      submit.disabled = stale;
      closeButtons.forEach(button => { button.disabled = false; });
    }
  });

  const typing = (el) =>
    el && (el.closest(".xterm") || ["INPUT", "TEXTAREA", "SELECT"].includes(el.tagName) || el.isContentEditable);

  document.addEventListener("keydown", (event) => {
    const mac = /Mac|iPhone|iPad/.test(navigator.platform);
    // ⌘K always opens the palette on a Mac. Ctrl+K is the shell's kill-line, so elsewhere it is
    // taken only when focus is not in a terminal.
    if (event.key.toLowerCase() === "k" && ((mac && event.metaKey) || (!mac && event.ctrlKey && !typing(document.activeElement)))) {
      event.preventDefault();
      return openPalette();
    }
    if (document.querySelector("dialog[open]")) return;
    if (event.altKey && !event.metaKey && !event.ctrlKey) {
      const map = { KeyN: openNewSpec, KeyT: deps.launchQuickTerminal, KeyC: openCommit };
      const digit = /^Digit([1-7])$/.exec(event.code);
      if (map[event.code] || digit) {
        event.preventDefault();
        return digit ? deps.showView(VIEWS[Number(digit[1]) - 1]) : map[event.code]();
      }
    }
    if (event.key === "?" && !typing(document.activeElement)) {
      event.preventDefault();
      showDialog("shortcuts-dialog");
    }
  });

  refreshSpecs();
  refreshGit();
  setInterval(() => {
    if (deps.ui.view !== "studio" || document.hidden) return;
    refreshGit();
  }, 5000);
  setInterval(() => {
    if (document.hidden) return;
    if (["bench-board", "needs-you"].includes(deps.ui.view)) refreshSpecs();
  }, 15000);

  return {
    selectSpec,
    openStage,
    refreshSpecs,
    runCheck: () => lifecycle.openCheck(),
    openProof: (action, index, evidenceId) => lifecycle.openProof(action, index, evidenceId),
    openCommit,
    openTransfer,
    reviewEvidence() { if (selected()) deps.extras.openBenchSpec?.(selected().id, "review"); },
    openLifecycle: lifecycle.openAction,
    evidenceHistory: lifecycle.history,
    exportEvidence: lifecycle.exportEvidence,
    async reopenCriterion(index) {
      const spec = selected();
      if (!spec?.criteria[index]?.done) return;
      replaceSpec(await api("POST", `/api/specs/${spec.id}/criteria/${index}`, { done: false }));
    },
    switchBranch,
    openHandoff,
    openNewSpec,
    openLinkSpec,
    current: () => ({ spec: selected(), git: state.git, meters: state.meters,
      presentation: selected() ? presentation(selected()) : null, specs: state.specs, error: state.specsError, loaded: state.specsLoaded }),
    summaryFor,
    boardSummaryFor: spec => state.summaries[spec.id],
    meterFor: (id) => state.meters?.[id] ?? null,
    render,
    suggestProof(index, evidence) {
      const spec = selected();
      if (!spec?.criteria[index] || spec.criteria[index].done) return false;
      deps.extras.openBenchSpec?.(spec.id, "review");
      lifecycle.openAction("claim", index, evidence).catch(error => toast(error.message, "error"));
      return true;
    },
    onShowView(view) {
      if (view === "studio") {
        refreshMeters();
        refreshSpecs();
        refreshGit();
      }
    },
  };
}
