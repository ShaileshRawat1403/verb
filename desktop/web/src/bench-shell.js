import { readWithBusyRetry } from "./read-retry.js";
import { makeResizer } from "./resize.js";
import { agentAvailability } from "./agent-availability.js";
import { initBenchReview } from "./bench-review.js";
import { documentDiff } from "./document-diff.js";
import { boardMarkup, specSignals } from "./bench-board.js";

export function initBenchShell({ $, api, ui, workbench, studio, briefExists, showView, selectTerminal, openSession, fitTerminals, escapeHtml, agentDisplayName, openFile, refreshFiles, openTask, openLaunch, loadInstalledAgents, showDialog, closeDialog, mountLaunchedSession, refreshState, refreshChecks, toast }) {
  const reviewTab = initBenchReview({ $, api, workbench, escapeHtml, showDialog, toast, refreshChecks });
  const app = $("#app");
  const panel = $("#bench-right-panel");
  const tree = $("#bench-work-tree");
  const toggleNavigation = $("#bench-toggle-navigation");
  const toggleDetails = $("#bench-toggle-right");
  const narrow = matchMedia("(max-width: 760px)");
  const preference = (key) => {
    try { return localStorage.getItem(key); } catch { return null; }
  };
  const remember = (key, value) => {
    try { localStorage.setItem(key, value); } catch { /* Optional browser preference. */ }
  };

  let detailsCollapsed = preference("verb.layout.details") === "collapsed";
  let mobileSurface = null;
  let activeTab = "plan";
  let selectedId = null;
  let terminalId = null;
  let selectedWork = null;
  let workItems = null;
  let workError = "";
  let request = 0;
  let lastTreeMarkup = "";

  const hostedSessions = () => (ui.state?.sessions ?? []).filter(session => session.hostedHere && session.terminal?.running);
  const openSessions = (spec) => hostedSessions().filter(session => session.specId === spec.id);
  const focusSpec = (id, tab) => {
    const spec = workbench.current().specs.find(spec => spec.id === id);
    if (!spec) return;
    selectedId = id;
    selectedWork = null;
    workbench.selectSpec(id, { navigate: false });
    showView("sessions");
    const running = openSessions(spec)[0];
    if (running) {
      selectTerminal(running.id);
    } else {
      if (narrow.matches) mobileSurface = "details";
      else detailsCollapsed = false;
      selectTab("plan");
    }
    if (tab) {
      if (narrow.matches) mobileSurface = "details";
      else detailsCollapsed = false;
      selectTab(tab);
    }
    render();
    syncLayout();
    if (!running || tab) {
      if (narrow.matches) panel.querySelector("[data-bench-overlay-close]").focus();
      else $("#bench-tab-" + (tab ?? "plan")).focus();
    }
  };

  const workTitle = item => item.brd?.match(/^#{1,6}[ 	]+(.+?)\s*#*\s*$/m)?.[1]?.replace(/\s+BRD$/i, "").trim()
    || item.id.replace(/^\d+-/, "").replaceAll("-", " ").replace(/^./, letter => letter.toUpperCase());

  const STAGES = ["spec", "plan", "build", "verify", "review", "ship"];
  const stageLabel = stage => stage[0].toUpperCase() + stage.slice(1);

  function render() {
    const { specs, error, loaded } = workbench.current();
    if (selectedId === null && !selectedWork && terminalId) {
      selectedId = specs.find(spec => openSessions(spec).some(session => session.id === terminalId))?.id ?? null;
    }
    const byId = new Map(specs.map(spec => [spec.id, spec]));
    const row = (id) => {
      const spec = byId.get(id);
      if (!spec) return "";
      const current = selectedId === id ? ' aria-current="true"' : "";
      return `<li><button type="button" data-bench-spec="${escapeHtml(id)}"${current} title="${escapeHtml(`${id} · ${spec.title}`)}"><span class="bench-spec-no type-cond">${escapeHtml(id)}</span><span class="visually-hidden"> · </span><span class="bench-spec-title">${escapeHtml(spec.title)}</span><small>${escapeHtml(stageLabel(spec.stage))}</small></button></li>`;
    };
    const group = (key, label, ids, item) => {
      const name = item
        ? `<button class="text-button bench-work-name" type="button" data-bench-work="${escapeHtml(key)}"${key === selectedWork ? ' aria-current="true"' : ""} title="${escapeHtml(label)}">${escapeHtml(label)}</button>`
        : `<span class="bench-work-name">${escapeHtml(label)}</span>`;
      return `<details data-bench-group="${escapeHtml(key)}"><summary><span class="bench-caret" aria-hidden="true"></span>${name} <span class="nav-count">${ids.length}</span>${item?.brd && item?.prd ? ` <button type="button" class="text-button bench-new-spec" data-bench-new-spec="${escapeHtml(key)}">New spec</button>` : ""}${item ? miniTraveler(item, ids.map(id => byId.get(id))) : ""}</summary><ul>${ids.map(row).join("")}</ul></details>`;
    };
    let markup;
    if (workError || error) {
      markup = `<p role="alert">${escapeHtml(workError || error)}</p><button type="button" class="text-button" data-bench-retry>Retry</button>`;
    } else if (!workItems || !loaded) {
      markup = '<p role="status">Loading work items…</p>';
    } else {
      const grouped = new Map(workItems.items.map(item => [item.id, []]));
      const unlinked = [];
      for (const spec of specs) (grouped.get(spec.work_item) ?? unlinked).push(spec.id);
      markup = workItems.items.map(item =>
        group(item.id, workTitle(item) + (item.draft ? " · Draft" : ""), grouped.get(item.id), item)
      ).join("") + group("unlinked", "Unlinked", unlinked);
    }
    if (markup !== lastTreeMarkup) {
      const expanded = new Set([...tree.querySelectorAll("details[open]")].map(group => group.dataset.benchGroup));
      const focused = tree.contains(document.activeElement) ? document.activeElement.closest("[data-bench-spec], [data-bench-work]")?.dataset : null;
      tree.innerHTML = markup;
      lastTreeMarkup = markup;
      tree.querySelectorAll("details").forEach(group => { group.open = expanded.has(group.dataset.benchGroup); });
      if (focused?.benchSpec) tree.querySelector(`[data-bench-spec="${CSS.escape(focused.benchSpec)}"]`)?.focus({ preventScroll: true });
      if (focused?.benchWork) tree.querySelector(`[data-bench-work="${CSS.escape(focused.benchWork)}"]`)?.focus({ preventScroll: true });
    }
    const spec = byId.get(selectedId);
    const item = workItems?.items.find(item => item.id === (selectedWork ?? spec?.work_item));
    renderTerminalEmpty(spec);
    reviewTab.render(spec ?? null);
    const plan = spec || item ? `${spec && item ? `<p class="bench-crumb">${escapeHtml(workTitle(item))}</p>` : ""}
      <h2>${spec ? `<span class="type-cond bench-spec-id">${escapeHtml(spec.id)}</span> ${escapeHtml(spec.title)}` : escapeHtml(item.id)}</h2>
      ${spec ? nextStep(spec) : ""}
      <section class="bench-traveler" aria-label="Traveler">
        <p class="bench-traveler-head"><span class="type-cond">Traveler</span>${item?.size ? `<span>${escapeHtml(stageLabel(item.size))} work item</span>` : ""}</p>
        ${item ? `<ul class="bench-documents">${documentRow(item, "brd", "Why (BRD)")}${documentRow(item, "prd", "What (PRD)")}</ul>` : '<p class="muted bench-traveler-note">No work item is linked.</p>'}
        ${spec ? specPlan(spec, item) : ""}
      </section>
      ${spec ? specExtras(spec) : ""}`
      : '<p class="muted">Select a spec or work item from the work tree.</p>';
    const planBody = $("#bench-plan-spec");
    if (planBody._markup !== plan) {
      const expanded = new Set([...planBody.querySelectorAll("details[open]")].map(row => row.dataset.criterion));
      planBody.innerHTML = plan;
      planBody._markup = plan;
      planBody.querySelectorAll("details").forEach(row => { row.open = expanded.has(row.dataset.criterion); });
    }
    renderNeeds();
  }

  function renderTerminalEmpty(spec) {
    const start = $("#bench-start-selected");
    if (!start) return;
    start.hidden = !spec;
    start.textContent = spec ? `Start a terminal for ${spec.id} · ${spec.title}` : "";
    start.nextElementSibling.hidden = !start.hidden;
    const actions = start.nextElementSibling.querySelector("[data-first-run]");
    actions.hidden = briefExists() !== false || workItems?.items.length !== 0;
  }

  function specPlan(spec, item) {
    const links = item?.specs.find(row => row.id === spec.id)?.criteria ?? [];
    const proof = workbench.summaryFor(spec)?.sdlc;
    const evidence = proof?.criteria ?? [];
    const stageIndex = STAGES.indexOf(spec.stage);
    const howState = stageIndex > STAGES.indexOf("build") ? "done" : "current";
    const accepted = evidence.filter(row => row.accepted).length;
    const proofLine = proof?.error ? "Proof unavailable" : !proof ? "Checking proof…"
      : `${accepted} of ${spec.criteria.length} ${spec.criteria.length === 1 ? "criterion has" : "criteria have"} accepted proof`;
    // Proof is done only when every criterion has accepted proof; the spec stage never implies it,
    // because gates warn and record but do not block a stage move.
    const allAccepted = Boolean(proof && !proof.error && spec.criteria.length && accepted === spec.criteria.length);
    const proofState = allAccepted ? "done" : ["verify", "review", "ship"].includes(spec.stage) ? "current" : "open";
    const shipState = spec.stage === "ship" ? "done" : "open";
    return `<div class="bench-station" data-station="how" data-state="${howState}">
      <div class="bench-station-row"><h3><button class="text-button" type="button" data-bench-document="${escapeHtml(spec.file)}">How (Spec)</button></h3>
        ${stamp(`how:${spec.id}`, howState, howState === "done" ? "Done" : stageLabel(spec.stage))}</div>
      <label class="bench-stage-picker">Stage <select data-bench-stage aria-label="Stage for spec ${escapeHtml(spec.id)}">${STAGES.map(stage => `<option value="${stage}"${stage === spec.stage ? " selected" : ""}>${stageLabel(stage)}</option>`).join("")}</select></label>
      ${!spec.work_item && linkableWorkItems().length ? '<button type="button" class="secondary-button" data-bench-link>Link to work item</button>' : ""}
      <h3 class="bench-done-when">Done when</h3><ul class="bench-criteria">${spec.criteria.map(criterion => {
        const link = links.find(row => row.number === criterion.index + 1);
        const result = evidence.find(row => row.index === criterion.index);
        const refs = link?.requirements ?? [];
        const labels = refs.map(id => {
          const goal = item.requirements[id]?.goal;
          return `<span>${escapeHtml(id)}</span>${goal ? ` → <span>${escapeHtml(goal)}</span>` : ""}`;
        }).join(" · ");
        const trace = refs.map(id => {
          const requirement = item.requirements[id];
          if (!requirement) return `<li>${escapeHtml(id)} · Missing requirement</li>`;
          const goal = requirement.goal;
          return `<li><strong>${escapeHtml(id)}</strong>${requirement.retired ? " · Retired" : ""} · ${escapeHtml(requirement.text)}
            <br><strong>${escapeHtml(goal)}</strong>${item.goals[goal] ? " · Retired" : ""} · ${escapeHtml(item.goalDetails[goal] ?? "Goal text unavailable")}</li>`;
        }).join("");
        const state = result?.accepted ? "Proof accepted" : result?.freshness === "stale" ? "Stale proof" :
          result?.result === "failed" ? "Check failed" : result?.recorded ? "Proof recorded" :
            proof?.error ? "Proof unavailable" : !proof ? "Checking proof…" : "No proof recorded";
        const tone = result?.accepted ? "pass" : result?.result === "failed" ? "fail"
          : result?.freshness === "stale" ? "hold" : "none";
        return `<li data-proof="${tone}"><span>${escapeHtml(criterion.text)}</span>
          <small>${criterion.done ? "Marked complete" : "Open"} · <span class="bench-proof-word">${state}</span></small>
          ${link?.stale ? '<small role="status">Stale traceability link</small>' : ""}
          ${trace ? `<details data-criterion="${spec.id}:${criterion.index}"><summary class="type-cond">${labels}</summary><ul>${trace}</ul></details>` : '<small class="muted">No requirement linked</small>'}</li>`;
      }).join("")}</ul>
    </div>
    <div class="bench-station" data-station="proof" data-state="${proofState}">
      <div class="bench-station-row"><h3>Proof</h3>${stamp(`proof:${spec.id}`, proofState, proofState === "done" ? "Accepted" : proof && !proof.error && proofState === "current" ? `${accepted} of ${spec.criteria.length}` : "Open")}</div>
      <small class="bench-station-note">${escapeHtml(proofLine)}</small>
    </div>
    <div class="bench-station" data-station="ship" data-state="${shipState}">
      <div class="bench-station-row"><h3>Ship</h3>${stamp(`ship:${spec.id}`, shipState, shipState === "done" ? "Ship" : "Open")}</div>
      <small class="bench-station-note">${shipState !== "done" ? "Reached when the spec stage is Ship." : allAccepted ? "Spec stage is Ship." : "Spec stage is Ship with proof still open."}</small>
    </div>`;
  }

  function specExtras(spec) {
    const tasks = (ui.state?.tasks ?? []).filter(task => task.specId === spec.id);
    return `<h3>Spec tasks</h3>${tasks.length ? `<ul>${tasks.map(task => `<li>${escapeHtml(task.title)} · ${escapeHtml(task.status)}</li>`).join("")}</ul>` : '<p class="muted">No tasks linked to this spec.</p>'}
      <button type="button" class="secondary-button" data-bench-task>New spec task</button>
      ${openSessions(spec).length ? "" : '<button type="button" class="secondary-button" data-bench-start>Start terminal</button>'}`;
  }

  // One obvious next action, only where the host has already resolved the facts behind it:
  // a spec past planning whose criteria still lack accepted proof. Drafting has its own buttons.
  function nextStep(spec) {
    if (!["build", "verify", "review"].includes(spec.stage)) return "";
    const proof = workbench.summaryFor(spec)?.sdlc;
    if (!proof || proof.error) return "";
    const open = spec.criteria.filter(criterion => !(proof.criteria ?? []).find(row => row.index === criterion.index)?.accepted);
    if (!open.length) return "";
    const first = open[0];
    return `<div class="bench-next" role="note"><p>Record proof for criterion ${first.index + 1}<span>${open.length} of ${spec.criteria.length} ${spec.criteria.length === 1 ? "criterion has" : "criteria have"} no accepted proof yet.</span></p>
      <button type="button" class="secondary-button" data-bench-open-review>Open Review</button></div>`;
  }

  // Stamps mark a station's state with a word. A station that becomes done while the panel is
  // open presses its stamp in once; first renders and reduced motion stay still.
  const stampMemory = new Map();
  function stamp(key, state, word, attributes = "") {
    const before = stampMemory.get(key);
    stampMemory.set(key, state);
    const fresh = before !== undefined && before !== "done" && state === "done";
    return `<small class="bench-stamp" data-state="${state}"${fresh ? " data-fresh" : ""}${attributes}>${escapeHtml(word)}</small>`;
  }

  // The state of a work item's BRD or PRD, as the host reports it: content, draft and warnings.
  function documentState(item, kind) {
    const content = item[kind];
    const draft = item[`${kind}Draft`];
    const warnings = item.warnings.filter(warning => kind === "brd"
      ? /^(?:BRD|Small BRD|Duplicate BRD)\b/.test(warning)
      : /^(?:PRD|Small PRD|Requirement|Duplicate PRD)\b/.test(warning));
    const word = content == null ? draft ? "Draft" : "Missing" : warnings.length ? "Warnings" : "Published";
    return { content, draft, warnings, word, state: word === "Published" ? "done" : word === "Missing" ? "open" : "hold" };
  }

  // Five segments under a work item. Why and What are the document states. How, Proof and Ship
  // only aggregate per-spec facts the host resolves: a segment is done when every linked spec has
  // reached it and current when at least one has. Nothing here is inferred beyond that.
  function miniTraveler(item, specs) {
    const known = specs.filter(Boolean);
    const after = stage => spec => STAGES.indexOf(spec.stage) >= STAGES.indexOf(stage);
    const accepted = spec => {
      const proof = workbench.summaryFor(spec)?.sdlc;
      return Boolean(proof && !proof.error && spec.criteria.length
        && spec.criteria.every(c => (proof.criteria ?? []).find(row => row.index === c.index)?.accepted));
    };
    const across = (reached, started) => !known.length ? "open"
      : known.every(reached) ? "done" : known.some(started ?? reached) ? "current" : "open";
    const segments = [
      ["Why", documentState(item, "brd").state],
      ["What", documentState(item, "prd").state],
      ["How", across(after("verify"), () => true)],
      ["Proof", across(accepted, after("verify"))],
      ["Ship", across(after("ship"))],
    ];
    const words = { done: "done", current: "in progress", hold: "needs a decision", open: "not started" };
    return `<span class="bench-mini" role="img" aria-label="${escapeHtml(segments.map(([name, state]) => `${name} ${words[state]}`).join(", "))}">${segments.map(([, state]) => `<i data-state="${state}"></i>`).join("")}</span>`;
  }

  function documentRow(item, kind, title) {
    const { content, warnings, word, state } = documentState(item, kind);
    const actions = content == null ? title : `<button type="button" class="text-button" data-bench-document="${escapeHtml(`work/${item.id}/${kind}.md`)}">${title}</button>`;
    const draft = item[`${kind}Draft`];
    const name = kind.toUpperCase();
    const draftButton = (text) => `<button type="button" class="text-button" data-bench-draft="${kind}" data-work="${escapeHtml(item.id)}">${text}</button>`;
    const canDraft = kind === "brd" || item.brd != null;
    const draftAction = !canDraft ? "" : content == null && !draft
      ? draftButton(`Draft ${name} with agent`)
      : `<details class="bench-document-menu"><summary aria-label="${name} actions" title="${name} actions">⋯</summary>${draftButton(content == null ? "Continue draft with agent" : "Redraft with agent")}</details>`;
    return `<li data-state="${state}"><div>${actions}${stamp(`${kind}:${item.id}`, state, word, ` title="${escapeHtml(warnings.join("\n"))}"`)}</div>
      ${draftAction}
      ${content != null && warnings.length ? `<p class="bench-doc-warnings">${warnings.map(escapeHtml).join("<br>")}</p>` : ""}
      ${draft ? `<button type="button" class="text-button" data-bench-review-draft="${kind}" data-work="${escapeHtml(item.id)}">Review ${name} draft</button>` : ""}</li>`;
  }

  const handling = new Set();
  const handled = new Map();

  function renderNeeds() {
    const waits = hostedSessions().filter(session => ui.waitingTerminals.has(session.id));
    const agentErrors = (ui.state?.sessions ?? []).filter(session => ui.agentErrors.has(session.id));
    const planning = (ui.state?.sessions ?? []).filter(session => session.planningAttention);
    const receiptFor = (session, kind) => session[`${kind}Handled`] ?? handled.get(`${kind}:${session.id}`);
    const specs = workbench.current().specs;
    const attention = specs.flatMap(spec => specSignals(spec, workbench.boardSummaryFor(spec)));
    $("#bench-needs-count").textContent = waits.length + agentErrors.length + attention.length;
    const sessionEntry = (session, kind) => {
      const receipt = kind === "planning" ? receiptFor(session, kind) : null;
      const paths = session.planningAttention?.paths ?? [];
      const key = `${kind}:${session.id}`;
      return `<section class="bench-needs-entry" data-tone="hold"><h3>${kind === "waiting" ? `${escapeHtml(agentDisplayName(session.agent))} · Waiting for you` : `Planning session · ${escapeHtml(session.workItemId)}`}</h3>
        ${kind === "planning" ? session.planningAttention.unavailable ? "<p>Verb could not compare Git status at session end.</p>" :
          `<p>Git changes outside the requested draft</p><ul>${paths.map(path => `<li><code>${escapeHtml(path)}</code> <button class="text-button" type="button" data-bench-open-path="${escapeHtml(path)}" aria-label="Open ${escapeHtml(path)}">Open</button></li>`).join("")}</ul>` : ""}
        <p>${kind === "waiting" ? "Clears when the session stops waiting." : "Clears from your review queue when you mark it handled."}</p>
        <button type="button" class="secondary-button" data-bench-open-session="${escapeHtml(session.id)}">Open session</button>
        ${receipt ? `<small>Handled by ${escapeHtml(receipt.actor)} · ${escapeHtml(new Date(receipt.at).toLocaleString())}</small>`
          : kind === "planning" ? `<button type="button" class="secondary-button" data-bench-handle="${kind}" data-session="${escapeHtml(session.id)}"${handling.has(key) ? " disabled" : ""}>Mark handled</button>` : ""}
      </section>`;
    };
    const clears = {
      "Failed check": "Clears when the check passes.",
      "Stale proof": "Clears when the proof is refreshed.",
      "Recorded gate warning": "Clears when the next stage move records no warnings.",
    };
    const specEntries = attention.map(signal => `<section class="bench-needs-entry" data-tone="${signal.label === "Failed check" ? "fail" : "hold"}"><h3>${escapeHtml(signal.label)} · ${escapeHtml(signal.specId)} ${escapeHtml(signal.specTitle)}</h3>
      <p>${escapeHtml(signal.detail ?? "")}</p>${signal.warnings ? `<ul>${signal.warnings.map(line => `<li>${escapeHtml(line)}</li>`).join("")}</ul>` : ""}<p>${clears[signal.label]}</p><button class="secondary-button" type="button" data-bench-attention-spec="${escapeHtml(signal.specId)}" data-criterion-index="${signal.criterion ?? ""}">Open Review</button></section>`).join("");
    const paint = (node, markup) => {
      if (node._markup === markup) return;
      const expanded = node.querySelector("[data-board-unlinked]")?.open;
      node.innerHTML = markup;
      node._markup = markup;
      if (expanded) node.querySelector("[data-board-unlinked]")?.setAttribute("open", "");
    };
    const errors = agentErrors.map(session => `<section class="bench-needs-entry" data-tone="fail">
      <h3>${escapeHtml(agentDisplayName(session.agent))} · API or model error</h3>
      <p>Verb saw an error on the terminal screen. Clears when the error leaves the recent screen output or you close the terminal output.</p>
      <button type="button" class="secondary-button" data-bench-open-session="${escapeHtml(session.id)}">Open session</button>
      </section>`).join("");
    const entries = waits.map(session => sessionEntry(session, "waiting")).join("") + errors + specEntries;
    $("#bench-board-attention").hidden = !entries;
    paint($("#bench-board-attention-list"), entries);
    // An empty list only means "nothing waiting" once the host's terminal signals have been read.
    paint($("#bench-needs-list"), entries
      || (ui.observerRead === "read" ? '<p class="muted">Nothing needs your attention.</p>'
        : ui.observerRead === "failed" ? '<p class="muted" role="status">Verb could not read terminal signals yet. It retries every few seconds.</p>'
          : '<p class="muted" role="status">Checking terminals for anything waiting on you…</p>'));
    $("#bench-planning-reports").hidden = !planning.length;
    paint($("#bench-planning-list"), planning.map(session => sessionEntry(session, "planning")).join(""));
    const signals = [...attention, ...waits.map(session => ({ workItemId: specs.find(spec => spec.id === session.specId)?.work_item }))];
    paint($("#bench-board-rows"), boardMarkup({ data: workItems, specs, signals, summaries: workbench.boardSummaryFor, workTitle,
      showArchived: $("#bench-show-archived").checked, error: workError, owner: ui.accessScope === "owner", esc: escapeHtml,
      documents: item => [["Why", "brd"], ["What", "prd"]].map(([name, kind]) => ({ name, ...documentState(item, kind) })) }));
  }

  $("#bench-show-archived").addEventListener("change", renderNeeds);
  $("#bench-plan-spec").addEventListener("change", event => {
    const stage = event.target.closest("[data-bench-stage]");
    const spec = workbench.current().specs.find(row => row.id === selectedId);
    if (!stage || !spec) return;
    const next = stage.value;
    stage.value = spec.stage;
    workbench.openStage(next);
  });

  async function archiveWork(button) {
    button.disabled = true;
    try {
      await api("POST", `/api/work-items/${button.dataset.benchArchive}/archive`, { archived: button.dataset.archiveValue === "true" });
      await refresh();
    } catch (error) {
      toast(error.message, "error");
    } finally {
      button.disabled = false;
    }
  }

  async function markHandled(button) {
    const { session, benchHandle: kind } = button.dataset;
    const key = `${kind}:${session}`;
    if (handling.has(key)) return;
    handling.add(key);
    renderNeeds();
    try {
      handled.set(key, await api("POST", `/api/terminals/${session}/handled`, { kind }));
      await refreshState();
    } catch (error) {
      toast(error.message, "error");
    } finally {
      handling.delete(key);
      renderNeeds();
    }
  }

  async function refresh() {
    const current = ++request;
    try {
      const data = await readWithBusyRetry(api, "/api/work-items");
      if (current !== request) return;
      workItems = data;
      workError = "";
    } catch (error) {
      if (current !== request) return;
      workError = error.message;
    }
    render();
  }

  let drafting;
  let reviewing;
  let reviewGeneration = 0;
  let publishingDraft = false;
  async function draftForm(button) {
    const form = $("#bench-draft-form");
    form.reset();
    const id = button.dataset.work ?? null;
    const item = workItems?.items.find(item => item.id === id);
    const document = button.dataset.benchDraft;
    drafting = { id, document, specId: selectedWork || !id ? null : selectedId };
    $("#bench-draft-title").textContent = `Draft ${document.toUpperCase()} with agent`;
    form.querySelector("[data-new-work-item]").hidden = Boolean(id);
    form.elements.title.required = !id;
    form.querySelector("[data-draft-size]").hidden = document !== "brd" || Boolean(item?.size);
    if (item?.size) form.elements.size.value = item.size;
    form.elements.agent.replaceChildren();
    form.querySelector("[type=submit]").disabled = true;
    formError(form, "");
    showDialog("bench-draft-dialog");
    const requestedDraft = drafting;
    await loadInstalledAgents();
    if (drafting !== requestedDraft || !form.closest("dialog").open) return;
    form.elements.agent.replaceChildren(...["claude", "codex", "agy", "opencode", "gemini"]
      .filter(agent => agentAvailability(ui.agentCapabilities, agent).available)
      .map(agent => new Option(agentDisplayName(agent), agent)));
    form.querySelector("[type=submit]").disabled = !form.elements.agent.options.length;
    formError(form, form.elements.agent.options.length ? "" : "No planning agent is available on this host.");
  }

  function formError(form, message) {
    const error = form.querySelector("[data-draft-error]");
    error.textContent = message;
    error.hidden = !message;
  }

  async function submit(form, action) {
    const button = form.querySelector("[type=submit]");
    button.disabled = true;
    formError(form, "");
    try { await action(); }
    catch (error) { formError(form, error.message); }
    finally { button.disabled = false; }
  }

  $("#bench-draft-form").addEventListener("submit", event => {
    event.preventDefault();
    const form = event.currentTarget;
    const request = { ...drafting, agent: form.elements.agent.value, title: form.elements.title.value || null,
      size: form.elements.size.value, brief: form.elements.brief.value };
    submit(form, async () => {
      const result = await api("POST", "/api/work-items/draft", request);
      closeDialog("bench-draft-dialog");
      if (!request.specId) {
        selectedId = null;
        selectedWork = result.workItemId;
      }
      closeOverlay();
      mountLaunchedSession({ id: result.sessionId, agent: request.agent, role: "plan",
        specId: request.specId, workItemId: result.workItemId, state: "live", hostedHere: true,
        hasTerminal: true, terminal: { running: true, replayAvailable: true } });
      toast([result.message, result.warning, result.contextWarning].filter(Boolean).join("\n"), result.warning || result.contextWarning ? "error" : undefined);
      await refresh();
    });
  });

  async function reviewDraft(button) {
    const path = `/api/work-items/${button.dataset.work}/draft/${button.dataset.benchReviewDraft}`;
    const form = $("#bench-draft-review-form");
    const dialog = form.closest("dialog");
    const generation = ++reviewGeneration;
    const publish = form.querySelector("[type=submit]");
    publish.disabled = true;
    form.reset();
    reviewing = null;
    formError(form, "");
    $("#bench-draft-review-title").textContent = `Review ${button.dataset.benchReviewDraft.toUpperCase()} draft`;
    $("#bench-draft-content").textContent = "Loading draft…";
    $("#bench-draft-stale").hidden = true;
    $("#bench-draft-current").hidden = true;
    $("#bench-draft-difference").hidden = true;
    $("#bench-draft-confirm-stale").hidden = true;
    showDialog(dialog.id);
    try {
      const draft = await readWithBusyRetry(api, path);
      if (generation !== reviewGeneration || !dialog.open) return;
      if (!draft) throw new Error("The draft is no longer available. Refresh Plan.");
      $("#bench-draft-content").textContent = draft.content || "The agent has not written the draft yet.";
      $("#bench-draft-stale").hidden = !draft.stale;
      $("#bench-draft-current").hidden = !draft.stale;
      $("#bench-draft-current").querySelector("pre").textContent = draft.publishedContent ?? "There is no published document.";
      const difference = $("#bench-draft-difference");
      difference.hidden = draft.publishedContent == null;
      if (!difference.hidden) difference.querySelector("pre").textContent = documentDiff(draft.publishedContent, draft.content);
      $("#bench-draft-confirm-stale").hidden = !draft.stale;
      if (draft.content.trim()) reviewing = { path, draft };
      publish.disabled = !reviewing;
    } catch (error) {
      if (generation === reviewGeneration && dialog.open) formError(form, error.message);
    }
  }

  $("#bench-draft-review-dialog").addEventListener("cancel", event => {
    if (publishingDraft) event.preventDefault();
  });
  $("#bench-draft-review-form").addEventListener("submit", event => {
    event.preventDefault();
    const form = event.currentTarget;
    if (!reviewing || publishingDraft) return;
    const { path, draft } = reviewing;
    if (draft.stale && !form.elements.acceptStale.checked) return formError(form, "Confirm you have reviewed the current published document.");
    const dismiss = [...form.querySelectorAll("[data-close]")];
    publishingDraft = true;
    dismiss.forEach(button => { button.disabled = true; });
    submit(form, async () => {
      const result = await api("POST", path, { expectedRevision: draft.publishedRevision,
        draftRevision: draft.revision, acceptStale: form.elements.acceptStale.checked });
      closeDialog("bench-draft-review-dialog");
      toast([ "Published.", ...(result.warnings ?? []), result.draftNotice, result.publicationNotice ].filter(Boolean).join("\n"));
      await refresh();
    }).finally(() => {
      publishingDraft = false;
      dismiss.forEach(button => { button.disabled = false; });
    });
  });

  function syncLayout() {
    const terminalView = ui.view === "sessions";
    const navVisible = narrow.matches ? mobileSurface === "navigation" : !ui.navigationCollapsed;
    const detailsVisible = terminalView && (narrow.matches ? mobileSurface === "details" : !detailsCollapsed);
    app.classList.toggle("bench-details-open", detailsVisible);
    app.dataset.benchOverlay = narrow.matches ? mobileSurface ?? "" : "";
    $("#bench-navigation").hidden = false;
    toggleNavigation.hidden = false;
    toggleDetails.hidden = !terminalView;
    $("#bench-new-terminal").hidden = false;
    $("#bench-terminal-tabs").hidden = !terminalView;
    if (!terminalView) $("#bench-terminal-menu").hidden = true;
    toggleNavigation.setAttribute("aria-expanded", String(navVisible));
    toggleDetails.setAttribute("aria-expanded", String(detailsVisible));
    panel.hidden = !detailsVisible;
    reviewTab.setActive(detailsVisible && activeTab === "review");
    studio.mountPreview(terminalView ? $("#bench-preview-mount") : null,
      detailsVisible && activeTab === "preview");
    $("#bench-right-resize").hidden = !detailsVisible || narrow.matches;
    $(".bench-overlay-scrim").hidden = !app.dataset.benchOverlay;
    // Keep the terminal mounted while an overlay owns the narrow layout.
    $("main.main-content").inert = Boolean(app.dataset.benchOverlay);
    requestAnimationFrame(fitTerminals);
  }

  function showDetails(tab) {
    if (ui.view !== "sessions") showView("sessions");
    if (narrow.matches) mobileSurface = "details";
    else detailsCollapsed = false;
    selectTab(tab);
    syncLayout();
  }

  function filesMode(open) {
    $("#bench-files-mode").setAttribute("aria-pressed", String(open));
    $("#bench-files-panel").hidden = !open;
    if (open) refreshFiles();
  }

  $("#bench-files-mode").addEventListener("click", event =>
    filesMode(event.currentTarget.getAttribute("aria-pressed") !== "true"));

  function selectTab(tab) {
    if (tab === "review" && activeTab !== tab) reviewTab.refreshChecks();

    activeTab = tab;
    reviewTab.setActive(ui.view === "sessions" && !panel.hidden && tab === "review");
    if (tab === "review") render();
    studio.mountPreview(ui.view === "sessions" ? $("#bench-preview-mount") : null,
      !panel.hidden && tab === "preview");
    for (const button of panel.querySelectorAll("[data-bench-tab]")) {
      const active = button.dataset.benchTab === tab;
      button.setAttribute("aria-selected", String(active));
      button.tabIndex = active ? 0 : -1;
      $(`#bench-${button.dataset.benchTab}`).hidden = !active;
    }
  }

  function closeOverlay(restoreFocus = false) {
    const previous = mobileSurface;
    mobileSurface = null;
    syncLayout();
    if (restoreFocus) {
      if (previous === "navigation") toggleNavigation.focus();
      if (previous === "details") toggleDetails.focus();
    }
  }

  toggleNavigation.addEventListener("click", () => {
    if (narrow.matches) {
      mobileSurface = "navigation";
      syncLayout();
      $("#bench-navigation [data-bench-overlay-close]").focus();
    } else {
      ui.navigationCollapsed = !ui.navigationCollapsed;
      app.classList.toggle("navigation-collapsed", ui.navigationCollapsed);
      remember("verb-panel-navigation", ui.navigationCollapsed ? "collapsed" : "expanded");
      syncLayout();
    }
  });
  toggleDetails.addEventListener("click", () => {
    if (narrow.matches) {
      mobileSurface = "details";
      syncLayout();
      panel.querySelector("[data-bench-overlay-close]").focus();
    } else {
      detailsCollapsed = !detailsCollapsed;
      remember("verb.layout.details", detailsCollapsed ? "collapsed" : "expanded");
      syncLayout();
    }
  });
  narrow.addEventListener("change", () => closeOverlay());
  document.addEventListener("keydown", event => {
    if (event.key === "Escape" && mobileSurface && !document.querySelector("dialog[open]")) {
      event.preventDefault();
      closeOverlay(true);
    }
  });
  function startSelectedTerminal(event) {
    if (!event.target.closest("[data-bench-start], #bench-start-selected")) return;
    const specId = selectedId;
    if (!specId) return;
    closeOverlay();
    openLaunch(specId);
  }

  panel.addEventListener("click", startSelectedTerminal);
  $("#terminal-grid").addEventListener("click", startSelectedTerminal);

  document.addEventListener("click", event => {
    const target = event.target;
    if (target.closest("[data-bench-overlay-close]")) return closeOverlay(true);
    const documentButton = target.closest("[data-bench-document]");
    if (documentButton) {
      selectTab("preview");
      return studio.openPreviewFile(documentButton.dataset.benchDocument);
    }
    const draft = target.closest("[data-bench-draft]");
    if (draft) {
      const menu = draft.closest(".bench-document-menu");
      if (menu) menu.open = false;
      return draftForm(draft);
    }
    const review = target.closest("[data-bench-review-draft]");
    if (review) return reviewDraft(review);
    const newSpec = target.closest("[data-bench-new-spec]");
    if (newSpec) {
      event.preventDefault();
      return workbench.openNewSpec({ workItem: newSpec.dataset.benchNewSpec });
    }
    if (target.closest("[data-bench-link]")) {
      return workbench.openLinkSpec(workbench.current().specs.find(spec => spec.id === selectedId));
    }
    const work = target.closest("[data-bench-work]");
    if (work) {
      event.preventDefault();
      selectedId = null;
      selectedWork = work.dataset.benchWork;
      showView("sessions");
      if (narrow.matches) mobileSurface = "details";
      else detailsCollapsed = false;
      selectTab("plan");
      render();
      syncLayout();
      $("#bench-tab-plan").focus();
      return;
    }
    if (target.closest("[data-bench-task]")) return openTask(selectedId);
    if (target.closest("[data-bench-open-review]")) return selectTab("review");
    const spec = target.closest("[data-bench-spec]");
    if (spec) return focusSpec(spec.dataset.benchSpec);
    const tab = target.closest("[data-bench-tab]");
    if (tab) return selectTab(tab.dataset.benchTab);
    const handle = target.closest("[data-bench-handle]");
    if (handle) return markHandled(handle);
    const session = target.closest("[data-bench-open-session]");
    if (session) return openSession(session.dataset.benchOpenSession);
    const file = target.closest("[data-bench-open-path]");
    if (file) return openFile(file.dataset.benchOpenPath);
    const archive = target.closest("[data-bench-archive]");
    if (archive) return archiveWork(archive);
    const attention = target.closest("[data-bench-attention-spec]");
    if (attention) {
      focusSpec(attention.dataset.benchAttentionSpec, "review");
      const index = attention.dataset.criterionIndex;
      if (index) $("#bench-review-body").querySelector(`[data-criterion="${index}"]`)?.closest("li")?.scrollIntoView({ block: "nearest" });
      return;
    }
    if (target.closest("[data-bench-retry]")) {
      refresh();
      workbench.refreshSpecs();
    }
  });
  panel.querySelector('[role="tablist"]').addEventListener("keydown", event => {
    const tabs = ["plan", "review", "preview"].filter(tab => !$("#bench-tab-" + tab).hidden);
    const index = tabs.indexOf(activeTab);
    const next = { ArrowRight: tabs[(index + 1) % tabs.length], ArrowLeft: tabs[(index + tabs.length - 1) % tabs.length], Home: tabs[0], End: tabs.at(-1) }[event.key];
    if (!next) return;
    event.preventDefault();
    selectTab(next);
    $(`#bench-tab-${next}`).focus();
  });
  makeResizer({
    handle: $("#bench-right-resize"), target: app, name: "--bench-right-w",
    key: "verb.layout.detailsWidth", min: 260, max: 580, initial: () => panel.offsetWidth || 360,
    axis: "x", direction: -1, onChange: fitTerminals,
  });
  syncLayout();
  refresh();

  function linkableWorkItems() {
    return (workItems?.items ?? []).filter(item => item.brd && item.prd)
      .map(item => ({ id: item.id, title: workTitle(item) }));
  }

  return {
    linkableWorkItems,
    selectedWorkItem: () => selectedWork ?? workbench.current().specs.find(spec => spec.id === selectedId)?.work_item,
    refresh,
    render,
    focusSpec,
    openReview() { showDetails("review"); },
    openFiles() { showDetails("preview"); filesMode(true); },
    openPreview(path) { showDetails("preview"); return studio.openPreviewFile(path); },
    renderNeeds,
    reviewChecks: reviewTab.setChecks,
    onView() {
      closeOverlay();
      if (["needs-you", "bench-board"].includes(ui.view)) { refreshState(); workbench.refreshSpecs(); }
    },
    terminalChanged(id) {
      if (id === terminalId) return;
      terminalId = id;
      if (!id) return render();
      const session = ui.state?.sessions.find(session => session.id === id);
      selectedId = session?.specId ?? null;
      selectedWork = selectedId ? null : session?.workItemId ?? null;
      render();
    },
  };
}
