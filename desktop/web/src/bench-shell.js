import { readWithBusyRetry } from "./read-retry.js";
import { makeResizer } from "./resize.js";
import { agentAvailability } from "./agent-availability.js";

export function initBenchShell({ $, api, ui, workbench, showView, selectTerminal, fitTerminals, escapeHtml, agentDisplayName, updateTerminalFocus, openFile, openLaunch, showDialog, closeDialog, mountLaunchedSession, refreshState, toast }) {
  const app = $("#app");
  const panel = $("#bench-right-panel");
  const tree = $("#bench-work-tree");
  const toggleNavigation = $("#bench-toggle-navigation");
  const toggleDetails = $("#bench-toggle-right");
  const legacy = $("#bench-legacy-layout");
  const narrow = matchMedia("(max-width: 760px)");
  const preference = (key) => {
    try { return localStorage.getItem(key); } catch { return null; }
  };
  const remember = (key, value) => {
    try { localStorage.setItem(key, value); } catch { /* Optional browser preference. */ }
  };

  legacy.checked = preference("verb.layout.navigation") === "legacy";
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

  const enabled = () => !legacy.checked;
  const hostedSessions = () => (ui.state?.sessions ?? []).filter(session => session.hostedHere && session.terminal?.running);
  const openSessions = (spec) => hostedSessions().filter(session => session.specId === spec.id);
  const focusSpec = (id) => {
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
    render();
    syncLayout();
    if (!running) {
      if (narrow.matches) panel.querySelector("[data-bench-overlay-close]").focus();
      else $("#bench-tab-plan").focus();
    }
  };

  function render() {
    if (!enabled()) return;
    const { specs, error, loaded } = workbench.current();
    if (selectedId === null && !selectedWork && terminalId) {
      selectedId = specs.find(spec => openSessions(spec).some(session => session.id === terminalId))?.id ?? null;
    }
    const byId = new Map(specs.map(spec => [spec.id, spec]));
    const row = (id) => {
      const spec = byId.get(id);
      if (!spec) return "";
      const current = selectedId === id ? ' aria-current="true"' : "";
      return `<li><button type="button" data-bench-spec="${escapeHtml(id)}"${current}><span>${escapeHtml(id)} · ${escapeHtml(spec.title)}</span><small>${escapeHtml(spec.stage)}</small></button></li>`;
    };
    const group = (key, label, ids, item) =>
      `<details data-bench-group="${escapeHtml(key)}"><summary>${escapeHtml(label)} <span class="nav-count">${ids.length}</span></summary><ul>${item ? `<li><button type="button" class="text-button" data-bench-work="${escapeHtml(key)}">Open work item</button></li>` : ""}${ids.map(row).join("")}</ul></details>`;
    let markup;
    if (workError || error) {
      markup = `<p role="alert">${escapeHtml(workError || error)}</p><button type="button" class="text-button" data-bench-retry>Retry</button>`;
    } else if (!workItems || !loaded) {
      markup = '<p role="status">Loading work items…</p>';
    } else {
      const grouped = new Map(workItems.items.map(item => [item.id, []]));
      const unlinked = [];
      for (const spec of specs) (grouped.get(spec.work_item) ?? unlinked).push(spec.id);
      markup = workItems.items.map(item => {
        const title = item.brd?.match(/^#{1,6}[ 	]+(.+?)\s*#*\s*$/m)?.[1]?.replace(/\s+BRD$/i, "").trim()
          || item.id.replace(/^\d+-/, "").replaceAll("-", " ").replace(/^./, letter => letter.toUpperCase());
        return group(item.id, item.draft ? `${title} · Draft` : title, grouped.get(item.id), item);
      }).join("") + group("unlinked", "Unlinked", unlinked);
    }
    if (markup !== lastTreeMarkup) {
      const expanded = new Set([...tree.querySelectorAll("details[open]")].map(group => group.dataset.benchGroup));
      const focused = tree.contains(document.activeElement) ? document.activeElement.closest("[data-bench-spec]")?.dataset.benchSpec : null;
      tree.innerHTML = markup;
      lastTreeMarkup = markup;
      tree.querySelectorAll("details").forEach(group => { group.open = expanded.has(group.dataset.benchGroup); });
      if (focused) [...tree.querySelectorAll("[data-bench-spec]")].find(button => button.dataset.benchSpec === focused)?.focus({ preventScroll: true });
    }
    const spec = byId.get(selectedId);
    const item = workItems?.items.find(item => item.id === (selectedWork ?? spec?.work_item));
    const startSelected = $("#bench-start-selected");
    startSelected.hidden = !spec || openSessions(spec).length > 0;
    startSelected.textContent = spec ? `Start a terminal for ${spec.id} · ${spec.title}` : "";
    const plan = spec || item ? `<h2>${escapeHtml(spec ? `${spec.id} · ${spec.title}` : item.id)}</h2>
      ${item ? `<ul class="bench-documents">${documentRow(item, "brd", "Why (BRD)")}${documentRow(item, "prd", "What (PRD)")}</ul>` : '<p class="muted">No work item is linked.</p>'}
      ${spec ? specPlan(spec, item) : ""}`
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

  function specPlan(spec, item) {
    const links = item?.specs.find(row => row.id === spec.id)?.criteria ?? [];
    const tasks = (ui.state?.tasks ?? []).filter(task => task.specId === spec.id);
    const proof = workbench.summaryFor(spec)?.sdlc;
    const evidence = proof?.criteria ?? [];
    return `<h3><button class="text-button" type="button" data-bench-document="${escapeHtml(spec.file)}">How (Spec)</button> <span class="bench-spec-stage">${escapeHtml(spec.stage)}</span></h3>
      <h3>Done when</h3><ul class="bench-criteria">${spec.criteria.map(criterion => {
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
        return `<li><span>${escapeHtml(criterion.text)}</span>
          <small>${criterion.done ? "Marked complete" : "Open"} · ${state}</small>
          ${link?.stale ? '<small role="status">Stale traceability link</small>' : ""}
          ${trace ? `<details data-criterion="${spec.id}:${criterion.index}"><summary>${labels}</summary><ul>${trace}</ul></details>` : '<small class="muted">No requirement linked</small>'}</li>`;
      }).join("")}</ul>
      <h3>Spec tasks</h3>${tasks.length ? `<ul>${tasks.map(task => `<li>${escapeHtml(task.title)} · ${escapeHtml(task.status)}</li>`).join("")}</ul>` : '<p class="muted">No tasks linked to this spec.</p>'}
      <button type="button" class="secondary-button" data-bench-task>New spec task</button>
      ${openSessions(spec).length ? "" : '<button type="button" class="secondary-button" data-bench-start>Start terminal</button>'}`;
  }

  function documentRow(item, kind, title) {
    const content = item[kind];
    const warnings = item.warnings.filter(warning => kind === "brd"
      ? /^(?:BRD|Small BRD|Duplicate BRD)\b/.test(warning)
      : /^(?:PRD|Small PRD|Requirement|Duplicate PRD)\b/.test(warning));
    const actions = content == null ? title : `<button type="button" class="text-button" data-bench-document="${escapeHtml(`work/${item.id}/${kind}.md`)}">${title}</button>`;
    const draft = item[`${kind}Draft`];
    const name = kind.toUpperCase();
    return `<li><div>${actions}<small title="${escapeHtml(warnings.join("\n"))}">${content == null ? draft ? "Draft" : "Missing" : warnings.length ? "Warnings" : "Published"}</small></div>
      ${kind === "brd" || item.brd != null ? `<button type="button" class="text-button" data-bench-draft="${kind}" data-work="${escapeHtml(item.id)}">${draft ? `Continue ${name} draft` : `Draft ${name} with agent`}</button>` : ""}
      ${draft ? `<button type="button" class="text-button" data-bench-review-draft="${kind}" data-work="${escapeHtml(item.id)}">Review ${name} draft</button>` : ""}</li>`;
  }

  function renderNeeds() {
    const waiting = hostedSessions().filter(session => ui.waitingTerminals.has(session.id));
    const planning = (ui.state?.sessions ?? []).filter(session => session.planningAttention);
    $("#bench-needs-count").textContent = waiting.length + planning.length;
    const list = $("#bench-needs-list");
    const markup = waiting.map(session => `<button type="button" class="secondary-button" data-bench-waiting="${escapeHtml(session.id)}">${escapeHtml(agentDisplayName(session.agent))} · ${escapeHtml(session.id.slice(0, 8))}</button>`).join("")
      + planning.map(({ workItemId, planningAttention }) => `<section><h3>Planning session · ${escapeHtml(workItemId)}</h3>
        ${planningAttention.unavailable ? "<p>Verb could not compare Git status at session end.</p>" : `<p>Git changes outside the requested draft</p><ul>${planningAttention.paths.map(path => `<li><code>${escapeHtml(path)}</code></li>`).join("")}</ul>`}</section>`).join("")
      || '<p class="muted">Nothing needs your attention.</p>';
    if (list._markup !== markup) {
      list.innerHTML = markup;
      list._markup = markup;
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
  let taskSpec;
  function draftForm(button) {
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
    form.elements.agent.replaceChildren(...["claude", "codex", "agy", "opencode", "gemini"]
      .filter(agent => agentAvailability(ui.agentCapabilities, agent).available)
      .map(agent => new Option(agentDisplayName(agent), agent)));
    form.querySelector("[type=submit]").disabled = !form.elements.agent.options.length;
    formError(form, form.elements.agent.options.length ? "" : "No planning agent is available on this host.");
    showDialog("bench-draft-dialog");
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
      toast([result.message, result.warning].filter(Boolean).join(" "), result.warning ? "error" : undefined);
      await refresh();
    });
  });

  async function reviewDraft(button) {
    const path = `/api/work-items/${button.dataset.work}/draft/${button.dataset.benchReviewDraft}`;
    const form = $("#bench-draft-review-form");
    form.reset();
    reviewing = null;
    formError(form, "");
    $("#bench-draft-review-title").textContent = `Review ${button.dataset.benchReviewDraft.toUpperCase()} draft`;
    $("#bench-draft-content").textContent = "Loading draft…";
    $("#bench-draft-stale").hidden = true;
    $("#bench-draft-current").hidden = true;
    $("#bench-draft-confirm-stale").hidden = true;
    showDialog("bench-draft-review-dialog");
    await submit(form, async () => {
      const draft = await readWithBusyRetry(api, path);
      if (!draft) throw new Error("The draft is no longer available. Refresh Plan.");
      $("#bench-draft-content").textContent = draft.content || "The agent has not written the draft yet.";
      $("#bench-draft-stale").hidden = !draft.stale;
      $("#bench-draft-current").hidden = !draft.stale;
      $("#bench-draft-current").querySelector("pre").textContent = draft.publishedContent ?? "There is no published document.";
      $("#bench-draft-confirm-stale").hidden = !draft.stale;
      if (draft.content.trim()) reviewing = { path, draft };
    });
    form.querySelector("[type=submit]").disabled = !reviewing;
  }

  $("#bench-draft-review-form").addEventListener("submit", event => {
    event.preventDefault();
    const form = event.currentTarget;
    if (!reviewing) return;
    const { path, draft } = reviewing;
    if (draft.stale && !form.elements.acceptStale.checked) return formError(form, "Confirm you have reviewed the current published document.");
    submit(form, async () => {
      const result = await api("POST", path, { expectedRevision: draft.publishedRevision,
        draftRevision: draft.revision, acceptStale: form.elements.acceptStale.checked });
      closeDialog("bench-draft-review-dialog");
      toast([ "Published.", ...(result.warnings ?? []), result.draftNotice ].filter(Boolean).join(" "));
      await refresh();
    });
  });

  $("#bench-plan-task-form").addEventListener("submit", event => {
    event.preventDefault();
    const form = event.currentTarget;
    submit(form, async () => {
      await api("POST", "/api/tasks", { specId: taskSpec, title: form.elements.title.value, brief: form.elements.brief.value });
      closeDialog("bench-plan-task-dialog");
      await refreshState();
    });
  });

  function syncLayout() {
    const isBench = enabled();
    const terminalView = ui.view === "sessions";
    const navVisible = narrow.matches ? mobileSurface === "navigation" : !ui.navigationCollapsed;
    const detailsVisible = isBench && terminalView && (narrow.matches ? mobileSurface === "details" : !detailsCollapsed);
    app.classList.toggle("bench-layout", isBench);
    app.classList.toggle("bench-details-open", detailsVisible);
    app.dataset.benchOverlay = isBench && narrow.matches ? mobileSurface ?? "" : "";
    $("#bench-navigation").hidden = !isBench;
    toggleNavigation.hidden = !isBench;
    toggleDetails.hidden = !isBench || !terminalView;
    $("#bench-new-terminal").hidden = !isBench;
    $("#bench-terminal-tabs").hidden = !isBench || !terminalView;
    if (!isBench || !terminalView) $("#bench-terminal-menu").hidden = true;
    toggleNavigation.setAttribute("aria-expanded", String(navVisible));
    toggleDetails.setAttribute("aria-expanded", String(detailsVisible));
    panel.hidden = !detailsVisible;
    $("#bench-right-resize").hidden = !detailsVisible || narrow.matches;
    $(".bench-overlay-scrim").hidden = !app.dataset.benchOverlay;
    // Keep the terminal mounted while an overlay owns the narrow layout.
    $("main.main-content").inert = Boolean(app.dataset.benchOverlay);
    requestAnimationFrame(fitTerminals);
  }

  function selectTab(tab) {
    activeTab = tab;
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

  legacy.addEventListener("change", () => {
    remember("verb.layout.navigation", legacy.checked ? "legacy" : "bench");
    mobileSurface = null;
    syncLayout();
    updateTerminalFocus();
    if (enabled()) { refresh(); render(); }
  });
  toggleNavigation.addEventListener("click", () => {
    if (narrow.matches) {
      mobileSurface = "navigation";
      syncLayout();
      $("#bench-navigation [data-bench-overlay-close]").focus();
    } else {
      $("#toggle-navigation").click();
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
  document.addEventListener("click", event => {
    const target = event.target;
    if (target.closest("[data-bench-overlay-close]")) return closeOverlay(true);
    const documentButton = target.closest("[data-bench-document]");
    if (documentButton) return openFile(documentButton.dataset.benchDocument);
    const draft = target.closest("[data-bench-draft]");
    if (draft) return draftForm(draft);
    const review = target.closest("[data-bench-review-draft]");
    if (review) return reviewDraft(review);
    const work = target.closest("[data-bench-work]");
    if (work) {
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
    if (target.closest("[data-bench-task]")) {
      taskSpec = selectedId;
      const form = $("#bench-plan-task-form");
      form.reset();
      formError(form, "");
      return showDialog("bench-plan-task-dialog");
    }
    const spec = target.closest("[data-bench-spec]");
    if (spec) return focusSpec(spec.dataset.benchSpec);
    const tab = target.closest("[data-bench-tab]");
    if (tab) return selectTab(tab.dataset.benchTab);
    const waiting = target.closest("[data-bench-waiting]");
    if (waiting) {
      showView("sessions");
      return selectTerminal(waiting.dataset.benchWaiting);
    }
    if (target.closest("[data-bench-start], #bench-start-selected")) {
      closeOverlay();
      workbench.selectSpec(selectedId, { navigate: false });
      return openLaunch(selectedId);
    }
    if (target.closest("[data-bench-board]")) {
      showView("specs");
      $("#spec-view-board").click();
      $('[data-action="spec-overview"]').click();
    }
    if (target.closest("[data-bench-retry]")) {
      refresh();
      workbench.refreshSpecs();
    }
  });
  panel.querySelector('[role="tablist"]').addEventListener("keydown", event => {
    const tabs = ["plan", "review", "preview"];
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
  if (enabled()) refresh();

  return {
    enabled,
    refresh: () => { if (enabled()) refresh(); },
    render,
    renderNeeds,
    onView() {
      closeOverlay();
      if (ui.view === "needs-you") refreshState();
    },
    terminalChanged(id) {
      if (!enabled() || id === terminalId) return;
      terminalId = id;
      const session = ui.state?.sessions.find(session => session.id === id);
      selectedId = session?.specId ?? null;
      selectedWork = selectedId ? null : session?.workItemId ?? null;
      render();
    },
  };
}
