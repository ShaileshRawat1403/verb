// The observer panel: opt-in, read-only badges about running terminals. Server signals come from
// GET /api/observer; context pressure and wrong-branch are evaluated here (workbench.js). Badges
// never pop up: the top-bar pill shows a count, and the panel opens only when clicked.

import { clientSignals } from "./workbench.js";

const KIND_NAMES = {
  waiting: "Agent waiting for you",
  stuck: "Agent quiet for a long time",
  failing: "Same command failing repeatedly",
  secret: "Possible secret on screen",
  context: "Agent context nearly full",
  branch: "Work on the wrong branch",
};

export function initObserver(deps) {
  const { api, toast, escapeHtml, $ } = deps;
  let state = { enabled: false, muted: [], signals: [] };
  const dismissed = new Set(readJson(sessionStorage, "verb.observer.dismissed", []));
  const stats = readJson(localStorage, "verb.observer.stats", {});

  function readJson(store, key, fallback) {
    try {
      return JSON.parse(store.getItem(key)) ?? fallback;
    } catch {
      return fallback;
    }
  }
  function writeJson(store, key, value) {
    try {
      store.setItem(key, JSON.stringify(value));
    } catch {
      // Dismissals and counts are conveniences; the observer works without them.
    }
  }
  /** Local, per-browser counts so noisy signals can be spotted and tuned. Never sent anywhere. */
  function count(kind, what) {
    stats[kind] ??= { shown: 0, dismissed: 0, acted: 0 };
    stats[kind][what] += 1;
    writeJson(localStorage, "verb.observer.stats", stats);
  }

  function visibleSignals() {
    const wb = deps.extras.workbench?.current?.() ?? {};
    const client = state.enabled
      ? clientSignals({ meters: wb.meters, sessions: deps.ui.state?.sessions ?? [], git: wb.git, spec: wb.spec, muted: state.muted })
      : [];
    return [...state.signals, ...client].filter((s) => !dismissed.has(s.key));
  }

  const seen = new Set();
  function renderPill() {
    const signals = visibleSignals();
    for (const s of signals) {
      if (!seen.has(s.key)) {
        seen.add(s.key);
        count(s.kind, "shown");
      }
    }
    const pill = $("#observer-pill");
    pill.dataset.state = !state.enabled ? "off" : signals.length ? "alert" : "on";
    $("#observer-label").textContent = !state.enabled
      ? "Observer off"
      : signals.length
        ? `${signals.length} to look at`
        : "Observer on";
  }

  function renderPanel() {
    const body = $("#observer-body");
    if (!state.enabled) {
      body.innerHTML = `<p>The observer watches your running terminals and shows a small badge when something needs you. It never pops up and never acts on its own.</p>
        <ul class="observer-promises">
          <li><b>Opt-in</b> for this project only.</li>
          <li><b>Local and read-only</b>: it reads screens and exit codes Verb already has, in memory.</li>
          <li><b>Secrets stay secret</b>: a possible key is reported by kind, never by value.</li>
          <li><b>Explains itself</b>: every badge says why it appeared.</li>
        </ul>
        <p class="muted">Signals: ${Object.values(KIND_NAMES).join(" · ")}.</p>
        <button class="primary-button" type="button" data-observer="enable">Turn on for this project</button>`;
      return;
    }
    const signals = visibleSignals();
    const actionLabel = { "show-terminal": "Show terminal", handoff: "Hand off", "switch-branch": "Switch branch" };
    body.innerHTML = `${
      signals.length
        ? signals
            .map(
              (s) => `<article class="observer-badge" data-kind="${escapeHtml(s.kind)}">
                <div class="badge-title">${escapeHtml(s.title)}</div>
                <details><summary>Why am I seeing this?</summary><p>${escapeHtml(s.why)}</p></details>
                <div class="badge-actions">
                  ${s.action ? `<button class="secondary-button small" type="button" data-observer="act" data-key="${escapeHtml(s.key)}">${actionLabel[s.action] ?? "Open"}</button>` : ""}
                  <button class="text-button" type="button" data-observer="dismiss" data-key="${escapeHtml(s.key)}">Dismiss</button>
                  <button class="text-button" type="button" data-observer="mute" data-kind="${escapeHtml(s.kind)}">Don't show "${escapeHtml(KIND_NAMES[s.kind] ?? s.kind)}"</button>
                </div>
              </article>`,
            )
            .join("")
        : '<p class="muted observer-calm">Nothing needs you right now.</p>'
    }
    ${
      state.muted.length
        ? `<p class="muted">Muted: ${state.muted.map((k) => `<button class="chip-button" type="button" data-observer="unmute" data-kind="${escapeHtml(k)}">${escapeHtml(KIND_NAMES[k] ?? k)} ×</button>`).join(" ")}</p>`
        : ""
    }
    <button class="text-button" type="button" data-observer="disable">Turn the observer off</button>`;
  }

  function render() {
    renderPill();
    if ($("#observer-panel").open) renderPanel();
  }

  async function refresh() {
    try {
      state = await api("GET", "/api/observer");
    } catch {
      return;
    }
    render();
  }
  async function update(body, message) {
    try {
      state = await api("POST", "/api/observer", body);
      if (message) toast(message);
      render();
    } catch (error) {
      toast(error.message, "error");
    }
  }

  function act(signal) {
    count(signal.kind, "acted");
    $("#observer-panel").close();
    const wb = deps.extras.workbench;
    if (signal.action === "show-terminal" && signal.terminal) {
      if (!["specs", "sessions"].includes(deps.ui.view)) deps.showView("sessions");
      deps.selectTerminal?.(signal.terminal);
    } else if (signal.action === "handoff") {
      wb?.openHandoff?.();
    } else if (signal.action === "switch-branch") {
      wb?.switchBranch?.();
    }
  }

  document.addEventListener("click", (event) => {
    if (event.target.closest('[data-action="observer"]')) {
      const panel = $("#observer-panel");
      if (panel.open) return panel.close();
      renderPanel();
      panel.show();
      refresh(); // show what is true now, not as of the last poll
      return;
    }
    if (event.target.closest('[data-action="observer-close"]')) return $("#observer-panel").close();
    const btn = event.target.closest("[data-observer]");
    if (!btn) return;
    const what = btn.dataset.observer;
    const signal = visibleSignals().find((s) => s.key === btn.dataset.key);
    if (what === "enable") update({ enabled: true }, "Observer on for this project. It only shows badges; it never acts.");
    else if (what === "disable") update({ enabled: false }, "Observer off.");
    else if (what === "mute") update({ mute: btn.dataset.kind });
    else if (what === "unmute") update({ unmute: btn.dataset.kind });
    else if (what === "dismiss" && signal) {
      dismissed.add(signal.key);
      count(signal.kind, "dismissed");
      writeJson(sessionStorage, "verb.observer.dismissed", [...dismissed]);
      render();
    } else if (what === "act" && signal) act(signal);
  });

  refresh();
  setInterval(() => !document.hidden && state.enabled && refresh(), 10000);
  setInterval(() => !document.hidden && state.enabled && renderPill(), 5000);
  return { refresh };
}
