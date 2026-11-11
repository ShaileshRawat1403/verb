import { readWithBusyRetry } from "./read-retry.js";
import { makeResizer } from "./resize.js";

export function initBenchShell({ $, api, ui, workbench, showView, selectTerminal, fitTerminals, escapeHtml, agentDisplayName, updateTerminalFocus, openFile, openLaunch }) {
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
    if (selectedId === null && terminalId) {
      selectedId = specs.find(spec => openSessions(spec).some(session => session.id === terminalId))?.id ?? null;
    }
    const byId = new Map(specs.map(spec => [spec.id, spec]));
    const row = (id) => {
      const spec = byId.get(id);
      if (!spec) return "";
      const current = selectedId === id ? ' aria-current="true"' : "";
      return `<li><button type="button" data-bench-spec="${escapeHtml(id)}"${current}><span>${escapeHtml(id)} · ${escapeHtml(spec.title)}</span><small>${escapeHtml(spec.stage)}</small></button></li>`;
    };
    const group = (key, label, ids) =>
      `<details data-bench-group="${escapeHtml(key)}"><summary>${escapeHtml(label)} <span class="nav-count">${ids.length}</span></summary><ul>${ids.map(row).join("")}</ul></details>`;
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
        return group(item.id, title, grouped.get(item.id));
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
    const item = workItems?.items.find(item => item.id === spec?.work_item);
    const startSelected = $("#bench-start-selected");
    startSelected.hidden = !spec || openSessions(spec).length > 0;
    startSelected.textContent = spec ? `Start a terminal for ${spec.id} · ${spec.title}` : "";
    const plan = spec ? `<h2>${escapeHtml(spec.id)} · ${escapeHtml(spec.title)}</h2>
      <p class="bench-spec-stage">${escapeHtml(spec.stage)}</p>
      <ul class="bench-documents">${documentRow(item, "brd", "Why (BRD)")}${documentRow(item, "prd", "What (PRD)")}</ul>
      <h3>Done when</h3><ul>${spec.criteria.map(criterion => `<li>${escapeHtml(criterion.text)}</li>`).join("")}</ul>
      ${openSessions(spec).length ? "" : '<button type="button" class="secondary-button" data-bench-start>Start terminal</button>'}`
      : '<p class="muted">Select a spec from the work tree.</p>';
    const planBody = $("#bench-plan-spec");
    if (planBody._markup !== plan) {
      planBody.innerHTML = plan;
      planBody._markup = plan;
    }
    renderNeeds();
  }

  function documentRow(item, kind, title) {
    const content = item?.[kind];
    if (content == null) return `<li><span>${title}</span><small>Missing</small></li>`;
    const warnings = item.warnings.filter(warning => kind === "brd"
      ? /^(?:BRD|Small BRD|Duplicate BRD)\b/.test(warning)
      : /^(?:PRD|Small PRD|Requirement|Duplicate PRD)\b/.test(warning));
    return `<li><button type="button" class="text-button" data-bench-document="${escapeHtml(`work/${item.id}/${kind}.md`)}">${title}</button><small title="${escapeHtml(warnings.join("\n"))}">${warnings.length ? "Warnings" : "Published"}</small></li>`;
  }

  function renderNeeds() {
    const waiting = hostedSessions().filter(session => ui.waitingTerminals.has(session.id));
    $("#bench-needs-count").textContent = waiting.length;
    const list = $("#bench-needs-list");
    const markup = waiting.length
      ? waiting.map(session => `<button type="button" class="secondary-button" data-bench-waiting="${escapeHtml(session.id)}">${escapeHtml(agentDisplayName(session.agent))} · ${escapeHtml(session.id.slice(0, 8))}</button>`).join("")
      : '<p class="muted">No terminals are waiting for input.</p>';
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
    onView: () => closeOverlay(),
    terminalChanged(id) {
      if (!enabled() || id === terminalId) return;
      terminalId = id;
      selectedId = null;
      render();
    },
  };
}
