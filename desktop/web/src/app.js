import { projectGitPresentation } from "./project-git.js";
import { readApiResponse } from "./api-response.js";
import { readWithBusyRetry } from "./read-retry.js";
import { agentAvailability, customCommandAvailable, isolatedCheckoutAvailability } from "./agent-availability.js";
import { createTerminalStreamView } from "./terminal-stream-view.js";
import { finishTerminalChrome } from "./terminal-ui.js";
import { isActiveSession, hasTerminalReplay, endedSession, sessionWithRuntime } from "./session-state.js";
import { createTerminalTransport } from "./terminal-transport.js";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { SearchAddon } from "@xterm/addon-search";
import { ImageAddon } from "@xterm/addon-image";
import QRCode from "qrcode";
import "@xterm/xterm/css/xterm.css";
import "./fonts.css";
import "./tap-targets.css";
import "./style.css";
import "./workbench.css";
import "./motion.css";
import "./studio.css";
import "./conversation.css";
import { initStudio } from "./studio-ui.js";
import { initBenchTerminals } from "./bench-terminals.js";
import { initBenchShell } from "./bench-shell.js";
import "./bench-shell.css";
import { initWorkbench } from "./specs-ui.js";
import { initProject } from "./project-ui.js";
import { initProjectSwitcher } from "./project-switcher.js";
import "./connections.css";
import { initConnections } from "./connections-ui.js";
import { initHost } from "./host-ui.js";
import { initAsk } from "./ask-ui.js";
import { initObserver } from "./observer-ui.js";
import { icon } from "./icons.js";
import { attachCommandBlocks } from "./blocks.js";
import { attachStream } from "./stream-ui.js";
import { renderMarkdown } from "./workbench.js";
import { makeResizer } from "./resize.js";
import { createTalk, talkTone } from "./talk-ui.js";
import { TERMINAL_THEMES, createThemeController } from "./theme.js";
import {
  takeInputChunk,
  agentDisplayName,
  agentMark,
  escapeHtml,
  filterSessions,
  filterTasks,
  sessionAction,
  stateName,
  taskName,
} from "./view.js";

const $ = (selector) => document.querySelector(selector);
const token =
  location.hash.slice(1) || sessionStorage.getItem("verb-web-token");
if (location.hash && token) {
  sessionStorage.setItem("verb-web-token", token);
  history.replaceState(null, "", location.pathname + location.search);
}

// The spec workbench (specs-ui.js), created once the workspace has loaded.
let workbench = null;
let shell = null;
let benchTerminals = null;
let studio = null;
let connections = null;
// Preview file tree and Context hub (project-ui.js).
let project = null;
// The Host view (host-ui.js): read-only machine health.
let host = null;
// Light/dark theme; open terminals are recoloured in place when it changes.
const theme = createThemeController({
  onChange(resolved) {
    ui?.terminals?.forEach(({ term }) => {
      if (term) term.options.theme = TERMINAL_THEMES[resolved];
    });
  },
});
const terminalTransport = createTerminalTransport({
 token, onOpen: () => terminalStreamView.connected(), onClose: () => terminalStreamView.disconnected(),
 onControl: msg => terminalStreamView.control(msg), onOutput: bytes => terminalStreamView.binary(bytes),
});
const { connectTerminalStream, sendStreamAttach, sendStreamDetach, sendStreamResize, sendStreamSignal, sendStreamInput, sendStreamAck } = terminalTransport;
const terminalStreamView = createTerminalStreamView({
 getUi: () => ui, pollTerminal, markDrawn, hasVisibleText, showSilent,
 refreshState, agentDisplayName, escapeHtml, sendStreamAttach, sendStreamAck,
});

function panelPreference(name) {
  try {
    return localStorage.getItem(`verb-panel-${name}`) === "collapsed";
  } catch {
    return false;
  }
}

const ui = {
  state: null,
  view: "sessions",
  selectedTask: null,
  terminals: new Map(),
  refreshing: null,
  workspaceRefreshing: null,
  sessionRevision: 0,
  toastTimer: null,
  focusedTerminal: null,
  focusProject: null,
  waitingTerminals: new Set(),
  agentErrors: new Set(),
  talks: new Map(),
  waitingTalkSpecs: new Set(),
  talkSpecs: new Set(),
  observerOn: false,
  splitTerminals: false,
  launchingTerminal: false,
  navigationCollapsed: panelPreference("navigation"),
  pairingExpiresAt: null,
  phoneStatus: null,
};

const emptyTerminalTemplate = $("#terminal-grid .terminal-empty").cloneNode(
  true,
);

$("#app").classList.toggle("navigation-collapsed", ui.navigationCollapsed);

document.addEventListener("verb:observer", (event) => {
  const { enabled, signals, asking, agentAttention, read } = event.detail;
  ui.observerOn = enabled;
  ui.observerRead = read;
  ui.waitingTerminals = new Set([
    ...asking,
    ...agentAttention.filter(signal => signal.kind === "agent-question").map(signal => signal.terminal),
    ...(enabled ? signals.filter((s) => s.kind === "waiting" && s.terminal).map((s) => s.terminal) : []),
  ]);
  ui.agentErrors = new Set(agentAttention.filter(signal => signal.kind === "agent-error").map(signal => signal.terminal));
  // Seen from anywhere: the Sessions nav and the browser tab's title.
  updateNeedsYou();
  renderTerminalTabs();
});

// Arrow keys move between terminal tabs, as in any tab list; Home and End jump to the ends.
document.addEventListener("keydown", (event) => {
  const tab = event.target.closest?.(".terminal-tab");
  if (!tab || !["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
  const tabs = [...tab.parentElement.querySelectorAll(".terminal-tab")].filter(button => button.getClientRects().length);
  const i = tabs.indexOf(tab);
  const next =
    event.key === "Home" ? 0 : event.key === "End" ? tabs.length - 1 : (i + (event.key === "ArrowRight" ? 1 : -1) + tabs.length) % tabs.length;
  event.preventDefault();
  ui.focusedTerminal = tabs[next].dataset.tabTerminal;
  updateTerminalFocus();
  renderListResults();
  document.querySelector(`.terminal-tabs [data-tab-terminal="${CSS.escape(ui.focusedTerminal)}"]`)?.focus();
});

/** Mounts the conversation tile for an Antigravity talk (src/talk-ui.js). */
function mountTalk(id, { focus = false } = {}) {
  if (!ui.talks.has(id)) {
    const talk = createTalk(id, {
      api,
      escapeHtml,
      icon,
      toast,
      workbench: () => workbench,
      onChange: () => {
        refreshTalkSignals();
        renderTerminalTabs();
      },
      openTerminal: (sessionId) =>
        mountLaunchedSession({ id: sessionId, agent: "agy", isolated: false, state: "live", hasTerminal: true, terminal: { running: true, replayAvailable: true }, hostedHere: true }),
      remove: removeTalk,
    });
    ui.talks.set(id, talk);
    $("#terminal-grid").append(talk.tile);
  }
  if (focus || !ui.focusedTerminal) {
    ui.splitTerminals = false;
    ui.focusedTerminal = id;
  }
  updateTerminalFocus();
  if (focus) ui.talks.get(id).tile.scrollIntoView({ behavior: "smooth", block: "nearest" });
}

function removeTalk(id) {
  const talk = ui.talks.get(id);
  if (!talk) return;
  talk.dispose();
  talk.tile.remove();
  ui.talks.delete(id);
  refreshTalkSignals();
  updateTerminalFocus();
}

/** Which specs have a talk, and which talks are stopped on a permission ("Needs you"). */
function refreshTalkSignals() {
  ui.talkSpecs = new Set();
  ui.waitingTalkSpecs = new Set();
  let waiting = 0;
  for (const talk of ui.talks.values()) {
    const data = talk.data();
    if (!data?.spec_id) continue;
    ui.talkSpecs.add(data.spec_id);
    if (talkTone(data) === "waiting") {
      ui.waitingTalkSpecs.add(data.spec_id);
      waiting += 1;
    }
  }
  ui.waitingTalks = waiting;
  updateNeedsYou();
  workbench?.render?.();
}

/** The Sessions nav and the tab title count every agent waiting on the person. */
function updateNeedsYou() {
  const waiting = ui.waitingTerminals.size + (ui.waitingTalks ?? 0);
  $('.nav-item[data-view="sessions"]')?.classList.toggle("needs-you", waiting > 0);
  shell?.renderNeeds();
  const base = document.title.replace(/^\(\d+\) Needs you · /, "");
  document.title = waiting ? `(${waiting}) Needs you · ${base}` : base;
}

/** The project's agent-stream opt-in, read once and shared by every terminal. */
const streamSettings = {
  value: null,
  async get() {
    if (this.value === null) {
      try {
        this.value = (await api("GET", "/api/stream")).enabled;
      } catch {
        this.value = false;
      }
    }
    return this.value;
  },
  async set(enabled) {
    this.value = (await api("POST", "/api/stream", { enabled })).enabled;
    return this.value;
  },
};

/** The open agent terminal that should receive a hand-off from `fromId`: the most recent one. */
function agentTerminalFor(fromId) {
  const agents = [...ui.terminals.entries()].filter(
    ([id, t]) => id !== fromId && !t.closed && !t.ended && t.agent && t.agent !== "shell",
  );
  return agents.length ? agents[agents.length - 1][1] : null;
}

function toast(message, tone = "info") {
  const element = $("#toast");
  element.textContent = message;
  element.dataset.tone = tone;
  element.classList.add("visible");
  clearTimeout(ui.toastTimer);
  ui.toastTimer = setTimeout(() => element.classList.remove("visible"), 4600);
}

async function api(method, path, body) {
  const headers = {
    ...(body === undefined ? {} : { "Content-Type": "application/json" }),
  };
  if (token) {
    headers["X-Verb-Token"] = token;
  }
  const response = await fetch(path, {
    method,
    cache: "no-store",
    credentials: "same-origin",
    redirect: "manual",
    headers,
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
  return readApiResponse(response);
}

function showView(destination) {
  if (destination === "specs") destination = "bench-board";
  if (destination === "project" && shell) return shell.openFiles();
  if (destination === "overview" && shell) return shell.openReview();
  const settings = ["host", "connections", "tasks", "phone", "session-history"];
  const section = settings.includes(destination) ? destination : "host";
  const view = settings.includes(destination) ? "settings" : destination === "memory" ? "context" : destination;
  ui.view = view;
  document.querySelectorAll("[data-settings-panel]").forEach(panel => {
    panel.hidden = view !== "settings" || panel.dataset.settingsPanel !== section;
  });
  if (view === "settings" && section === "phone") renderPhoneSettings();
  $("#app").classList.toggle("studio-open", view === "studio");
  document
    .querySelectorAll(".view")
    .forEach((element) =>
      element.classList.toggle("active", element.id === `view-${view}`),
    );
  document.querySelectorAll(".nav-item").forEach((element) => {
    const active = element.dataset.view === view;
    element.classList.toggle("active", active);
    if (active) element.setAttribute("aria-current", "page");
    else element.removeAttribute("aria-current");
  });
  $("#breadcrumb-view").textContent = view[0].toUpperCase() + view.slice(1);
  if (["tasks", "session-history"].includes(destination) || view === "context") refreshState();
  workbench?.onShowView(view);
  project?.onShowView(view);
  host?.onShowView(view === "settings" ? section : view);
  studio?.onShowView(view);
  connections?.onShowView(view === "settings" ? section : view);
  shell?.onView();
  if (shell) renderTerminalTabs();
  benchTerminals?.render();
  if (view === "sessions") {
    requestAnimationFrame(() => {
      syncHostedTerminals();
      fitTerminals();
    });
  }
}

function fitTerminals() {
  ui.terminals.forEach(({ fit, mount }) => {
    if (mount.clientWidth > 0 && mount.clientHeight > 0) fit.fit();
  });
}

function sessionItem(session, compact = false, showDelete = false) {
  const note = session.isolated ? "Isolated checkout" : "Main checkout";
  const name = agentDisplayName(session.agent);
  const btn = `<button class="session-item ${compact ? "compact" : ""} ${session.id === ui.focusedTerminal ? "current" : ""}" type="button" data-session-id="${escapeHtml(session.id)}" ${session.id === ui.focusedTerminal ? 'aria-current="true"' : ""} aria-label="${escapeHtml(`${name} ${session.id.slice(0, 6)} · ${sessionAction(session)}`)}">
    <span class="session-agent-icon">${agentMark(session.agent)}</span>
    <span class="session-copy"><strong>${escapeHtml(name)} <small>${escapeHtml(session.id.slice(0, 6))}</small></strong><span>${escapeHtml(note)} · ${escapeHtml(stateName(session.state))}</span></span>
    ${session.attention ? `<span class="attention-badge">${session.attention}</span>` : ""}
    ${compact ? "" : `<span class="session-next">${escapeHtml(sessionAction(session))}</span>`}
    <span class="row-arrow" aria-hidden="true">${session.id === ui.focusedTerminal ? "•" : "↗"}</span>
  </button>`;
  if (showDelete) {
    return `<div class="session-item-row">${btn}<button class="session-delete-btn" type="button" data-delete-session="${escapeHtml(session.id)}" title="Forget session record" aria-label="Delete session">×</button></div>`;
  }
  return btn;
}

function taskItem(task, compact = false) {
  return `<button class="task-item ${compact ? "compact" : ""} ${ui.selectedTask === task.id ? "selected" : ""}" type="button" data-task-id="${escapeHtml(task.id)}">
    <span class="task-status-dot ${task.needsHelp ? "help" : escapeHtml(task.status.replaceAll(" ", "-"))}"></span>
    <span class="task-copy"><strong>${escapeHtml(task.title)}</strong><span>${escapeHtml(task.needsHelp ? "Help requested" : taskName(task.status))}${task.owner ? ` · ${escapeHtml(sessionLabel(task.owner))}` : " · Unassigned"}</span></span><span class="row-arrow">→</span>
  </button>`;
}

function sessionLabel(id) {
  const session = ui.state?.sessions.find((item) => item.id === id);
  return session
    ? `${agentDisplayName(session.agent)} ${id.slice(0, 6)}`
    : `Session ${id.slice(0, 6)}`;
}

/** An empty list: an icon from the icon set (by name), a title, a line of help, an optional action. */
function emptyState(iconName, title, body, action) {
  return `<div class="list-empty"><span class="empty-glyph">${icon(iconName, { size: 18 })}</span><strong>${title}</strong><p>${body}</p>${action || ""}</div>`;
}

function renderPhoneSettings() {
  const sessions = (ui.state?.sessions ?? []).filter(session => session.hostedHere && session.terminal?.running);
  const owner = ui.accessScope === "owner";
  const list = $("#settings-phone-sessions");
  const markup = !owner ? "<p>Phone pairing requires owner access.</p>" : sessions.length
    ? `<ul>${sessions.map(session => `<li><button class="secondary-button" type="button" data-phone-terminal="${escapeHtml(session.id)}">Pair ${escapeHtml(sessionLabel(session.id))}</button></li>`).join("")}</ul>`
    : "<p>Start a terminal to pair a phone.</p>";
  if (list.dataset.markup !== markup) {
    list.innerHTML = markup;
    list.dataset.markup = markup;
  }
}

function render() {
  if (ui.view === "settings" && !$("#settings-phone").hidden) renderPhoneSettings();
  const state = ui.state;
  if (!state) return;
  const { project, sessions, tasks, memory, deployment } = state;
  if (deployment) {
    $("#deployment-name").textContent = deployment.name;
    $("#deployment-mode").textContent = deployment.mode;
  }
  const activeCount = sessions.filter(isActiveSession).length;
  $("#view-sessions").classList.toggle("has-sessions", activeCount > 0);
  $("#sidebar-project").textContent = project.name || "Project";
  $(".ws-mark").textContent = (project.name || "v").trim().charAt(0).toLowerCase() || "v";
  $(".workspace-head").title =
    project.workspace || project.path || project.name || "Current project";
  const git = projectGitPresentation(project);
  $("#sidebar-branch").textContent = git.branch;
  $("#sidebar-branch").title = git.detail;
  $("#breadcrumb-project").textContent = project.name || "Project";
  $("#top-branch").textContent = git.branch;
  $("#top-branch").title = git.detail;
  $("#top-changes").textContent = git.changes;

  $("#session-count").textContent = activeCount;
  $("#task-count").textContent = tasks.length;
  renderListResults();
  // One note per line (that is how they are appended); each keeps its own inline Markdown.
  const notes = (memory || "").split("\n").filter((line) => line.trim());
  $("#memory-meta").textContent = notes.length
    ? `${notes.length} note${notes.length === 1 ? "" : "s"} · saved with this project`
    : "Saved with this project";
  $("#memory-content").innerHTML = notes.length
    ? `<ol class="note-list">${notes.map((line) => `<li class="md-preview">${renderMarkdown(line, escapeHtml)}</li>`).join("")}</ol>`
    : emptyState(
        "file-text",
        "No shared notes yet",
        "Add one when a decision or constraint should reach every agent, such as “Use pnpm, not npm.”",
        '<button class="secondary-button" type="button" data-action="add-note">Add the first note</button>',
      );
  // With no tasks there is nothing to select: one empty panel, not a list and a detail pane.
  $("#task-layout").classList.toggle("no-tasks", tasks.length === 0);
  if (!tasks.some((task) => task.id === ui.selectedTask))
    ui.selectedTask = tasks[0]?.id || null;
  renderTaskDetail();
  syncHostedTerminals();
  studio?.render();
  shell?.render();
}

function renderListResults() {
  if (!ui.state) return;
  const { sessions, tasks } = ui.state;
  const sessionMatches = filterSessions(sessions, $("#session-search").value);
  const taskMatches = filterTasks(tasks, $("#task-search").value, sessionLabel);

  const historyOpen =
    $("#session-list details")?.open ||
    Boolean($("#session-search").value.trim());
  if (!sessionMatches.length) {
    $("#session-list").innerHTML = emptyState(
      "search",
      sessions.length ? "No matching sessions" : "No sessions yet",
      sessions.length
        ? "Try an agent name, state, or session ID."
        : "Open a Terminal or choose an agent to begin.",
    );
  } else {
    const activeSessions = sessionMatches.filter(isActiveSession);
    const endedSessions = sessionMatches.filter(
      (s) => !activeSessions.includes(s),
    );
    let html = "";
    if (activeSessions.length) {
      html += activeSessions.map((session) => sessionItem(session)).join("");
    } else {
      html = emptyState(
        "terminal",
        "No active sessions",
        "Open a new session, or check History for previous work.",
      );
    }
    if (endedSessions.length) {
      html += `<details class="session-history" ${historyOpen ? "open" : ""}>
        <summary class="session-history-summary">
          <span>History (${endedSessions.length})</span>
          <button class="clear-history-button" type="button" id="clear-ended-button" title="Remove ended session records from ledger">Clear ended</button>
        </summary>
        <div class="session-history-items">
          ${endedSessions.map((session) => sessionItem(session, false, session.state === "ended")).join("")}
        </div>
      </details>`;
    }
    $("#session-list").innerHTML = html;
  }

  $("#task-list").innerHTML = taskMatches.length
    ? taskMatches.map((task) => taskItem(task)).join("")
    : emptyState(
        "search",
        tasks.length ? "No matching tasks" : "No tasks yet",
        tasks.length
          ? "Try a task title, owner, or status."
          : "A task is work that should outlive one agent session: who owns it, where it stands, what was handed off.",
        tasks.length ? "" : '<button class="secondary-button" type="button" data-action="new-task">Create the first task</button>',
      );
}

$("#session-search").addEventListener("input", renderListResults);
$("#task-search").addEventListener("input", renderListResults);

function renderTaskDetail() {
  const task = ui.state?.tasks.find((item) => item.id === ui.selectedTask);
  const panel = $("#task-detail");
  if (!task) {
    panel.innerHTML =
      `<div class="empty-detail"><span class="empty-icon">${icon("list", { size: 20 })}</span><h3>Select a task</h3><p>See its brief, owner, and recorded handoffs here.</p></div>`;
    return;
  }
  const buttons =
    task.status === "done"
      ? ""
      : [
          ...(task.status === "open" || task.status === "needs review"
            ? [
                [
                  "claim",
                  task.status === "needs review" ? "Take review" : "Claim task",
                ],
              ]
            : []),
          ...(task.needsHelp ? [["reply", "Reply to help"]] : []),
          ...(task.status === "active"
            ? [
                ["help", "Ask for help"],
                ["handoff", "Hand off"],
                ["done", "Mark done"],
              ]
            : []),
          ["reassign", "Reassign"],
        ]
          .map(
            ([action, label], index) =>
              `<button class="${index === 0 ? "primary-button" : "secondary-button"}" type="button" data-task-action="${action}">${label}</button>`,
          )
          .join("");
  const history = task.events.length
    ? task.events
        .slice()
        .reverse()
        .map(
          (event) =>
            `<div class="history-item"><span class="history-marker"></span><div><strong>${escapeHtml(event.kind)}</strong><small>${escapeHtml(sessionLabel(event.sessionId))}</small>${event.note ? `<p>${escapeHtml(event.note)}</p>` : ""}</div></div>`,
        )
        .join("")
    : '<p class="muted">No actions recorded yet.</p>';
  panel.innerHTML = `<div class="detail-top"><h2>${escapeHtml(task.title)}</h2><div class="detail-meta"><span class="status-pill ${escapeHtml(task.status.replaceAll(" ", "-"))}">${escapeHtml(task.needsHelp ? "Help requested" : taskName(task.status))}</span><span>${task.owner ? `Owned by ${escapeHtml(sessionLabel(task.owner))}` : "Unassigned"}</span></div></div><div class="detail-section"><h3>Brief</h3><p class="task-brief">${escapeHtml(task.brief || "No brief recorded.")}</p></div><div class="detail-section"><h3>History</h3><div class="history-list">${history}</div></div>${buttons ? `<div class="detail-actions">${buttons}</div>` : ""}`;
}

function removeTerminal(id) {
  sendStreamDetach(id);
  const terminal = ui.terminals.get(id);
  if (!terminal) return;
  terminal.closed = true;
  clearTimeout(terminal.pollTimer);
  clearTimeout(terminal.ackTimer);
  terminal.observer.disconnect();
  terminal.blocks?.dispose();
  terminal.stream?.dispose();
  terminal.term.dispose();
  terminal.tile.remove();
  ui.terminals.delete(id);
}

function selectTerminal(id) {
  ui.splitTerminals = false;
  ui.focusedTerminal = id;
  const terminal = ui.terminals.get(id);
  if (terminal) {
    terminal.tile.classList.remove("has-activity");
  }
  updateTerminalFocus();
  renderListResults();
  terminal?.term.focus();
  pollTerminal(id);
}

function syncHostedTerminals() {
  if (!ui.state || ui.view !== "sessions") return;
  const projectId = ui.state.project.id;
  let savedFocus = null;
  if (ui.focusProject !== projectId) {
    try {
      savedFocus = sessionStorage.getItem(`verb.terminal-focus.${projectId}`);
    } catch {
      // Terminal navigation still works when browser storage is unavailable.
    }
    ui.focusProject = projectId;
  }
  for (const id of ui.terminals.keys()) {
    if (
      !ui.state.sessions.some(
        (session) => session.id === id && hasTerminalReplay(session),
      )
    )
      removeTerminal(id);
  }
  for (const session of ui.state.sessions) {
    if (session.hostedHere && !ui.terminals.has(session.id))
      addTerminal(session);
  }
  // Restore after every pane is mounted: mounting the first pane picks a fallback.
  if (savedFocus && (ui.terminals.has(savedFocus) || ui.talks.has(savedFocus)))
    ui.focusedTerminal = savedFocus;
  updateTerminalFocus();
}

function showDialog(id) {
  const dialog = document.getElementById(id);
  if (!dialog.open) dialog.showModal();
}

function closeDialog(id) {
  document.getElementById(id).close();
}

async function refreshState(silent = false) {
  if (ui.refreshing) {
    if (silent) return;
    try {
      await ui.refreshing;
    } catch {
      // The explicit refresh below still runs after a failed background request.
    }
  }
  const revision = ui.sessionRevision;
  const request = api("GET", "/api/state");
  ui.refreshing = request;
  try {
    const state = await request;
    if (revision !== ui.sessionRevision) return;
    if (JSON.stringify(state) !== JSON.stringify(ui.state)) {
      ui.state = state;
      render();
    }
  } catch (error) {
    if (!silent) toast(error.message, "error");
  } finally {
    if (ui.refreshing === request) ui.refreshing = null;
  }
}

async function refreshWorkspace() {
  if (ui.workspaceRefreshing) return;
  const request = api("GET", "/api/workspace");
  ui.workspaceRefreshing = request;
  const revision = ui.sessionRevision;
  try {
    const workspace = await request;
    ui.workspaceAccessInterrupted = false;
    if (revision !== ui.sessionRevision) return;
    const previous = ui.state;
    const ids = new Set(workspace.sessions.map((session) => session.id));
    const history = (previous?.sessions || [])
      .filter((session) => !ids.has(session.id))
      .map((session) =>
        workspace.terminalStates
          ? sessionWithRuntime(session, workspace.terminalStates[session.id])
          : session.hostedHere
          ? endedSession(session)
          : session,
      );
    const state = {
      ...workspace,
      tasks: previous?.tasks || [],
      memory: previous?.memory || "",
      specSummaries: previous?.specSummaries,
      specError: previous?.specError,
      observedAt: previous?.observedAt,
      project: {
        ...workspace.project,
        gitStatus: previous?.project.gitStatus ?? workspace.project.gitStatus,
        branch: previous?.project.branch ?? null,
        changedFiles: previous?.project.changedFiles ?? null,
      },
      sessions: [...workspace.sessions, ...history],
    };
    if (JSON.stringify(state) !== JSON.stringify(previous)) {
      ui.state = state;
      render();
    }
  } catch (error) {
    if (ui.state && error.code === "access_redirect" && !ui.workspaceAccessInterrupted) {
      ui.workspaceAccessInterrupted = true;
      toast(error.message, "error");
    }
    return error;
  } finally {
    ui.workspaceRefreshing = null;
  }
}

function mountLaunchedSession(session) {
  ui.sessionRevision += 1;
  if (ui.state) {
    ui.state.sessions = [
      session,
      ...ui.state.sessions.filter((item) => item.id !== session.id),
    ];
  }
  addTerminal(session);
  selectTerminal(session.id);
  showView("sessions");
  render();
  refreshState(true);
}

function renderChecks(report) {
  shell?.reviewChecks(report);
}

async function refreshChecks(silent = true, attempt = 0) {
  try {
    const report = await api("GET", "/api/checks");
    // The host reads checks on its own thread; until the first report exists it says so.
    if (report.pending) {
      if (attempt < 40)
        setTimeout(() => refreshChecks(silent, attempt + 1), 750);
      return;
    }
    if (report.error) throw new Error(report.error);
    renderChecks(report);
    if (report.refreshing && attempt < 40)
      setTimeout(() => refreshChecks(true, attempt + 1), 1500);
  } catch (error) {
    if (!silent) toast(error.message, "error");
  }
}

let sessionDetailRequest = 0;
async function activateSession(id, { resume = true } = {}) {
  const session = ui.state?.sessions.find((item) => item.id === id);
  if (!session) return;
  const detailRequest = ++sessionDetailRequest;
  if (hasTerminalReplay(session)) {
    showView("sessions");
    if (!ui.terminals.has(id)) addTerminal(session);
    selectTerminal(id);
    return;
  }
  if (resume && session.canResume) {
    try {
      const result = await api("POST", "/api/terminals", {
        agent: "resume",
        resume_id: id,
      });
      const sessionObj = {
        id: result.sessionId,
        agent: session.agent,
        isolated: session.isolated,
        state: "live",
        hasTerminal: true,
        terminal: { running: true, replayAvailable: true }, hostedHere: true,
      };
      mountLaunchedSession(sessionObj);
    } catch (error) {
      toast(error.message, "error");
    }
    return;
  }
  try {
    const inbox = session.agent === "shell" ? { items: [] } : await api("GET", `/api/inbox/${encodeURIComponent(id)}`);
    if (detailRequest !== sessionDetailRequest) return;
    const name = agentDisplayName(session.agent);
    $("#inbox-title").textContent = `${name} session ${id.slice(0, 8)}`;
    $("#inbox-subtitle").textContent =
      session.state === "interrupted"
        ? "Recovery is unconfirmed. There is no terminal replay."
        : "This session has ended.";
    const items = inbox.items.length
      ? `<p class="inbox-lead">It left these for you:</p>${inbox.items
          .map(
            (item) =>
              `<div class="inbox-item"><span class="inbox-kind">${escapeHtml(item.kind.replaceAll("_", " "))}</span><strong>${escapeHtml(item.title)}</strong></div>`,
          )
          .join("")}`
      : `<p class="inbox-lead">Nothing was left waiting. Its work stays where it was: in your files, in Git, and in the audit trail of any spec it worked on.</p>`;
    const continueArgs = ui.installed?.[session.agent] === false ? null : CONTINUE_ARGS[session.agent];
    const actions = `<div class="inbox-actions">${
      continueArgs
        ? `<button type="button" class="primary-button" data-inbox-launch="${escapeHtml(session.agent)}" data-inbox-continue="1">Continue ${escapeHtml(name)}'s latest conversation</button>`
        : ""
    }<button type="button" class="${continueArgs ? "secondary-button" : "primary-button"}" data-inbox-launch="${escapeHtml(session.agent)}">Start a new ${escapeHtml(name)} session</button></div>${
      continueArgs
        ? `<p class="inbox-note">Continuing picks up ${escapeHtml(name)}'s most recent conversation in this project, which is usually this one.</p>`
        : ""
    }`;
    const notHere = ui.installed?.[session.agent] === false;
    $("#inbox-content").innerHTML =
      items +
      (notHere
        ? `<p class="inbox-note">${escapeHtml(name)} is unavailable in this execution environment, so this conversation cannot be continued here. Ask the host operator to provision it, or continue on the machine where it ran.</p>`
        : actions);
    showView("session-history");
    const detail = $("#session-history-detail");
    detail.hidden = false;
    detail.focus();
  } catch (error) {
    toast(error.message, "error");
  }
}

/** How each agent's own CLI continues its most recent conversation in the current directory. */
const CONTINUE_ARGS = {
  agy: ["-c"],
  claude: ["--continue"],
  codex: ["resume", "--last"],
};

$("#inbox-content")?.addEventListener("click", async (event) => {
  const button = event.target.closest("[data-inbox-launch]");
  if (!button) return;
  const agent = button.dataset.inboxLaunch;
  const args = button.dataset.inboxContinue ? CONTINUE_ARGS[agent] : [];
  $("#session-history-detail").hidden = true;
  try {
    const result = await api("POST", "/api/terminals", { agent, isolated: false, args });
    mountLaunchedSession({
      id: result.sessionId,
      agent,
      isolated: false,
      state: "live",
      hasTerminal: true,
      terminal: { running: true, replayAvailable: true }, hostedHere: true,
    });
  } catch (error) {
    toast(error.message, "error");
  }
});

function addTerminal(session) {
  if (ui.terminals.has(session.id)) return;
  const grid = $("#terminal-grid");
  grid.querySelector(".terminal-empty")?.remove();
  const tile = document.createElement("div");
  tile.className = "terminal-tile";
  tile.id = `terminal-${session.id}`;
  tile.innerHTML = `<div class="terminal-titlebar"><span class="pane-agent" aria-hidden="true">${agentMark(session.agent)}</span><span class="terminal-title">${escapeHtml(agentDisplayName(session.agent))} <small>${escapeHtml(session.id.slice(0, 6))}</small></span><span class="terminal-checkout"${session.isolated ? "" : " hidden"}>Isolated</span><button class="terminal-phone" type="button" data-phone-terminal="${escapeHtml(session.id)}" hidden>Phone</button><button class="terminal-phone" type="button" data-take-terminal="${escapeHtml(session.id)}" hidden>Take back</button><button class="terminal-search-toggle" type="button" data-search-terminal="${escapeHtml(session.id)}" aria-label="Search terminal" title="Search output (Cmd+F / Ctrl+F)">${icon("search", { size: 14 })}</button><button class="terminal-focus" type="button" data-focus-terminal="${escapeHtml(session.id)}" aria-label="Focus terminal" aria-pressed="false" title="Focus this terminal">⤢</button><button class="terminal-close" type="button" data-close-terminal="${escapeHtml(session.id)}" aria-label="End session" title="End this session and stop its running commands">End</button></div><div class="terminal-search-bar" data-search-bar="${escapeHtml(session.id)}" hidden><input type="search" class="search-input" placeholder="Find in terminal…" aria-label="Find text" /><button type="button" class="search-btn search-prev" title="Previous match (Shift+Enter)">↑</button><button type="button" class="search-btn search-next" title="Next match (Enter)">↓</button><button type="button" class="search-btn search-case" title="Match Case" aria-pressed="false">Aa</button><button type="button" class="search-btn search-regex" title="Use Regular Expression" aria-pressed="false">.*</button><button type="button" class="search-btn search-close" title="Close search (Escape)">×</button></div><div class="terminal-mount"></div><p class="terminal-silent" role="status" hidden></p><div class="terminal-bottom"><span class="terminal-state">${terminalTransport.ready ? "Connected to Verb (streaming)" : "Connected to Verb"}</span><span>PTY · ${escapeHtml(session.isolated ? "isolated checkout" : "project checkout")}</span></div><p class="bench-connection" role="status"><span class="bench-reconnecting">Reconnecting…</span><span class="bench-disconnected">Disconnected</span></p>`;
  grid.append(tile);
  const mount = tile.querySelector(".terminal-mount");
  const term = new Terminal({
    cursorBlink: true,
    cursorStyle: "bar",
    fontFamily: '"JetBrains Mono", SFMono-Regular, Menlo, Consolas, monospace',
    fontSize: 13,
    lineHeight: 1.22,
    letterSpacing: 0.2,
    scrollback: 10000,
    allowProposedApi: true,
    scrollOnUserInput: true,
    fastScrollSensitivity: 2,
    windowsMode: false,
    macOptionIsMeta: true,
    macOptionClickForcesSelection: true,
    theme: TERMINAL_THEMES[theme.resolved],
  });
  const fit = new FitAddon();
  term.loadAddon(fit);
  try {
    term.loadAddon(new WebLinksAddon());
  } catch {}
  try {
    const unicode11 = new Unicode11Addon();
    term.loadAddon(unicode11);
    term.unicode.activeVersion = "11";
  } catch {}
  let search = null;
  try {
    search = new SearchAddon();
    term.loadAddon(search);
  } catch {}
  try {
    const imageAddon = new ImageAddon({
      enableSizeReports: true,
      pixelLimit: 16777216,
      sixelSupport: true,
      iipSupport: true,
    });
    term.loadAddon(imageAddon);
  } catch (e) {
    console.warn("Image addon unavailable", e);
  }
  term.open(mount);
  if (mount.clientWidth > 0 && mount.clientHeight > 0) {
    try {
      fit.fit();
    } catch {}
  }
  try {
    const webgl = new WebglAddon();
    webgl.onContextLoss(() => {
      webgl.dispose();
    });
    term.loadAddon(webgl);
  } catch (e) {
    console.warn("WebGL addon unavailable, falling back to DOM renderer", e);
  }
  const terminal = {
    term,
    fit,
    search,
    mount,
    tile,
    agent: session.agent,
    cursor: 0,
    closed: false,
    streaming: false,
    unackedBytes: 0,
    pendingAck: 0,
    ackTimer: null,
    inputBuffer: "",
    sendingInput: false,
    polling: false,
    pollTimer: null,
  };
  ui.terminals.set(session.id, terminal);
  watchSilence(terminal);
  terminal.blocks = attachCommandBlocks(term, {
    copy(text) {
      navigator.clipboard
        .writeText(text)
        .then(() => toast("Copied the command and its output."))
        .catch(() => toast("Could not copy to the clipboard.", "error"));
    },
    canHandOff: () => Boolean(agentTerminalFor(session.id)),
    handOff(note) {
      const target = agentTerminalFor(session.id);
      if (!target) {
        toast("No agent is open to hand this to.", "error");
        return;
      }
      // A bracketed paste through the agent's own input: it lands in its prompt, unsent.
      target.term.paste(note);
      target.term.focus();
      toast(`Pasted into ${agentDisplayName(target.agent)}. Review it, then press Enter to send.`);
    },
  });
  terminal.stream = attachStream(tile, session, terminal, {
    api,
    escapeHtml,
    icon,
    toast,
    agentDisplayName,
    agentMark,
    settings: streamSettings,
    autoOpen: matchMedia("(max-width: 760px)").matches,
    workbench: () => workbench,
  });
  sendStreamAttach(session.id, term.rows, term.cols);

  term.onTitleChange((title) => {
    if (title && !terminal.closed && !terminal.ended) {
      const titleEl = tile.querySelector(".terminal-title");
      if (titleEl) {
        titleEl.innerHTML = `${escapeHtml(title)} <small>${escapeHtml(session.id.slice(0, 6))}</small>`;
      }
    }
  });
  term.onBell(() => {
    tile.classList.add("bell");
    setTimeout(() => tile.classList.remove("bell"), 400);
  });

  // OSC 52 clipboard support (allow-listed base64 copy to clipboard)
  term.parser.registerOscHandler(52, (data) => {
    const parts = data.split(";");
    if (parts.length < 2) return true;
    const b64 = parts[1];
    if (b64 === "?") return true;
    try {
      const bin = atob(b64);
      const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
      const text = new TextDecoder().decode(bytes);
      if (text && navigator.clipboard?.writeText) {
        navigator.clipboard.writeText(text).catch(() => {});
      }
    } catch {}
    return true;
  });

  // OSC 7 working directory tracking
  term.parser.registerOscHandler(7, (data) => {
    let path = data;
    if (path.startsWith("file://")) {
      path = path.replace(/^file:\/\/[^\/]*/, "");
      try {
        path = decodeURIComponent(path);
      } catch {}
    }
    if (path) {
      terminal.cwd = path;
      const checkoutEl = tile.querySelector(".terminal-checkout");
      if (checkoutEl) {
        checkoutEl.title = `Working directory: ${path}`;
      }
      const bottomPty = tile.querySelector(".terminal-bottom span:last-child");
      if (bottomPty) {
        bottomPty.textContent = `PTY · ${path}`;
        bottomPty.title = path;
      }
    }
    return true;
  });

  // OSC 133 / OSC 633 shell integration marks
  const handleShellMark = (data) => {
    const parts = data.split(";");
    const mark = parts[0];
    if (mark === "A") {
      terminal.inCommand = false;
    } else if (mark === "C") {
      terminal.inCommand = true;
    } else if (mark === "D") {
      // Only a finish that follows a start is a command's result. The first prompt reports the
      // status of the shell's startup files, which painted a fresh terminal's tab red.
      const code = parts[1];
      if (terminal.inCommand && code !== undefined && code !== "") {
        terminal.lastCommandExitCode = parseInt(code, 10);
      }
      terminal.inCommand = false;
    }
    if (mark === "C" || mark === "D") renderTerminalTabs();
    // Not consumed: the command blocks (src/blocks.js) read the same marks.
    return false;
  };
  term.parser.registerOscHandler(133, handleShellMark);
  term.parser.registerOscHandler(633, handleShellMark);

  const searchBar = tile.querySelector(
    `[data-search-bar="${CSS.escape(session.id)}"]`,
  );
  const searchInput = searchBar.querySelector(".search-input");
  const searchPrev = searchBar.querySelector(".search-prev");
  const searchNext = searchBar.querySelector(".search-next");
  const searchCase = searchBar.querySelector(".search-case");
  const searchRegex = searchBar.querySelector(".search-regex");
  const searchClose = searchBar.querySelector(".search-close");
  const searchToggle = tile.querySelector(
    `[data-search-terminal="${CSS.escape(session.id)}"]`,
  );

  const searchOptions = {
    caseSensitive: false,
    regex: false,
    incremental: true,
  };

  const runSearch = (forward = true) => {
    const q = searchInput.value;
    if (!q) {
      try {
        search?.clearDecorations?.();
      } catch {}
      return;
    }
    try {
      if (forward) {
        search?.findNext(q, searchOptions);
      } else {
        search?.findPrevious(q, searchOptions);
      }
    } catch {}
  };

  searchInput.addEventListener("input", () => runSearch(true));
  searchInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      runSearch(!e.shiftKey);
    } else if (e.key === "Escape") {
      e.preventDefault();
      closeSearch();
    }
  });

  searchPrev.addEventListener("click", () => runSearch(false));
  searchNext.addEventListener("click", () => runSearch(true));
  searchCase.addEventListener("click", () => {
    searchOptions.caseSensitive = !searchOptions.caseSensitive;
    searchCase.setAttribute("aria-pressed", String(searchOptions.caseSensitive));
    runSearch(true);
  });
  searchRegex.addEventListener("click", () => {
    searchOptions.regex = !searchOptions.regex;
    searchRegex.setAttribute("aria-pressed", String(searchOptions.regex));
    runSearch(true);
  });

  const openSearch = () => {
    searchBar.hidden = false;
    searchToggle.setAttribute("aria-pressed", "true");
    renderTerminalTabs();
    searchInput.focus();
    searchInput.select();
    if (searchInput.value) runSearch(true);
  };
  const closeSearch = () => {
    searchBar.hidden = true;
    searchToggle.setAttribute("aria-pressed", "false");
    renderTerminalTabs();
    try {
      search?.clearDecorations?.();
    } catch {}
    term.focus();
  };
  searchClose.addEventListener("click", closeSearch);
  searchToggle.addEventListener("click", () => {
    if (searchBar.hidden) openSearch();
    else closeSearch();
  });

  const adjustZoom = (delta) => {
    const current = term.options.fontSize || 13;
    const next = delta === 0 ? 13 : Math.min(28, Math.max(9, current + delta));
    if (next !== current) {
      term.options.fontSize = next;
      fit.fit();
      sendStreamResize(session.id, term.rows, term.cols);
    }
  };

  // In a full-screen app that has not asked for the mouse, xterm turns the wheel into arrow keys.
  // On a scrolling page that silently moved an agent's selection: scrolling past Antigravity's
  // "trust this folder?" prompt changed its answer to "No, exit" (found in a user test). Here the
  // wheel scrolls the page instead; apps that ask for the mouse still get it.
  //
  // An app that does take the mouse (opencode, for one) redraws its whole screen per wheel step, and
  // a trackpad swipe fires dozens of steps; over a slow link those redraws queued up and scrolling
  // lagged behind the finger. Steps are passed at most every 33 ms, so the app keeps up.
  let lastWheel = 0;
  term.attachCustomWheelEventHandler(() => {
    if (term.buffer.active.type !== "alternate") return true;
    if (term.modes.mouseTrackingMode === "none") return false;
    const now = performance.now();
    if (now - lastWheel < 33) return false;
    lastWheel = now;
    return true;
  });

  term.attachCustomKeyEventHandler((event) => {
    const isMac = navigator.platform.includes("Mac");
    const mod = isMac ? event.metaKey : event.ctrlKey;

    if (mod && event.code === "KeyC" && term.hasSelection()) {
      if (event.type === "keydown") {
        navigator.clipboard.writeText(term.getSelection()).catch(() => {});
      }
      return false;
    }

    if (mod && (event.key === "ArrowUp" || event.key === "ArrowDown") && !event.shiftKey) {
      if (event.type === "keydown") {
        if (event.key === "ArrowUp") terminal.blocks?.previous();
        else terminal.blocks?.next();
      }
      return false;
    }

    if (mod && event.code === "KeyF") {
      if (event.type === "keydown") {
        openSearch();
      }
      return false;
    }

    if (mod && (event.key === "=" || event.key === "+")) {
      if (event.type === "keydown") adjustZoom(1);
      return false;
    }
    if (mod && event.key === "-") {
      if (event.type === "keydown") adjustZoom(-1);
      return false;
    }
    if (mod && event.key === "0") {
      if (event.type === "keydown") adjustZoom(0);
      return false;
    }

    return true;
  });

  mount.addEventListener("paste", (event) => {
    const text = event.clipboardData?.getData("text");
    if (!text) return;
    const lines = text.split(/\r\n|\r|\n/);
    if (lines.length > 1 && term.buffer?.active?.type !== "alternate") {
      event.preventDefault();
      event.stopPropagation();
      promptMultiLinePaste(session.id, text, lines.length);
    }
  });

  mount.addEventListener("contextmenu", async () => {
    if (term.hasSelection()) {
      try {
        await navigator.clipboard.writeText(term.getSelection());
        toast("Selection copied to clipboard.");
      } catch {}
    }
  });

  const sendInputBatch = async () => {
    if (
      !terminal.inputBuffer ||
      terminal.closed ||
      terminal.ending ||
      terminal.ended ||
      terminal.sendingInput
    )
      return;
    terminal.sendingInput = true;
    const { chunk: payload, rest } = takeInputChunk(terminal.inputBuffer);
    terminal.inputBuffer = rest;
    try {
      await api("POST", `/api/terminals/${session.id}/input`, {
        data: payload,
      });
      // Trigger prompt poll to render the typed character echo
      pollTerminal(session.id);
    } catch (error) {
      if (/phone controls input/.test(error.message)) {
        if (
          window.confirm(
            "Your phone controls input for this session. Take it back to this desktop?",
          )
        ) {
          try {
            const result = await api(
              "POST",
              `/api/terminals/${session.id}/control`,
            );
            toast(result.message);
          } catch (takeError) {
            toast(takeError.message, "error");
          }
        }
        return;
      }
      toast(error.message, "error");
    } finally {
      terminal.sendingInput = false;
      if (terminal.inputBuffer) {
        sendInputBatch();
      }
    }
  };

  term.onData((data) => {
    if (terminal.closed || terminal.ended) return;
    if (!sendStreamInput(session.id, data)) {
      terminal.inputBuffer += data;
      sendInputBatch();
    }
  });

  term.onBinary((data) => {
    if (terminal.closed || terminal.ended) return;
    if (!sendStreamInput(session.id, data)) {
      terminal.inputBuffer += data;
      sendInputBatch();
    }
  });
  let resizeTimer;
  const observer = new ResizeObserver(() => {
    clearTimeout(resizeTimer);
    resizeTimer = setTimeout(() => {
      if (mount.clientWidth < 30 || mount.clientHeight < 30 || terminal.closed)
        return;
      fit.fit();
      // The WebGL renderer can keep stale glyph tiles after a resize or after being hidden;
      // a full redraw costs one frame and only happens once the resize settles.
      term.clearTextureAtlas?.();
      term.refresh(0, term.rows - 1);
      sendStreamResize(session.id, term.rows, term.cols);
      api("POST", `/api/terminals/${session.id}/resize`, {
        rows: term.rows,
        cols: term.cols,
      }).catch(() => {});
    }, 80);
  });
  observer.observe(mount);
  terminal.observer = observer;
  updateTerminalFocus();
  pollTerminal(session.id);
}

/** The status a terminal's tab shows: its words for screen readers, its tone for the dot. */
function terminalTabStatus(terminal, id) {
  if (terminal.ended || terminal.closed) return { tone: "ended", words: "ended" };
  if (ui.waitingTerminals.has(id)) return { tone: "waiting", words: "waiting for you to answer" };
  if (terminal.inCommand) return { tone: "running", words: "running a command" };
  if (terminal.lastCommandExitCode > 0)
    return { tone: "failed", words: `last command failed (exit ${terminal.lastCommandExitCode})` };
  return { tone: "ok", words: "ready" };
}

function renderTerminalTabs() {
  const grid = $("#terminal-grid");
  let strip = document.querySelector(".terminal-tabs");
  if (!strip) {
    // Not a div: the grid's CSS uses `.terminal-tile:only-of-type` to mean "the only terminal".
    strip = document.createElement("nav");
    strip.className = "terminal-tabs";
    strip.setAttribute("role", "tablist");
    strip.setAttribute("aria-label", "Terminals");
    grid.prepend(strip);
  }
  const tabMount = $("#bench-terminal-tabs");
  if (strip.parentElement !== tabMount) tabMount.prepend(strip);
  strip.hidden = ui.terminals.size === 0 && ui.talks.size === 0;
  const menu = $("#bench-terminal-menu");
  $("#bench-terminal-id").textContent = ui.focusedTerminal ? `Session ${ui.focusedTerminal}` : "";
  const activeTerminal = ui.terminals.get(ui.focusedTerminal);
  menu.hidden = ui.view !== "sessions" || !activeTerminal;
  if (menu.hidden) menu.open = false;
  menu.querySelector('[data-bench-terminal-action="search"]').setAttribute("aria-pressed",
    String(activeTerminal?.tile.querySelector(".terminal-search-bar").hidden === false));
  menu.querySelector('[data-bench-terminal-action="end"]').textContent = activeTerminal?.ended ? "Close terminal output" : "End session";
  const chat = menu.querySelector('[data-bench-terminal-action="chat"]');
  chat.hidden = !activeTerminal?.stream;
  chat.textContent = activeTerminal?.tile.classList.contains("stream-mode") ? "Return to terminal" : "Open chat";
  for (const action of ["phone", "take"]) {
    menu.querySelector(`[data-bench-terminal-action="${action}"]`).hidden =
      activeTerminal?.tile.querySelector(`[data-${action}-terminal]`)?.hidden !== false;
  }
  const tabs = [...ui.terminals.entries()]
    .map(([id, terminal]) => {
      const status = terminalTabStatus(terminal, id);
      const selected = id === ui.focusedTerminal;
      const session = ui.state?.sessions.find(item => item.id === id);
      const agent = agentDisplayName(terminal.agent);
      const role = session?.role ? session.role[0].toUpperCase() + session.role.slice(1) : "";
      const assignment = session?.specId ? `Spec ${session.specId}${role ? ` · ${role}` : ""}`
        : session?.role === "plan" && session.workItemId ? `Work item ${session.workItemId.split("-")[0]} · Plan` : "Scratch";
      const shortAgent = terminal.agent === "claude" ? "Claude" : terminal.agent === "agy" ? "AGY" : agent;
      const title = `${agent} ${assignment}: ${status.words}`;
      const details = session?.specId
        ? `<span class="bench-tab-spec-prefix">Spec </span><span class="type-cond bench-tab-spec">${escapeHtml(session.specId)}</span>${role ? ` · ${escapeHtml(role)}` : ""}`
        : escapeHtml(assignment);
      return `<button type="button" role="tab" class="terminal-tab" data-tab-terminal="${escapeHtml(id)}" aria-selected="${selected}" tabindex="${selected ? 0 : -1}" title="${escapeHtml(title)}" aria-label="${escapeHtml(title)}"><i class="tab-dot" data-tone="${status.tone}" aria-hidden="true"></i><span class="bench-tab-agent">${escapeHtml(agent)}</span><span class="bench-tab-short-agent">${escapeHtml(shortAgent)}</span><small>${details}</small>${status.tone === "waiting" ? '<em class="tab-needs">Needs you</em>' : ""}</button>`;
    })
    .join("");
  const talkTabs = [...ui.talks.entries()]
    .map(([id, talk]) => {
      const tone = talkTone(talk.data());
      const words = tone === "running" ? "answering" : tone === "waiting" ? "wants permission: continue in its terminal" : "ready";
      const selected = id === ui.focusedTerminal;
      return `<button type="button" role="tab" class="terminal-tab" data-bench-talk data-tab-terminal="${escapeHtml(id)}" aria-selected="${selected}" tabindex="${selected ? 0 : -1}" title="${escapeHtml(`Antigravity talk: ${words}`)}"><i class="tab-dot" data-tone="${tone}" aria-hidden="true"></i><span>Antigravity</span><small>Talk</small>${tone === "waiting" ? '<em class="tab-needs">Needs you</em>' : ""}</button>`;
    })
    .join("");
  const markup = talkTabs + tabs;
  // State polls should not discard keyboard focus or restart chrome transitions.
  if (strip._markup !== markup) {
    const focused = strip.contains(document.activeElement)
      ? document.activeElement.dataset.tabTerminal
      : null;
    strip.innerHTML = markup;
    strip._markup = markup;
    if (focused) {
      [...strip.querySelectorAll("[data-tab-terminal]")]
        .find((tab) => tab.dataset.tabTerminal === focused)
        ?.focus({ preventScroll: true });
    }
  }
  if (strip._revealedTerminal !== ui.focusedTerminal && strip.clientWidth) {
    strip._revealedTerminal = ui.focusedTerminal;
    const active = strip.querySelector('[aria-selected="true"]');
    if (active) {
      const bounds = strip.getBoundingClientRect();
      const tab = active.getBoundingClientRect();
      if (tab.right > bounds.right) strip.scrollLeft += Math.ceil(tab.right - bounds.right);
      else if (tab.left < bounds.left) strip.scrollLeft -= Math.ceil(bounds.left - tab.left);
    }
  }
}

function updateTerminalFocus() {
  const grid = $("#terminal-grid");
  const primary = ui.focusedTerminal;
  const secondary = [...ui.terminals.keys()].find(id => id !== primary);
  grid.classList.toggle("bench-split", Boolean(ui.splitTerminals && secondary));
  const talkVisible = ui.talks.size > 0;
  if (!ui.terminals.size && !talkVisible && !grid.querySelector(".terminal-empty"))
    grid.append(emptyTerminalTemplate.cloneNode(true));
  if (talkVisible) grid.querySelector(".terminal-empty")?.remove();
  if (!ui.terminals.has(ui.focusedTerminal) && !(talkVisible && ui.talks.has(ui.focusedTerminal)))
    ui.focusedTerminal = ui.terminals.keys().next().value || (talkVisible ? ui.talks.keys().next().value : null);
  if (ui.focusProject) {
    try {
      const key = `verb.terminal-focus.${ui.focusProject}`;
      if (ui.focusedTerminal) sessionStorage.setItem(key, ui.focusedTerminal);
      else sessionStorage.removeItem(key);
    } catch {
      // Storage denial must not prevent switching or closing terminals.
    }
  }
  ui.talks.forEach(({ tile }, id) => {
    tile.classList.toggle("focused", id === ui.focusedTerminal && !ui.splitTerminals);
  });
  grid.classList.toggle(
    "focused",
    Boolean(ui.focusedTerminal) && !ui.splitTerminals,
  );
  ui.terminals.forEach(({ tile, fit, mount }, id) => {
    const focused = id === ui.focusedTerminal && !ui.splitTerminals;
    tile.classList.toggle("focused", focused);
    tile.classList.toggle("bench-split-secondary", Boolean(ui.splitTerminals && id === secondary));
    tile.classList.toggle("bench-split-active", Boolean(ui.splitTerminals && id === primary));
    tile.classList.toggle("bench-split-hidden", Boolean(ui.splitTerminals && id !== primary && id !== secondary));
    // The tab strip chooses and splits terminals now; the old per-pane button stays hidden.
    tile.querySelector("[data-focus-terminal]").hidden = true;
    requestAnimationFrame(() => {
      if (mount.clientWidth > 0 && mount.clientHeight > 0) fit.fit();
    });
  });
  renderTerminalTabs();
  shell?.terminalChanged(ui.focusedTerminal);
  benchTerminals?.render();
}

async function pollTerminal(id) {
  const terminal = ui.terminals.get(id);
  if (!terminal || terminal.closed || terminal.ending) return;
  if (terminal.streaming) return;
  if (terminal.polling) {
    terminal.pollAgain = true;
    return;
  }
  clearTimeout(terminal.pollTimer);
  const startedAt = performance.now();
  terminal.polling = true;
  let receivedBytes = 0;
  try {
    const output = await api(
      "GET",
      `/api/terminals/${id}/output?after=${terminal.cursor}`,
    );
    if (terminal.closed || terminal.ending || terminal.streaming) return;
    const bytes = Uint8Array.from(atob(output.data), (character) =>
      character.charCodeAt(0),
    );
    receivedBytes = bytes.length;
    if (output.reset) terminal.term.reset();
    if (bytes.length) {
      markDrawn(terminal);
      await new Promise((resolve) => terminal.term.write(bytes, resolve));
    }
    if (terminal.closed || terminal.ending || terminal.streaming) return;
    terminal.cursor = output.cursor;
    if (output.running) {
      delete terminal.tile.querySelector(".terminal-state").dataset.connection;
      terminal.tile.querySelector(".terminal-state").dataset.tone = "";
      terminal.reportedError = false;
      const phoneControls = output.controller === "phone";
      const hasPhone = phoneControls || Boolean(output.phoneConnected);
      terminal.tile.querySelector("[data-phone-terminal]").hidden = !hasPhone;
      terminal.tile.querySelector("[data-take-terminal]").hidden =
        !phoneControls;
      terminal.tile.querySelector(".terminal-state").textContent = phoneControls
        ? "Phone controls input"
        : output.phoneConnected
          ? "Desktop controls input · phone connected"
          : terminal.streaming
            ? "Connected to Verb (streaming)"
            : "Connected to Verb (polling)";
      if (
        $("#phone-dialog").open &&
        $("#phone-dialog").dataset.sessionId === id
      ) {
        if (ui.phoneStatus)
          ui.phoneStatus.connected = Boolean(output.phoneConnected);
        updatePhoneStatus();
      }
    }
    if (!output.running) {
      finishTerminalChrome(terminal, output.error
        ? `Session stopped: ${output.error}`
        : "Session ended");
    }
  } catch (error) {
    if (terminal.closed || terminal.ending || terminal.streaming) return;
    terminal.tile.querySelector(".terminal-state").textContent = "Connection interrupted";
    terminal.tile.querySelector(".terminal-state").dataset.connection = "disconnected";
    terminal.tile.querySelector(".terminal-state").dataset.tone = "warn";
    if (!terminal.reportedError) toast(error.message, "error");
    terminal.reportedError = true;
  } finally {
    terminal.polling = false;
  }
  if (!terminal.closed && !terminal.ended) {
    clearTimeout(terminal.pollTimer);
    const cadence =
      id === ui.focusedTerminal && ui.view === "sessions" && !document.hidden
        ? receivedBytes > 0 || terminal.pollAgain
          ? 0
          : 100
        : 1000;
    terminal.pollAgain = false;
    const delay = Math.max(0, cadence - (performance.now() - startedAt));
    terminal.pollTimer = setTimeout(() => pollTerminal(id), delay);
  }
}

async function openAction(action) {
  const task = ui.state?.tasks.find((item) => item.id === ui.selectedTask);
  if (!task) return;
  const candidates = ui.state.sessions.filter(
    (session) =>
      session.state !== "ended" &&
      ((action !== "reply" && action !== "reassign") ||
        session.id !== task.owner),
  );
  if (!candidates.length)
    return toast(
      "Start or resume an agent session before assigning this action.",
      "error",
    );
  const labels = {
    claim: "Claim task",
    reassign: "Reassign task",
    help: "Ask for help",
    reply: "Reply to help",
    handoff: "Hand off for review",
    done: "Mark task done",
  };
  const descriptions = {
    claim: "Choose the agent session taking ownership.",
    reassign: "Choose the new owner and record why.",
    help: "Record what is blocking this agent.",
    reply: "A different session can respond to the owner.",
    handoff: "Record what the reviewer needs to check.",
    done: "Record the completed result.",
  };
  const form = $("#action-form");
  form.reset();
  form.dataset.action = action;
  form.dataset.taskId = task.id;
  form.dataset.expectedRevision = "";
  $("#action-title").textContent = labels[action];
  $("#action-description").textContent = descriptions[action];
  $("#action-submit").textContent = labels[action];
  const select = $("#action-agent");
  select.innerHTML = candidates
    .map(
      (session) =>
        `<option value="${escapeHtml(session.id)}">${escapeHtml(agentDisplayName(session.agent))} · ${escapeHtml(session.id.slice(0, 8))} · ${escapeHtml(stateName(session.state))}${session.isolated ? " · isolated" : ""}</option>`,
    )
    .join("");
  if (task.owner && candidates.some((session) => session.id === task.owner))
    select.value = task.owner;
  $("#action-note").required = action !== "claim";
  $("#action-note")
    .closest(".field-label")
    ?.classList.toggle("optional", action === "claim");
  if (action === "handoff") {
    try {
      const result = await api("GET", `/api/tasks/${task.id}/handoff-revision`);
      form.dataset.expectedRevision = result.revision;
    } catch (error) {
      return toast(error.message, "error");
    }
  }
  showDialog("action-dialog");
}

async function endTerminal(id) {
  const terminal = ui.terminals.get(id);
  if (!terminal) return;
  terminal.ending = true;
  clearTimeout(terminal.pollTimer);
  let result;
  try {
    // Release retained output too, or a state refresh can reattach the closed pane.
    // This endpoint preserves the durable session record.
    result = await api("DELETE", `/api/terminals/${id}`);
  } catch (error) {
    terminal.ending = false;
    pollTerminal(id);
    throw error;
  }
  ui.sessionRevision += 1;
  removeTerminal(id);
  if (ui.state)
    ui.state.sessions = ui.state.sessions.map((session) =>
      session.id === id
        ? endedSession(session)
        : session,
    );
  render();
  updateTerminalFocus();
  refreshState(true);
  // The pane is closed either way; a warning means its processes may still hold a slot.
  if (result?.warning) showCleanupNotice(id, result.warning);
}

/** Closing a pane is not proof its processes were cleaned up. When the execution boundary has not
 * confirmed cleanup, say so until dismissed rather than in a toast that fades. */
function showCleanupNotice(id, warning) {
  const item = document.createElement("li");
  item.dataset.sessionId = id;
  item.textContent = `${sessionLabel(id)}: ${warning}`;
  $("#cleanup-notice-list").append(item);
  $("#cleanup-notice").hidden = false;
}
$("#cleanup-notice-dismiss").addEventListener("click", () => {
  $("#cleanup-notice-list").replaceChildren();
  $("#cleanup-notice").hidden = true;
});
$("#end-session-confirm").addEventListener("click", async () => {
  const button = $("#end-session-confirm");
  button.disabled = true;
  try {
    const id = $("#end-session-dialog").dataset.sessionId;
    await endTerminal(id);
    closeDialog("end-session-dialog");
    benchTerminals?.offerNote(id, true);
  } catch (error) {
    toast(error.message, "error");
  } finally {
    button.disabled = false;
  }
});

async function launchQuickTerminal() {
  if (ui.launchingTerminal) return;
  ui.launchingTerminal = true;
  const buttons = [
    ...document.querySelectorAll(
      "#quick-terminal-button, #quick-terminal-sessions, #quick-terminal-empty",
    ),
  ];
  const labels = buttons.map((button) => button.textContent);
  buttons.forEach((button) => {
    button.disabled = true;
    button.textContent = "Opening…";
  });
  try {
    const result = await api("POST", "/api/terminals", {
      agent: "shell",
      isolated: false,
      args: [],
    });
    const sessionObj = {
      id: result.sessionId,
      agent: "shell",
      isolated: false,
      state: "live",
      hasTerminal: true,
      terminal: { running: true, replayAvailable: true }, hostedHere: true,
    };
    mountLaunchedSession(sessionObj);
  } catch (error) {
    toast(error.message, "error");
  } finally {
    ui.launchingTerminal = false;
    buttons.forEach((button, index) => {
      button.disabled = false;
      button.textContent = labels[index];
    });
  }
}

document.addEventListener("click", async (event) => {
  const terminalMenu = $("#bench-terminal-menu");
  if (terminalMenu.open && !terminalMenu.contains(event.target)) terminalMenu.open = false;
  const quickTerm = event.target.closest(
    "#quick-terminal-button, #quick-terminal-sessions, #quick-terminal-empty",
  );
  if (quickTerm) return launchQuickTerminal();
  const clearHistory = event.target.closest("#clear-ended-button");
  if (clearHistory) {
    event.preventDefault();
    event.stopPropagation();
    if (
      !confirm(
        "Remove ended session records in this project? Active sessions and agent transcripts are kept.",
      )
    )
      return;
    clearHistory.disabled = true;
    try {
      const res = await api("POST", "/api/sessions/clear-ended");
      toast(`Cleared ${res.cleared} ended session(s).`);
      await refreshState(true);
    } catch (e) {
      toast(e.message, "error");
    } finally {
      clearHistory.disabled = false;
    }
    return;
  }
  const deleteSession = event.target.closest("[data-delete-session]");
  if (deleteSession) {
    const id = deleteSession.dataset.deleteSession;
    if (
      !confirm(
        "Remove this ended session record? The agent transcript is kept.",
      )
    )
      return;
    try {
      await api("DELETE", `/api/sessions/${id}`);
      toast("Session record forgotten.");
      await refreshState(true);
    } catch (e) {
      toast(e.message, "error");
    }
    return;
  }
  const close = event.target.closest("[data-close]");
  if (close) return closeDialog(close.dataset.close);
  const view = event.target.closest("[data-view]");
  if (view) return showView(view.dataset.view);
  const session = event.target.closest("[data-session-id]");
  if (session) return activateSession(session.dataset.sessionId);
  const task = event.target.closest("[data-task-id]");
  if (task) {
    ui.selectedTask = task.dataset.taskId;
    showView("tasks");
    render();
    return;
  }
  const action = event.target.closest("[data-task-action]");
  if (action) return openAction(action.dataset.taskAction);
  const tab = event.target.closest("[data-tab-terminal]");
  if (tab) {
    ui.focusedTerminal = tab.dataset.tabTerminal;
    updateTerminalFocus();
    renderListResults();
    const chosen = ui.terminals.get(ui.focusedTerminal);
    chosen?.term.focus();
    // An agent waiting for an answer: bring its prompt (the bottom of the pane) into view.
    if (chosen && ui.waitingTerminals.has(ui.focusedTerminal)) {
      chosen.term.scrollToBottom();
      chosen.tile.scrollIntoView({ behavior: "smooth", block: "end" });
    }
    return;
  }
  const focus = event.target.closest("[data-focus-terminal]");
  if (focus) {
    ui.splitTerminals =
      ui.focusedTerminal === focus.dataset.focusTerminal && !ui.splitTerminals;
    ui.focusedTerminal = focus.dataset.focusTerminal;
    updateTerminalFocus();
    renderListResults();
    if (ui.focusedTerminal) ui.terminals.get(ui.focusedTerminal)?.term.focus();
    return;
  }
  const take = event.target.closest("[data-take-terminal]");
  if (take) {
    try {
      const result = await api(
        "POST",
        `/api/terminals/${take.dataset.takeTerminal}/control`,
      );
      toast(result.message);
    } catch (error) {
      toast(error.message, "error");
    }
    return;
  }
  const phone = event.target.closest("[data-phone-terminal]");
  if (phone) {
    const id = phone.dataset.phoneTerminal;
    phone.disabled = true;
    try {
      const existing = await api("GET", `/api/terminals/${id}/phone`);
      const result = existing.links
        ? existing
        : await api("POST", `/api/terminals/${id}/phone`);
      $("#phone-dialog").dataset.sessionId = id;
      phone.dataset.sharing = "true";
      if (phone.closest(".terminal-tile")) phone.textContent = "Phone access";
      await showPhoneOffer(result);
      showDialog("phone-dialog");
    } catch (error) {
      toast(error.message, "error");
    } finally {
      phone.disabled = false;
    }
    return;
  }
  const menuAction = event.target.closest("[data-bench-terminal-action]");
  if (menuAction) {
    const menu = $("#bench-terminal-menu");
    menu.open = false;
    menu.querySelector("summary").focus();
    const action = menuAction.dataset.benchTerminalAction;
    const terminal = ui.terminals.get(ui.focusedTerminal);
    if (action === "chat") {
      terminal?.stream?.setMode(terminal.tile.classList.contains("stream-mode") ? "terminal" : "stream");
      return;
    }
    const control = action === "end" ? "close" : action;
    terminal?.tile.querySelector(`[data-${control}-terminal]`)?.click();
    return;
  }
  const terminalClose = event.target.closest("[data-close-terminal]");
  if (terminalClose) {
    const id = terminalClose.dataset.closeTerminal;
    const terminal = ui.terminals.get(id);
    if (!terminal || terminal.ending) return;
    if (terminal.tile.classList.contains("ended")) {
      try {
        await endTerminal(id);
        benchTerminals?.offerNote(id, true);
      } catch (error) {
        toast(error.message, "error");
      }
      return;
    }
    $("#end-session-title").textContent = `End ${sessionLabel(id)}?`;
    $("#end-session-dialog").dataset.sessionId = id;
    showDialog("end-session-dialog");
    return;
  }
  const actionButton = event.target.closest("[data-action]");
  if (actionButton) {
    if (actionButton.dataset.action === "new-session") {
      $("#session-launch-error").hidden = true;
      benchTerminals?.openLaunch();
      updateLaunchForm();
      loadInstalledAgents();
      return;
    }
    if (actionButton.dataset.action === "new-task")
      return openTask();
    if (actionButton.dataset.action === "add-note")
      return showDialog("note-dialog");
    if (actionButton.dataset.action === "refresh-checks")
      return refreshChecks(false);
    if (actionButton.dataset.action === "mark-good") {
      try {
        const result = await api("POST", "/api/good/mark");
        toast(result.message);
        await refreshChecks(false);
      } catch (error) {
        toast(error.message, "error");
      }
      return;
    }
  }
});

async function showPhoneOffer(result) {
  const address = $("#phone-address");
  const chosenHost = address.selectedOptions[0]?.textContent;
  address.replaceChildren();
  for (const link of result.links || []) {
    const option = document.createElement("option");
    option.value = link;
    option.textContent = new URLSearchParams(new URL(link).hash.slice(1)).get(
      "host",
    );
    address.append(option);
  }
  const matching = [...address.options].find(
    (option) => option.textContent === chosenHost,
  );
  if (matching) address.value = matching.value;
  ui.pairingExpiresAt = result.expiresAt;
  ui.phoneStatus = result.status;
  if (address.value) {
    try {
      await renderPhoneLink(address.value);
    } catch {
      $("#phone-link").value = address.value;
    }
  }
  updatePhoneStatus();
}

function updatePhoneStatus() {
  const status = ui.phoneStatus;
  const seconds = Math.max(
    0,
    (ui.pairingExpiresAt || 0) - Math.floor(Date.now() / 1000),
  );
  const ready = Boolean(status?.pairingReady && seconds > 0);
  const text = ready
    ? status?.connected
      ? `New link ready for ${seconds}s · current phone stays connected until replaced`
      : `Ready to pair · link expires in ${seconds}s`
    : status?.connected
      ? "Phone connected · pairing link already used"
      : status?.paired
        ? "Phone paired but offline · use New pairing link for another device"
        : "Pairing link expired · create a new one";
  $("#phone-status-text").textContent = text;
  $("#phone-status").dataset.state = ready
    ? "ready"
    : status?.connected
      ? "connected"
      : "expired";
  $("#phone-copy").disabled = !ready;
  $(".phone-pairing").classList.toggle("unavailable", !ready);
}

async function refreshPhoneStatus() {
  const dialog = $("#phone-dialog");
  if (!dialog.open) return;
  try {
    const result = await api(
      "GET",
      `/api/terminals/${dialog.dataset.sessionId}/phone`,
    );
    if (!result.links) {
      closeDialog("phone-dialog");
      toast("Phone access was stopped.");
      return;
    }
    ui.phoneStatus = result.status;
    ui.pairingExpiresAt = result.expiresAt;
    updatePhoneStatus();
  } catch {
    // The terminal poll reports connection failures.
  }
}

async function renderPhoneLink(link) {
  $("#phone-link").value = link;
  $("#phone-qr").src = await QRCode.toDataURL(link, { width: 240, margin: 1 });
}
$("#phone-address").addEventListener("change", (event) => {
  renderPhoneLink(event.target.value).catch((error) =>
    toast(error.message, "error"),
  );
});
$("#phone-copy").addEventListener("click", async () => {
  if ($("#phone-copy").disabled) return;
  try {
    await navigator.clipboard.writeText($("#phone-link").value);
    toast("Pairing link copied.");
  } catch {
    $("#phone-link").select();
    toast("Select and copy the pairing link.", "error");
  }
});
$("#phone-renew").addEventListener("click", async () => {
  const button = $("#phone-renew");
  button.disabled = true;
  try {
    const id = $("#phone-dialog").dataset.sessionId;
    await showPhoneOffer(await api("POST", `/api/terminals/${id}/phone/renew`));
    toast("New one-use pairing link is ready.");
  } catch (error) {
    toast(error.message, "error");
  } finally {
    button.disabled = false;
  }
});
$("#phone-stop").addEventListener("click", async () => {
  const id = $("#phone-dialog").dataset.sessionId;
  try {
    await api("DELETE", `/api/terminals/${id}/phone`);
    const button = ui.terminals.get(id)?.tile.querySelector("[data-phone-terminal]");
    if (button) {
      button.dataset.sharing = "false";
      button.textContent = "Phone";
    }
    $("#phone-link").value = "";
    ui.phoneStatus = null;
    ui.pairingExpiresAt = null;
    closeDialog("phone-dialog");
    toast("Phone access revoked.");
  } catch (error) {
    toast(error.message, "error");
  }
});

$("#refresh-button").addEventListener("click", () => {
  refreshState();
  shell?.refresh();
});
function updateLaunchForm() {
  const form = $("#session-form");
  const checkout = isolatedCheckoutAvailability(ui.agentCapabilities);
  const isolated = form.elements.isolated;
  isolated.disabled = !checkout.available;
  if (isolated.disabled) isolated.checked = false;
  const checkoutLabel = isolated.closest("label");
  const checkoutDetail = checkoutLabel.querySelector("small");
  checkoutDetail.dataset.originalText ??= checkoutDetail.textContent;
  checkoutDetail.textContent = checkout.reason || checkoutDetail.dataset.originalText;
  checkoutLabel.title = checkout.reason;
  form.querySelectorAll('input[name="agent"]').forEach((input) => {
    const state = agentAvailability(ui.agentCapabilities, input.value);
    input.disabled = !state.available;
    const label = input.closest("label");
    label.classList.toggle("not-installed", input.disabled);
    const detail = label.querySelector("small");
    detail.dataset.originalText ??= detail.textContent;
    detail.textContent = state.reason || detail.dataset.originalText;
    label.title = state.reason;
  });
  if (form.querySelector('input[name="agent"]:checked')?.disabled) {
    form.querySelector('input[value="shell"]').checked = true;
  }
  const agent = form.elements.agent.value;
  const talk = form.elements.talk;
  const talkAvailable = agent === "agy" && !!form.elements.specId.value;
  $("#bench-launch-talk").hidden = !talkAvailable;
  talk.disabled = !talkAvailable;
  if (!talkAvailable) talk.checked = false;
  if (talk.checked) { isolated.checked = false; isolated.disabled = true; }
  $("#bench-launch-role").hidden = talk.checked || ["shell", "custom"].includes(agent);
  $("#custom-command-fields").hidden = agent !== "custom";
  $("#launch-project").textContent =
    ui.state?.project.name || "Current project";
  const deployment = ui.state?.deployment;
  $("#launch-node").textContent =
    deployment?.mode === "REMOTE"
      ? `Runs on ${deployment.name}`
      : "Runs on this computer";
  form.querySelector('[type="submit"]').textContent =
    talk.checked ? "Open Antigravity Talk →" : agent === "custom" ? "Open CLI →" : `Open ${agentDisplayName(agent)} →`;
}
document
  .querySelectorAll('#session-dialog input[name="agent"]')
  .forEach((input) => input.addEventListener("change", () => {
    $("#session-launch-error").hidden = true;
    updateLaunchForm();
  }));

$("#session-form").elements.specId.addEventListener("change", updateLaunchForm);
$("#session-form").elements.talk.addEventListener("change", updateLaunchForm);

function launchError(message) {
  const error = $("#session-launch-error");
  error.textContent = message;
  error.hidden = false;
}

$("#session-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const form = event.currentTarget;
  const agent = form.elements.agent.value;
  const command = form.elements.command.value.trim();
  const isolated = form.elements.isolated.checked;
  const specId = form.elements.specId.value;
  const role = specId && !["shell", "custom"].includes(agent) ? form.elements.role.value : null;
  if (specId && agent === "custom") return launchError("Choose a named agent or terminal for a spec.");
  if (isolated && !isolatedCheckoutAvailability(ui.agentCapabilities).available)
    return launchError(isolatedCheckoutAvailability(ui.agentCapabilities).reason);
  if (!agentAvailability(ui.agentCapabilities, agent).available)
    return launchError("This agent is unavailable in the current execution environment.");
  if (agent === "custom" && !command)
    return launchError("Enter a CLI executable.");
  if (agent === "custom" && !customCommandAvailable(ui.agentCapabilities, command))
    return launchError("Choose one of the approved commands shown above.");
  $("#session-launch-error").hidden = true;
  const button = form.querySelector('[type="submit"]');
  button.disabled = true;
  button.textContent = "Opening…";
  try {
    if (agent === "agy" && specId && form.elements.talk.checked) {
      const { talkId } = await api("POST", `/api/specs/${specId}/talk`);
      closeDialog("session-dialog");
      form.reset();
      showView("sessions");
      mountTalk(talkId, { focus: true });
      await workbench.refreshSpecs();
      return;
    }
    const result = specId
      ? await api("POST", `/api/specs/${specId}/agent`, { agent, role, isolated })
      : await api("POST", "/api/terminals", {
      agent,
      isolated,
      command: agent === "custom" ? command : null,
      args:
        agent === "custom"
          ? form.elements.args.value
              .split("\n")
              .map((item) => item.trim())
              .filter(Boolean)
          : [],
    });
    closeDialog("session-dialog");
    form.reset();
    $("#custom-command-fields").hidden = true;
    const sessionObj = {
      id: result.sessionId,
      specId: specId || null,
      role,
      agent: agent === "custom" ? command || "custom" : agent,
      isolated,
      state: "live",
      hasTerminal: true,
      terminal: { running: true, replayAvailable: true }, hostedHere: true,
    };
    mountLaunchedSession(sessionObj);
    if (result.warning || result.contextWarning) toast(result.warning || result.contextWarning, "error");
  } catch (error) {
    launchError(error.message);
  } finally {
    button.disabled = false;
    updateLaunchForm();
  }
});

function openTask(specId = "") {
  const form = $("#task-form");
  form.reset();
  form.elements.specId.value = specId;
  $("#task-spec-label").hidden = !specId;
  $("#task-spec-label").textContent = specId ? `Linked to spec ${specId}` : "";
  showDialog("task-dialog");
}

$("#task-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const form = event.currentTarget;
  const specId = form.elements.specId.value;
  const button = form.querySelector("[type=submit]");
  if (button.disabled) return;
  button.disabled = true;
  try {
    const result = await api("POST", "/api/tasks", {
      ...(specId ? { specId } : {}),
      title: form.elements.title.value,
      brief: form.elements.brief.value,
    });
    closeDialog("task-dialog");
    form.reset();
    ui.selectedTask = result.id;
    await refreshState();
    if (!specId) showView("tasks");
    toast(result.message);
  } catch (error) {
    toast(error.message, "error");
  } finally { button.disabled = false; }
});

$("#note-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const form = event.currentTarget;
  try {
    const result = await api("POST", "/api/memory", {
      note: form.elements.note.value,
    });
    closeDialog("note-dialog");
    form.reset();
    await refreshState();
    showView("memory");
    toast(result.message);
  } catch (error) {
    toast(error.message, "error");
  }
});

$("#action-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const form = event.currentTarget;
  try {
    const result = await api(
      "POST",
      `/api/tasks/${form.dataset.taskId}/actions`,
      {
        action: form.dataset.action,
        session_id: form.elements.session_id.value,
        note: form.dataset.action === "claim" ? null : form.elements.note.value,
        expected_revision:
          form.dataset.action === "handoff"
            ? form.dataset.expectedRevision
            : null,
      },
    );
    closeDialog("action-dialog");
    await refreshState();
    toast(result.message);
  } catch (error) {
    toast(error.message, "error");
  }
});

document.querySelectorAll("dialog").forEach((dialog) =>
  dialog.addEventListener("click", (event) => {
    if (event.target === dialog) dialog.close();
  }),
);

function promptMultiLinePaste(id, text, lineCount) {
  const dialog = $("#paste-dialog");
  if (!dialog) {
    sendStreamInput(id, text);
    return;
  }
  $("#paste-lines-count").textContent = `${lineCount} lines`;
  const preview = text.split(/\r\n|\r|\n/).slice(0, 6).join("\n");
  $("#paste-preview-text").textContent =
    preview + (lineCount > 6 ? `\n… (+${lineCount - 6} more lines)` : "");
  dialog.dataset.sessionId = id;
  ui.pendingPasteText = text;
  showDialog("paste-dialog");
}

$("#paste-confirm-button")?.addEventListener("click", () => {
  const dialog = $("#paste-dialog");
  const id = dialog.dataset.sessionId;
  const text = ui.pendingPasteText;
  ui.pendingPasteText = null;
  closeDialog("paste-dialog");
  if (id && text) {
    if (!sendStreamInput(id, text)) {
      const terminal = ui.terminals.get(id);
      if (terminal) {
        terminal.inputBuffer += text;
        terminal.sendingInput = false;
      }
    }
  }
});

/** The three adjustable panels. Sizes live in this browser only. */
function initResizers() {
  const app = $("#app");
  makeResizer({
    handle: $("#resize-sidebar"),
    target: app,
    name: "--sidebar-w",
    key: "verb.layout.sidebar",
    min: 200,
    max: 400,
    initial: () => $(".sidebar").getBoundingClientRect().width,
    axis: "x",
  });
  const grid = $("#terminal-grid");
  const termHandle = document.createElement("span");
  termHandle.className = "resize-handle term-resize";
  termHandle.setAttribute("aria-label", "Resize the terminals");
  grid.append(termHandle);
  makeResizer({
    handle: termHandle,
    target: grid,
    name: "--term-h",
    key: "verb.layout.terminal",
    min: 220,
    max: 1400,
    initial: () => grid.querySelector(".terminal-tile:not([hidden]) .terminal-mount")?.getBoundingClientRect().height || 480,
    axis: "y",
  });
}

/** An agent that has drawn nothing says so, instead of an empty black tile (Claude on Node 1 hung
 * before its first frame and looked like a broken Verb). */
function showSilent(terminal, text) {
  const note = terminal.tile.querySelector(".terminal-silent");
  if (!note) return;
  note.textContent = text;
  note.hidden = false;
}
/** Whether the screen shows any character. Output alone is not enough: an empty screen still
 * arrives as escape sequences (a reset, a cursor move). */
function hasVisibleText(terminal) {
  const buffer = terminal.term.buffer.active;
  for (let y = 0; y < buffer.length; y += 1) {
    if (buffer.getLine(y)?.translateToString(true).trim()) return true;
  }
  return false;
}
function markDrawn(terminal) {
  if (terminal.drew) return;
  // Checked once xterm has applied the write.
  requestAnimationFrame(() => {
    if (terminal.drew || !hasVisibleText(terminal)) return;
    terminal.drew = true;
    clearTimeout(terminal.silentTimer);
    const note = terminal.tile.querySelector(".terminal-silent");
    if (note) note.hidden = true;
  });
}
function watchSilence(terminal) {
  if (!terminal.agent || terminal.agent === "shell") return;
  terminal.silentTimer = setTimeout(() => {
    if (terminal.ended || terminal.closed || hasVisibleText(terminal)) return;
    showSilent(
      terminal,
      `${agentDisplayName(terminal.agent)} has not drawn anything for 15 seconds. It may still be starting, be waiting on the network, or be stuck. End it if it stays like this.`,
    );
  }, 15000);
}
/** Which agents this execution environment can start. Unknown stays unavailable. */
let agentCapabilityRequest;
async function loadInstalledAgents() {
  if (agentCapabilityRequest) return agentCapabilityRequest;
  agentCapabilityRequest = (async () => {
    try {
      ui.agentCapabilities = await readWithBusyRetry(api, "/api/agents");
      ui.installed = ui.agentCapabilities.installed;
    } catch {
      ui.installed = {};
      ui.agentCapabilities = { unavailable: true };
    }
    updateLaunchForm();
  })();
  try {
    await agentCapabilityRequest;
  } finally {
    agentCapabilityRequest = null;
  }
}

/** Driven with an access token (an agent, a script): say so on every page, with its scope and expiry. */
async function showAccess() {
  try {
    const access = await api("GET", "/api/whoami");
    ui.accessScope = access.scope;
    shell?.renderNeeds();
    if (ui.view === "settings" && !$("#settings-phone").hidden) renderPhoneSettings();
    benchTerminals?.render();
    $("#open-project-switch").hidden = access.scope !== "owner";
    const banner = $("#access-banner");
    banner.hidden = !access.token;
    if (!access.token) return;
    const until = new Date(access.expires * 1000).toLocaleString([], { dateStyle: "medium", timeStyle: "short" });
    banner.innerHTML = `${icon("info", { size: 14 })}<span>Signed in with the access token <b>${escapeHtml(access.token)}</b> · ${escapeHtml(access.scope)} scope · until ${escapeHtml(until)}. Actions are recorded under its name.</span>`;
  } catch {
    /* older server: no banner */
  }
}

async function bootstrap() {
  const workspaceError = await refreshWorkspace();
  if (!ui.state) {
    if (!token && workspaceError?.status === 403 && location.protocol === "http:") {
      $("#app").innerHTML =
        '<div class="locked"><div class="brand-mark">v<span>.</span></div><h1>Open Verb from its local URL</h1><p>Run <code>verb web</code> in your project and open the URL printed in that terminal. It contains this browser session’s local access key.</p></div>';
    } else {
      $("#app").innerHTML =
        '<div class="locked"><div class="brand-mark">v<span>.</span></div><h1>Could not open Verb</h1><p></p><button class="primary-button" type="button">Reload Verb</button></div>';
      $("#app .locked p").textContent = workspaceError?.message || "Reload the page to try again.";
      $("#app .locked button").addEventListener("click", () => location.reload());
    }
    return;
  }
  initResizers();
  // Sessions still running in this Verb come back with the page: after a reload the spec said
  // "1 running" while its work area said nothing was open (found in a user test).
  for (const session of (ui.state.sessions ?? [])
    .filter((s) => s.hostedHere && s.state === "live")
    .slice(0, 6)) {
    addTerminal(session);
  }
  // Talks live on the server for as long as Verb runs; they come back with the page too.
  api("GET", "/api/talk")
    .then(({ talks }) => talks.forEach((t) => mountTalk(t.id)))
    .catch(() => {});
  refreshState(true);
  connectTerminalStream();
  // Late-bound cross-links between views (Ask Verb opens specs and files).
  const extras = {};
  host = initHost({ api, escapeHtml, $ });
  let projectFilesRequest;
  function readProjectFiles() {
    projectFilesRequest ??= readWithBusyRetry(api, "/api/files")
      .finally(() => { projectFilesRequest = null; });
    return projectFilesRequest;
  }
  project = initProject({ api, readProjectFiles, toast, showDialog, closeDialog, showView, escapeHtml, $, openPreview: path => shell.openPreview(path), onBriefChange: () => shell?.render() });
  initProjectSwitcher({ $, showDialog, closeDialog });
  workbench = initWorkbench({
    refreshState,
    extras,
    project,
    api,
    toast,
    showDialog,
    closeDialog,
    showView,
    escapeHtml,
    $,
    ui,
    fitTerminals,
    launchQuickTerminal,
    mountLaunchedSession,
    mountTalk,
    selectTerminal,
    theme,
  });
  studio = initStudio({ $, api, readProjectFiles, ui, workbench, showView, selectTerminal, escapeHtml, toast, agentDisplayName, agentMark });
  connections = initConnections({ $, api, escapeHtml, useInStudio: result => studio.acceptExternalResult(result) });
  shell = initBenchShell({ $, api, ui, workbench, studio, briefExists: project.briefExists, showView, selectTerminal, openSession: id => activateSession(id, { resume: false }), fitTerminals, escapeHtml, agentDisplayName, openFile: project.openFile, refreshFiles: project.refreshFiles, openTask, loadInstalledAgents, showDialog, closeDialog, mountLaunchedSession, refreshState, refreshChecks, toast, openLaunch: id => {
    benchTerminals?.openLaunch(id);
    updateLaunchForm();
    loadInstalledAgents();
  } });
  benchTerminals = initBenchTerminals({ $, api, ui, workbench, showDialog, closeDialog, refreshState, toast,
    toggleSplit() { ui.splitTerminals = !ui.splitTerminals; updateTerminalFocus(); },
  });
  $("#bench-terminal-menu").addEventListener("toggle", event => {
    if (event.target.open) { renderTerminalTabs(); benchTerminals?.render(); }
  });
  project.refreshHub();
  extras.openBenchSpec = shell.focusSpec;
  extras.linkableWorkItems = shell.linkableWorkItems;
  extras.selectedWorkItem = shell.selectedWorkItem;
  extras.refreshBenchWork = shell.refresh;
  extras.openBenchLaunch = id => { benchTerminals.openLaunch(id); updateLaunchForm(); };
  extras.shellCommands = () => [
    ...connections.commands(),
    ...Object.entries({ Studio: "studio", Board: "bench-board", "Project files": "project", Review: "overview", Host: "host",
      Connections: "connections", Tasks: "tasks", "Phone pairing": "phone", "Session history": "session-history" }).map(([label, view]) => ({
        title: `Open ${label}`, run: () => showView(view),
      })),
    { title: "Open Context", run: () => showView("context") },
    { title: "Open Settings", run: () => showView("settings") },
    { title: "Open Sessions", run: () => showView("sessions") },
    { title: "Refresh project", run: () => $("#refresh-button").click() },
    { title: "Open Observer", run: () => $("#observer-pill").click() },
    { title: "Ask Verb", run: () => $('[data-action="ask"]').click() },
    { title: "Open plain terminal", run: launchQuickTerminal },
    ...(ui.terminals.size > 1 ? [{ title: "Toggle terminal split", run: () => {
      ui.splitTerminals = !ui.splitTerminals;
      updateTerminalFocus();
    } }] : []),
    ...(!$("#top-commit").hidden ? [{ title: "Review commit", run: () => $("#top-commit").click() }] : []),
  ];
  extras.onWorkbenchChange = () => {
    studio.render();
    shell.render();
  };
  extras.onStudioSpecCreated = problem => studio.onSpecCreated(problem);
  extras.ask = initAsk({ api, toast, showDialog, showView, escapeHtml, $, extras });
  extras.selectSpec = workbench.selectSpec;
  extras.openFile = project.openFile;
  extras.workbench = workbench;
  extras.observer = initObserver({ api, toast, escapeHtml, $, ui, showView, selectTerminal, extras });
  // These controls are in the page before their handlers exist; they stay disabled until now, so an
  // early click reads as "not yet" instead of silently doing nothing.
  document.querySelectorAll("[data-ready-gate]").forEach((el) => (el.disabled = false));
  showAccess();
  loadInstalledAgents();
  const url = new URL(location.href);
  const requestedView = url.searchParams.get("view");
  const settingsSections = ["host", "connections", "tasks", "phone", "session-history"];
  const requestedSection = url.searchParams.get("section");
  const linkedView = requestedView === "settings" && settingsSections.includes(requestedSection)
    ? requestedSection : requestedView;
  if (requestedView === "memory" || settingsSections.includes(requestedView)) {
    url.searchParams.set("view", requestedView === "memory" ? "context" : "settings");
    if (requestedView !== "memory") url.searchParams.set("section", requestedView);
    history.replaceState(null, "", url.pathname + url.search);
  }
  const views = ["sessions", "bench-board", "needs-you", "context", "memory", "settings",
    "specs", "project", "overview", "host", "connections", "tasks", "phone", "session-history"];
  showView(views.includes(linkedView) ? linkedView : ui.view);
  setInterval(refreshWorkspace, 4000);
  setInterval(() => refreshState(true), 30000);
  setInterval(updatePhoneStatus, 1000);
  setInterval(refreshPhoneStatus, 2500);
}

bootstrap();
