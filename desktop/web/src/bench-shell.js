import { readWithBusyRetry } from "./read-retry.js";
import { makeResizer } from "./resize.js";
import { agentAvailability } from "./agent-availability.js";
import { initBenchReview } from "./bench-review.js";
import { documentDiff } from "./document-diff.js";

export function initBenchShell({ $, api, ui, workbench, studio, showView, selectTerminal, openSession, fitTerminals, escapeHtml, agentDisplayName, updateTerminalFocus, openFile, openLaunch, showDialog, closeDialog, mountLaunchedSession, refreshState, refreshChecks, toast }) {
  const reviewTab = initBenchReview({ $, api, workbench, escapeHtml, showDialog, toast, refreshChecks });
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

  const workTitle = item => item.brd?.match(/^#{1,6}[ 	]+(.+?)\s*#*\s*$/m)?.[1]?.replace(/\s+BRD$/i, "").trim()
    || item.id.replace(/^\d+-/, "").replaceAll("-", " ").replace(/^./, letter => letter.toUpperCase());

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
    const group = (key, label, ids, item) => {
      const name = item
        ? `<button class="text-button" type="button" data-bench-work="${escapeHtml(key)}"${key === selectedWork ? ' aria-current="true"' : ""}>${escapeHtml(label)}</button>`
        : escapeHtml(label);
      return `<details data-bench-group="${escapeHtml(key)}"><summary>${name} <span class="nav-count">${ids.length}</span></summary><ul>${ids.map(row).join("")}</ul></details>`;
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

  function renderTerminalEmpty(spec) {
    const start = $("#bench-start-selected");
    if (!start) return;
    start.hidden = !enabled() || !spec;
    start.textContent = spec ? `Start a terminal for ${spec.id} · ${spec.title}` : "";
    start.nextElementSibling.hidden = !start.hidden;
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
    const draftButton = (text) => `<button type="button" class="text-button" data-bench-draft="${kind}" data-work="${escapeHtml(item.id)}">${text}</button>`;
    const canDraft = kind === "brd" || item.brd != null;
    const draftAction = !canDraft ? "" : content == null && !draft
      ? draftButton(`Draft ${name} with agent`)
      : `<details class="bench-document-menu"><summary aria-label="${name} actions" title="${name} actions">⋯</summary>${draftButton(content == null ? "Continue draft with agent" : "Redraft with agent")}</details>`;
    return `<li><div>${actions}<small title="${escapeHtml(warnings.join("\n"))}">${content == null ? draft ? "Draft" : "Missing" : warnings.length ? "Warnings" : "Published"}</small></div>
      ${draftAction}
      ${draft ? `<button type="button" class="text-button" data-bench-review-draft="${kind}" data-work="${escapeHtml(item.id)}">Review ${name} draft</button>` : ""}</li>`;
  }

  const handling = new Set();
  const handled = new Map();

  function renderNeeds() {
    const waiting = hostedSessions().filter(session => ui.waitingTerminals.has(session.id));
    const planning = (ui.state?.sessions ?? []).filter(session => session.planningAttention);
    const receiptFor = (session, kind) => session[`${kind}Handled`] ?? handled.get(`${kind}:${session.id}`);
    const entries = [...waiting.map(session => [session, "waiting"]), ...planning.map(session => [session, "planning"])];
    $("#bench-needs-count").textContent = entries.filter(([session, kind]) => !receiptFor(session, kind)).length;
    const list = $("#bench-needs-list");
    const markup = entries.map(([session, kind]) => {
      const receipt = receiptFor(session, kind);
      const paths = session.planningAttention?.paths ?? [];
      const key = `${kind}:${session.id}`;
      return `<section class="bench-needs-entry"><h3>${kind === "waiting" ? `${escapeHtml(agentDisplayName(session.agent))} · Waiting for you` : `Planning session · ${escapeHtml(session.workItemId)}`}</h3>
        ${kind === "planning" ? session.planningAttention.unavailable ? "<p>Verb could not compare Git status at session end.</p>" :
          `<p>Git changes outside the requested draft</p><ul>${paths.map(path => `<li><code>${escapeHtml(path)}</code> <button class="text-button" type="button" data-bench-open-path="${escapeHtml(path)}" aria-label="Open ${escapeHtml(path)}">Open</button></li>`).join("")}</ul>` : ""}
        <button type="button" class="secondary-button" data-bench-open-session="${escapeHtml(session.id)}">Open session</button>
        ${receipt ? `<small>Handled by ${escapeHtml(receipt.actor)} · ${escapeHtml(new Date(receipt.at).toLocaleString())}</small>`
          : `<button type="button" class="secondary-button" data-bench-handle="${kind}" data-session="${escapeHtml(session.id)}"${handling.has(key) ? " disabled" : ""}>Mark handled</button>`}
      </section>`;
    }).join("") || '<p class="muted">Nothing needs your attention.</p>';
    if (list._markup !== markup) {
      list.innerHTML = markup;
      list._markup = markup;
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
      toast([ "Published.", ...(result.warnings ?? []), result.draftNotice ].filter(Boolean).join(" "));
      await refresh();
    }).finally(() => {
      publishingDraft = false;
      dismiss.forEach(button => { button.disabled = false; });
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
    studio.mountPreview(isBench && terminalView ? $("#bench-preview-mount") : null,
      detailsVisible && activeTab === "preview");
    $("#bench-right-resize").hidden = !detailsVisible || narrow.matches;
    $(".bench-overlay-scrim").hidden = !app.dataset.benchOverlay;
    // Keep the terminal mounted while an overlay owns the narrow layout.
    $("main.main-content").inert = Boolean(app.dataset.benchOverlay);
    requestAnimationFrame(fitTerminals);
  }

  function selectTab(tab) {
    if (tab === "review" && activeTab !== tab) reviewTab.refreshChecks();
    if (tab === "preview" && $("#bench-tab-preview").hidden) return;
    activeTab = tab;
    studio.mountPreview(enabled() && ui.view === "sessions" ? $("#bench-preview-mount") : null,
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

  legacy.addEventListener("change", () => {
    remember("verb.layout.navigation", legacy.checked ? "legacy" : "bench");
    mobileSurface = null;
    syncLayout();
    updateTerminalFocus();
    if (enabled()) { refresh(); render(); }
    else renderTerminalEmpty(null);
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
    if (documentButton) {
      if (!studio.hasPreviewFiles()) return openFile(documentButton.dataset.benchDocument);
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
    const handle = target.closest("[data-bench-handle]");
    if (handle) return markHandled(handle);
    const session = target.closest("[data-bench-open-session]");
    if (session) return openSession(session.dataset.benchOpenSession);
    const file = target.closest("[data-bench-open-path]");
    if (file) return openFile(file.dataset.benchOpenPath);
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
  document.addEventListener("verb:preview-files", () => {
    $("#bench-tab-preview").hidden = !studio.hasPreviewFiles();
    if ($("#bench-tab-preview").hidden && activeTab === "preview") selectTab("plan");
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
  if (enabled()) refresh();

  return {
    enabled,
    refresh: () => { if (enabled()) refresh(); },
    render,
    renderNeeds,
    reviewChecks: reviewTab.setChecks,
    onView() {
      closeOverlay();
      if (ui.view === "needs-you") refreshState();
    },
    terminalChanged(id) {
      if (!enabled() || id === terminalId) return;
      terminalId = id;
      if (!id) return render();
      const session = ui.state?.sessions.find(session => session.id === id);
      selectedId = session?.specId ?? null;
      selectedWork = selectedId ? null : session?.workItemId ?? null;
      render();
    },
  };
}
