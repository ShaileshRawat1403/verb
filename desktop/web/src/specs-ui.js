// The spec-driven workbench: specs list, stage bar, acceptance criteria with evidence, Git, audit
// trail, command palette and keyboard shortcuts. Rules live in workbench.js (tested); this file
// only talks to the DOM and the API.

import {
  STAGES,
  STAGE_GUIDE,
  changeLabel,
  suggestCommitMessage,
  commitPrefix,
  filterCommands,
  groupByStage,
  lastAgent,
  meterLabel,
  progressLabel,
  skippedStages,
  specSessions,
  stageWarnings,
} from "./workbench.js";
import { agentDisplayName } from "./view.js";
import { icon } from "./icons.js";

const SHORTCUTS = [
  ["⌘K / Ctrl+K", "Command palette: search specs and run any action"],
  ["Alt+N", "New spec"],
  ["Alt+T", "New terminal"],
  ["Alt+C", "Commit changes"],
  ["Alt+1 … Alt+7", "Specs, Project, Sessions, Overview, Tasks, Memory, Host"],
  ["Alt+A", "Ask Verb about this project"],
  ["?", "This list"],
  ["Esc", "Close a dialog"],
];
const VIEWS = ["specs", "project", "sessions", "overview", "tasks", "memory", "host"];

export function initWorkbench(deps) {
  const { api, toast, showDialog, closeDialog, escapeHtml, $ } = deps;
  const state = {
    specs: [],
    git: null,
    selectedId: readSelected(),
    pending: null, // what an open stage/evidence dialog will do on submit
    meters: {}, // session id -> context meter, for running Claude/Codex sessions
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

  // ------------------------------------------------------------------------------- data

  async function refreshSpecs() {
    try {
      const result = await api("GET", "/api/specs");
      state.specs = (result.specs || []).map(normalize);
      if (!selected() && state.specs.length) state.selectedId = state.specs.at(-1).id;
      render();
    } catch (error) {
      toast(error.message, "error");
    }
  }
  async function refreshGit() {
    try {
      state.git = await api("GET", "/api/git");
    } catch (error) {
      state.git = { error: error.message, changes: [], recent: [] };
    }
    renderGit();
    renderProof();
    renderChanges();
  }
  const normalize = (spec) => ({ ...spec, criteriaDone: spec.criteria_done ?? spec.criteriaDone ?? 0 });
  function replaceSpec(spec) {
    const next = normalize(spec);
    state.specs = state.specs.map((s) => (s.id === next.id ? next : s));
    if (!state.specs.some((s) => s.id === next.id)) state.specs.push(next);
    render();
  }

  // ----------------------------------------------------------------------------- render

  function render() {
    $("#nav-spec-count").textContent = state.specs.filter((s) => s.stage !== "ship").length;
    renderList();
    renderDetail();
    renderSessions();
    renderProof();
    renderChanges();
    renderGit();
    placeTerminals();
  }

  function renderList() {
    const groups = groupByStage(state.specs);
    $("#spec-list").innerHTML = groups.length
      ? groups
          .map(
            ({ stage, specs }) =>
              `<div class="spec-group"><div class="spec-group-label">${STAGE_GUIDE[stage].label}</div>${specs
                .map(
                  (spec) =>
                    `<button type="button" class="spec-item${spec.id === state.selectedId ? " current" : ""}${specNeedsYou(spec) ? " needs-you" : ""}" data-spec-id="${escapeHtml(spec.id)}" data-stage="${escapeHtml(spec.stage)}"><i class="spec-dot" aria-hidden="true"></i><span class="spec-name">${escapeHtml(spec.title)}</span><small>${specNeedsYou(spec) ? '<em class="needs-text">An agent needs you</em>' : `<span class="spec-id">${escapeHtml(spec.id)}</span> · ${escapeHtml(progressLabel(spec))}`}</small></button>`,
                )
                .join("")}</div>`,
          )
          .join("")
      : '<p class="rail-empty">No specs yet. A spec is a short description of one piece of work.</p>';
  }

  function renderDetail() {
    const spec = selected();
    $("#spec-empty").hidden = Boolean(spec);
    $("#spec-detail").hidden = !spec;
    $("#flow-explainer").innerHTML = STAGES.map(
      (stage, i) =>
        `<li><b>${i + 1}. ${STAGE_GUIDE[stage].label}</b><span>${escapeHtml(STAGE_GUIDE[stage].goal)}</span></li>`,
    ).join("");
    if (!spec) return;
    $("#spec-kicker").textContent = `Spec ${spec.id} · ${spec.file}`;
    $("#spec-title").textContent = spec.title;
    $("#spec-branch-chip").textContent = spec.branch || "no branch";
    const onBranch = state.git?.branch && state.git.branch === spec.branch;
    const switchButton = document.querySelector('[data-action="switch-branch"]');
    switchButton.hidden = !spec.branch || onBranch;
    $("#spec-branch-chip").classList.toggle("active", Boolean(onBranch));
    const current = STAGES.indexOf(spec.stage);
    const skipped = skippedStages(spec);
    $("#stage-bar").innerHTML = STAGES.map((stage, i) => {
      const wasSkipped = i < current && skipped.has(stage);
      const status = wasSkipped ? "skipped" : i < current ? "done" : i === current ? "current" : "todo";
      const dot = wasSkipped ? "–" : i < current ? icon("check", { size: 12 }) : i + 1;
      const title = wasSkipped ? "Skipped (recorded in the audit trail)" : STAGE_GUIDE[stage].goal;
      return `<li class="${status}"><button type="button" data-stage="${stage}" title="${escapeHtml(title)}" ${i === current ? 'aria-current="step"' : ""}><span class="stage-dot">${dot}</span>${STAGE_GUIDE[stage].label}</button></li>`;
    }).join("");
    const guide = STAGE_GUIDE[spec.stage];
    const nextStage = STAGES[current + 1];
    const agentLive =
      specSessions(spec, deps.ui.state?.sessions ?? []).some((x) => x.live && x.agent !== "shell") ||
      (deps.ui.talkSpecs?.has(spec.id) ?? false);
    // An agent building a spec that still says Spec or Plan: offer to make the record match the
    // work (a nudge, never a block; moving still shows its usual warnings).
    $("#stage-guide").innerHTML =
      agentLive && current < STAGES.indexOf("build")
        ? `<div><b>An agent is working on this spec while it is still in ${escapeHtml(guide.label)}.</b> Move it to Build so the record matches the work.</div><button class="text-button" type="button" data-stage="build">Move to Build</button>`
        : `<div><b>${escapeHtml(guide.goal)}.</b> ${escapeHtml(guide.next)}</div>${
            nextStage
              ? `<button class="text-button" type="button" data-stage="${nextStage}">Move to ${STAGE_GUIDE[nextStage].label}</button>`
              : `<span class="done-badge${state.shipBadge?.id === spec.id && state.shipBadge.pending ? " pending" : ""}" id="ship-badge">${escapeHtml(state.shipBadge?.id === spec.id ? state.shipBadge.text : "Checking…")}</span>`
          }`;
    if (!nextStage) paintShipBadge(spec);
    // One primary at a time: starting an agent until one is working, then committing its work.
    const start = document.querySelector('[data-action="spec-agent"]');
    start.classList.toggle("primary-button", !agentLive);
    start.classList.toggle("secondary-button", agentLive);
    // Committing leads once there is work to commit: an agent's changes beyond the spec file itself
    // (in a user test it turned primary the moment the agent started, with nothing done yet).
    const work = (state.git?.changes ?? []).some((c) => c.path !== spec.file);
    const commit = $("#top-commit");
    commit.classList.toggle("primary-button", agentLive && work);
    commit.classList.toggle("secondary-button", !(agentLive && work));
  }

  /** "Shipped" only when it is true: the work committed and its branch merged. */
  async function paintShipBadge(spec) {
    let ship = null;
    try {
      ship = (await api("GET", `/api/specs/${spec.id}/stage-check?to=ship`)).ship;
    } catch {
      /* leave the plain label */
    }
    const badge = $("#ship-badge");
    if (!badge || selected()?.id !== spec.id) return;
    const todo = [];
    if (ship?.uncommitted) todo.push(`commit ${ship.uncommitted} change${ship.uncommitted === 1 ? "" : "s"}`);
    if (ship?.unmerged) todo.push(`merge ${ship.unmerged[0]} into ${ship.unmerged[1]}`);
    const text = !ship ? "Ship" : todo.length ? `To finish: ${todo.join(", then ")}` : "Shipped";
    state.shipBadge = { id: spec.id, text, pending: todo.length > 0 };
    badge.classList.toggle("pending", todo.length > 0);
    badge.textContent = text;
  }

  /** Who has worked on this spec, from its audit trail, and which of them are running now. */
  /** Whether a session is an agent waiting for the person's answer (from the observer reply). */
  const needsYou = (id) => deps.ui.waitingTerminals?.has(id) ?? false;
  const specNeedsYou = (spec) =>
    specSessions(spec, deps.ui.state?.sessions ?? []).some((x) => x.live && needsYou(x.id)) ||
    (deps.ui.waitingTalkSpecs?.has(spec.id) ?? false);

  function renderSessions() {
    const spec = selected();
    const board = $("#spec-sessions");
    const sessions = spec ? specSessions(spec, deps.ui.state?.sessions ?? []) : [];
    board.hidden = !sessions.length;
    board.innerHTML = sessions.length
      ? `<div class="board-head"><span class="section-kicker">SESSIONS ON THIS SPEC</span><span class="muted">${sessions.filter((x) => x.live).length} running</span></div>${boardRows(sessions)}`
      : "";
  }

  /** Running sessions and the two latest ended ones; older ones fold into "N earlier sessions". */
  function boardRows(sessions) {
    const live = sessions.filter((x) => x.live);
    const ended = sessions.filter((x) => !x.live);
    const row = (x) =>
              `<div class="board-row${x.live ? " live" : ""}${needsYou(x.id) ? " needs-you" : ""}"><span class="state-dot" data-state="${needsYou(x.id) ? "warn" : x.live ? "ok" : "idle"}"></span><b>${escapeHtml(agentDisplayName(x.agent))}</b><code>${escapeHtml(x.short)}</code><span class="muted">${needsYou(x.id) ? "<em>needs you</em>" : x.live ? "running" : "ended"} · started ${escapeHtml(x.at.replace("T", " ").slice(0, 16))} UTC</span>${x.live ? `<button class="text-button" type="button" data-focus-session="${escapeHtml(x.id)}">Show</button>` : ""}</div>${x.live && state.meters[x.id] ? meterRow(state.meters[x.id]) : ""}`;
    const shown = [...live, ...ended.slice(0, 2)].map(row).join("");
    const older = ended.slice(2);
    return older.length
      ? `${shown}<details class="board-older"><summary>${older.length} earlier session${older.length === 1 ? "" : "s"}</summary>${older.map(row).join("")}</details>`
      : shown;
  }

  function meterRow(meter) {
    const m = meterLabel(meter);
    const bar = m.percent == null ? "" : `<span class="meter-bar" data-tone="${m.tone}"><span style="width:${m.percent}%"></span></span>`;
    const act = m.tone === "warm" || m.tone === "hot" ? `<button class="text-button" type="button" data-action="handoff">Hand off</button>` : "";
    return `<div class="meter-row" data-tone="${m.tone}" title="${escapeHtml(m.title ?? "")}">${bar}<span>${escapeHtml(m.text)}</span>${act}</div>`;
  }

  async function refreshMeters() {
    const spec = selected();
    const anyAgentLive = spec && specSessions(spec, deps.ui.state?.sessions ?? []).some((x) => x.live && /^(claude|codex)$/.test(x.agent));
    if (!anyAgentLive) return;
    try {
      state.meters = (await api("GET", "/api/meters")).meters ?? {};
      renderSessions();
    } catch {
      // A missing meter is shown as nothing, never as a wrong number.
    }
  }

  function renderProof() {
    const spec = selected();
    $("#criteria-progress").textContent = spec ? progressLabel(spec) : "";
    $("#criteria-list").innerHTML = spec
      ? spec.criteria
          .map(
            (c) =>
              `<li><label class="criterion${c.done ? " done" : ""}"><input type="checkbox" data-criterion="${c.index}" ${c.done ? "checked" : ""} /><span>${escapeHtml(c.text)}</span></label></li>`,
          )
          .join("") || '<li class="muted">This spec has no acceptance criteria yet. Add some to its file.</li>'
      : '<li class="muted">Select a spec.</li>';
    const changes = state.git?.changes ?? [];
    $("#changes-count").textContent = changes.length;
    $("#changes-list").innerHTML = changes.length
      ? changes
          .slice(0, 60)
          .map(
            (c) =>
              `<li><span class="change-kind" data-kind="${escapeHtml(c.code)}">${escapeHtml(changeLabel(c.code))}</span><button type="button" class="change-link" data-open-diff="${escapeHtml(c.path)}" title="Show the changes in ${escapeHtml(c.path)}"><code>${escapeHtml(c.path)}</code></button>${diffStat(c)}</li>`,
          )
          .join("")
      : '<li class="muted">No unsaved changes.</li>';
    $("#audit-list").innerHTML = spec
      ? [...spec.audit]
          .reverse()
          .map(
            (a) =>
              `<li><time>${escapeHtml(a.at.replace("T", " ").slice(0, 16))}</time><span class="audit-actor">${escapeHtml(a.actor)}</span><span>${escapeHtml(a.action)}</span></li>`,
          )
          .join("")
      : "";
  }

  function renderGit() {
    const git = state.git;
    if (!git) return;
    if (git.error) {
      $("#git-branch").textContent = "Git unavailable";
      $("#git-status").textContent = git.error;
      return;
    }
    $("#git-branch").innerHTML = `<span class="branch-icon">${icon("branch", { size: 14 })}</span> ${escapeHtml(git.branch ?? "detached")}`;
    const parts = [
      git.changes.length ? `${git.changes.length} unsaved change${git.changes.length === 1 ? "" : "s"}` : "All changes saved",
    ];
    if (git.ahead) parts.push(`${git.ahead} to push`);
    if (git.behind) parts.push(`${git.behind} to pull`);
    $("#git-status").textContent = parts.join(" · ");
    // The top bar's count comes from a slower poll; keep it in step with what this card shows.
    $("#top-changes").textContent = `${git.changes.length} changed`;
    const commit = $("#top-commit");
    commit.hidden = git.changes.length === 0;
    commit.querySelector(".btn-label").textContent = `Commit ${git.changes.length} file${git.changes.length === 1 ? "" : "s"}`;
    $("#git-recent").innerHTML = git.recent
      .map(
        (c) =>
          `<li title="${escapeHtml(`${c.author}, ${c.when}`)}"><code>${escapeHtml(c.sha)}</code> ${escapeHtml(c.subject)}</li>`,
      )
      .join("");
  }

  // ---------------------------------------------------------------------------- changes (diffs)

  function diffStat(c) {
    if (c.added === undefined) return "";
    return `<span class="diff-stat"><span class="plus">+${c.added}</span><span class="minus">−${c.removed}</span></span>`;
  }

  const diffCache = new Map(); // "path|added|removed" → rendered HTML
  const openDiffs = new Set();
  let changesSignature = "";

  /** One collapsible row per changed file, in the spec document. Rebuilt only when the list changes. */
  function renderChanges() {
    const section = $("#spec-changes");
    const changes = state.git?.changes ?? [];
    section.hidden = !selected() || changes.length === 0;
    const signature = changes.map((c) => `${c.path}|${c.added}|${c.removed}|${c.code}`).join("\n");
    if (signature === changesSignature) return;
    changesSignature = signature;
    const added = changes.reduce((n, c) => n + (c.added ?? 0), 0);
    const removed = changes.reduce((n, c) => n + (c.removed ?? 0), 0);
    $("#spec-changes-total").innerHTML = `${changes.length} file${changes.length === 1 ? "" : "s"} <span class="diff-stat"><span class="plus">+${added}</span><span class="minus">−${removed}</span></span>`;
    const shown = changes.slice(0, 40);
    $("#spec-changes-list").innerHTML =
      shown
        .map(
          (c) =>
            `<details class="diff-card" data-diff-path="${escapeHtml(c.path)}"${openDiffs.has(c.path) ? " open" : ""}><summary><span class="diff-chevron" aria-hidden="true">${icon("chevron-right", { size: 14 })}</span><code class="diff-path">${escapeHtml(c.path)}</code><span class="change-kind">${escapeHtml(changeLabel(c.code))}</span>${diffStat(c)}</summary><div class="diff-body"></div></details>`,
        )
        .join("") +
      (changes.length > shown.length
        ? `<p class="muted">and ${changes.length - shown.length} more; the commit dialog lists every file.</p>`
        : "");
    for (const card of section.querySelectorAll(".diff-card[open]")) loadDiff(card);
  }

  async function loadDiff(card) {
    const path = card.dataset.diffPath;
    const change = (state.git?.changes ?? []).find((c) => c.path === path);
    const key = `${path}|${change?.added}|${change?.removed}`;
    const body = card.querySelector(".diff-body");
    if (diffCache.has(key)) {
      body.innerHTML = diffCache.get(key);
      return;
    }
    body.innerHTML = '<p class="diff-note">Reading the diff…</p>';
    try {
      const diff = await api("GET", `/api/git/diff?path=${encodeURIComponent(path)}`);
      const html = renderDiff(diff);
      diffCache.set(key, html);
      body.innerHTML = html;
    } catch (error) {
      body.innerHTML = `<p class="diff-note">${escapeHtml(error.message)}</p>`;
    }
  }

  function renderDiff(diff) {
    const note = diff.reason ? `<p class="diff-note">${escapeHtml(diff.reason)}</p>` : "";
    const rows = diff.hunks
      .map(
        (h) =>
          `<div class="diff-hunk">${escapeHtml(h.header)}</div>` +
          h.lines
            .map(
              (l) =>
                `<div class="diff-line ${l.kind}"><span class="ln">${l.old ?? ""}</span><span class="ln">${l.new ?? ""}</span><span class="sign">${l.kind === "add" ? "+" : l.kind === "del" ? "−" : ""}</span><span class="code">${escapeHtml(l.text) || " "}</span></div>`,
            )
            .join(""),
      )
      .join("");
    return note + (rows ? `<div class="diff-lines">${rows}</div>` : "");
  }

  /** The live terminals belong to the spec you are working on: show them in its work area. */
  function placeTerminals() {
    const grid = $("#terminal-grid");
    const inSpecs = deps.ui.view === "specs" && selected();
    const target = inSpecs ? $("#spec-work") : $("#view-sessions .terminal-area");
    if (grid && target && grid.parentElement !== target) {
      target.append(grid);
      requestAnimationFrame(deps.fitTerminals);
    }
  }

  // ---------------------------------------------------------------------------- actions

  function selectSpec(id) {
    state.selectedId = id;
    remember(id);
    if (deps.ui.view !== "specs") deps.showView("specs");
    render();
  }

  function openNewSpec() {
    $("#spec-dialog-form").reset();
    showDialog("spec-dialog");
    $("#spec-title-input").focus();
  }

  async function openStage(stage) {
    const spec = selected();
    if (!spec || stage === spec.stage) return;
    // The server's list is the one the move records (it also knows git: uncommitted work, an
    // unmerged branch); the local rules are the fallback if it cannot be asked.
    let warnings = stageWarnings(spec, stage);
    try {
      warnings = (await api("GET", `/api/specs/${spec.id}/stage-check?to=${encodeURIComponent(stage)}`)).warnings;
    } catch {
      /* keep the local warnings */
    }
    state.pending = { kind: "stage", stage };
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

  async function toggleCriterion(index, checkbox) {
    const spec = selected();
    const criterion = spec?.criteria[index];
    if (!criterion) return;
    if (!criterion.done) {
      checkbox.checked = false; // stays unticked until evidence is given and saved
      state.pending = { kind: "criterion", index };
      $("#evidence-dialog-sub").textContent = criterion.text;
      $("#evidence-text").value = "";
      showDialog("evidence-dialog");
      $("#evidence-text").focus();
      return;
    }
    try {
      replaceSpec(await api("POST", `/api/specs/${spec.id}/criteria/${index}`, { done: false }));
      toast(`Criterion ${index + 1} reopened. Recorded in the audit trail.`);
      refreshGit();
    } catch (error) {
      checkbox.checked = true;
      toast(error.message, "error");
    }
  }

  function openCommit() {
    const spec = selected();
    const changes = state.git?.changes ?? [];
    $("#commit-files").innerHTML = changes.length
      ? changes
          .slice(0, 40)
          .map((c) => `<li><span class="change-kind">${escapeHtml(changeLabel(c.code))}</span><code>${escapeHtml(c.path)}</code></li>`)
          .join("")
      : '<li class="muted">No changes to commit.</li>';
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
      ? `Record this commit in spec ${spec.id}'s audit trail`
      : "Select a spec to record this commit in its audit trail";
    showDialog("commit-dialog");
    const box = $("#commit-message");
    box.focus();
    box.setSelectionRange(prefix.length, box.value.length);
  }

  /** Starts an agent, or a plain terminal ("shell"), on the selected spec. Recorded server-side. */
  async function startOnSpec(agent) {
    const spec = selected();
    if (!spec) return deps.launchQuickTerminal();
    try {
      const result = await api("POST", `/api/specs/${spec.id}/agent`, { agent });
      deps.mountLaunchedSession({
        id: result.sessionId,
        agent,
        isolated: false,
        state: "live",
        hasTerminal: true,
        hostedHere: true,
      });
      if (agent !== "shell") toast(`${agentDisplayName(agent)} started on spec ${spec.id}. Recorded in the audit trail.`);
      refreshSpecs();
    } catch (error) {
      toast(error.message, "error");
    }
  }
  /** Antigravity starts as a Talk (src/talk.rs): a conversation view that continues in its terminal. */
  async function startTalk() {
    const spec = selected();
    if (!spec) return;
    try {
      const { talkId } = await api("POST", `/api/specs/${spec.id}/talk`);
      deps.mountTalk(talkId, { focus: true });
      toast(`Talking with Antigravity about spec ${spec.id}. Recorded in the audit trail.`);
      refreshSpecs();
    } catch (error) {
      toast(error.message, "error");
    }
  }
  // Every agent, Antigravity included, starts in its own terminal with the spec brief. Talk is an
  // optional, read-only conversation for Antigravity, offered beside it, never instead of it.
  const startAgent = () => startOnSpec($("#spec-agent").value);
  const syncTalkButton = () => {
    $("#spec-talk").hidden = $("#spec-agent").value !== "agy";
  };
  $("#spec-agent").addEventListener("change", syncTalkButton);

  function openHandoff() {
    const spec = selected();
    if (!spec) return;
    const from = lastAgent(spec);
    $("#handoff-from").textContent = from ? agentDisplayName(from) : "nobody yet";
    $("#handoff-title").textContent = `Hand off spec ${spec.id}`;
    const to = $("#handoff-to");
    if (from && to.value === from) to.value = [...to.options].find((o) => o.value !== from).value;
    $("#handoff-note").value = "";
    showDialog("handoff-dialog");
    $("#handoff-note").focus();
  }

  async function switchBranch() {
    const spec = selected();
    if (!spec) return;
    try {
      const result = await api("POST", `/api/specs/${spec.id}/branch`);
      toast(`${result.message}. Recorded in the audit trail.`);
      await Promise.all([refreshSpecs(), refreshGit()]);
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
      selectSpec(spec.id);
      toast(`Spec ${spec.id} created in ${spec.file}`);
      refreshGit();
    } catch (error) {
      toast(error.message, "error");
    }
  });

  $("#stage-dialog-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const spec = selected();
    if (!spec || state.pending?.kind !== "stage") return;
    try {
      const result = await api("POST", `/api/specs/${spec.id}/stage`, {
        stage: state.pending.stage,
        note: $("#stage-note").value,
      });
      closeDialog("stage-dialog");
      replaceSpec(result.spec);
      toast(
        result.warnings.length
          ? `Moved to ${STAGE_GUIDE[result.spec.stage].label}. Warning recorded: ${result.warnings.join("; ")}`
          : `Moved to ${STAGE_GUIDE[result.spec.stage].label}. Recorded in the audit trail.`,
        result.warnings.length ? "error" : "info",
      );
      refreshGit();
    } catch (error) {
      toast(error.message, "error");
    }
  });

  $("#evidence-dialog-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const spec = selected();
    if (!spec || state.pending?.kind !== "criterion") return;
    try {
      replaceSpec(
        await api("POST", `/api/specs/${spec.id}/criteria/${state.pending.index}`, {
          done: true,
          evidence: $("#evidence-text").value,
        }),
      );
      closeDialog("evidence-dialog");
      toast(`Criterion ${state.pending.index + 1} proven. Evidence recorded.`);
      refreshGit();
    } catch (error) {
      toast(error.message, "error");
    }
  });

  $("#commit-dialog-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const spec = selected();
    try {
      const result = await api("POST", "/api/git/commit", {
        message: $("#commit-message").value,
        specId: $("#commit-record").checked && spec ? spec.id : null,
      });
      closeDialog("commit-dialog");
      toast(`${result.message}.`);
      await Promise.all([refreshGit(), refreshSpecs()]);
    } catch (error) {
      toast(error.message, "error");
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
        { title: `Start agent on spec ${spec.id}`, hint: spec.title, run: startAgent },
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

  // -------------------------------------------------------------------------- events

  document.addEventListener("click", (event) => {
    const specButton = event.target.closest("[data-spec-id]");
    if (specButton) return selectSpec(specButton.dataset.specId);
    const stage = event.target.closest("[data-stage]");
    if (stage) return openStage(stage.dataset.stage);
    const action = event.target.closest("[data-action]")?.dataset.action;
    const handlers = {
      "new-spec": openNewSpec,
      commit: openCommit,
      "refresh-git": refreshGit,
      "switch-branch": switchBranch,
      "copy-spec-path": copySpecPath,
      "spec-terminal": () => startOnSpec("shell"),
      "spec-agent": startAgent,
      "spec-talk": startTalk,
      handoff: openHandoff,
      palette: openPalette,
      theme: () => {
        deps.theme.cycle();
        renderThemeToggle();
      },
    };
    if (handlers[action]) handlers[action]();
  });
  document.addEventListener("verb:git-changed", () => refreshGit());
  document.addEventListener("verb:observer", () => {
    renderList();
    renderSessions();
  });

  // A diff loads when its row opens; the rail's file names open (and scroll to) their row.
  document.addEventListener(
    "toggle",
    (event) => {
      const card = event.target.closest?.(".diff-card");
      if (!card) return;
      if (card.open) {
        openDiffs.add(card.dataset.diffPath);
        loadDiff(card);
      } else {
        openDiffs.delete(card.dataset.diffPath);
      }
    },
    true,
  );
  document.addEventListener("click", (event) => {
    const link = event.target.closest("[data-open-diff]");
    if (!link) return;
    const card = [...document.querySelectorAll(".diff-card")].find(
      (c) => c.dataset.diffPath === link.dataset.openDiff,
    );
    if (!card) return;
    card.open = true;
    card.scrollIntoView({ behavior: "smooth", block: "start" });
  });
  document.addEventListener("click", (event) => {
    const focus = event.target.closest("[data-focus-session]");
    if (focus) deps.selectTerminal?.(focus.dataset.focusSession);
  });

  $("#handoff-dialog-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const spec = selected();
    if (!spec) return;
    const to = $("#handoff-to").value;
    const from = lastAgent(spec);
    // Only say the previous agent is still running when one actually is.
    const fromLive = specSessions(spec, deps.ui.state?.sessions ?? []).some((x) => x.live && x.agent === from);
    try {
      const result = await api("POST", `/api/specs/${spec.id}/handoff`, { to, note: $("#handoff-note").value });
      closeDialog("handoff-dialog");
      deps.mountLaunchedSession({ id: result.sessionId, agent: to, isolated: false, state: "live", hasTerminal: true, hostedHere: true });
      toast(
        `Handed off to ${agentDisplayName(to)}. The note is in the spec's Handoff notes${fromLive ? `; ${agentDisplayName(from)} is still running, end it when you're ready` : ""}.`,
      );
      await Promise.all([refreshSpecs(), refreshGit()]);
    } catch (error) {
      toast(error.message, "error");
    }
  });

  // The agent picker remembers the last choice in this browser.
  try {
    const saved = localStorage.getItem("verb.agent");
    if (saved && $("#spec-agent").querySelector(`option[value="${CSS.escape(saved)}"]`)) $("#spec-agent").value = saved;
  } catch {}
  $("#spec-agent").addEventListener("change", () => {
    try {
      localStorage.setItem("verb.agent", $("#spec-agent").value);
    } catch {}
  });
  syncTalkButton();

  document.addEventListener("change", (event) => {
    const box = event.target.closest("[data-criterion]");
    if (box) toggleCriterion(Number(box.dataset.criterion), box);
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
    if (deps.ui.view !== "specs" || document.hidden) return;
    refreshGit();
    renderSessions(); // live/ended follows the workspace poll
  }, 5000);
  setInterval(() => deps.ui.view === "specs" && !document.hidden && refreshMeters(), 15000);

  return {
    selectSpec,
    switchBranch,
    openHandoff,
    current: () => ({ spec: selected(), git: state.git, meters: state.meters }),
    meterFor: (id) => state.meters?.[id] ?? null,
    render,
    /** Opens the evidence dialog for a criterion of the selected spec, prefilled; the person decides. */
    suggestProof(index, evidence) {
      const spec = selected();
      const criterion = spec?.criteria[index];
      if (!criterion || criterion.done) return false;
      state.pending = { kind: "criterion", index };
      $("#evidence-dialog-sub").textContent = criterion.text;
      $("#evidence-text").value = evidence;
      showDialog("evidence-dialog");
      $("#evidence-text").focus();
      return true;
    },
    onShowView(view) {
      if (view === "specs") {
        refreshMeters();
        refreshSpecs();
        refreshGit();
      }
      placeTerminals();
    },
  };
}
