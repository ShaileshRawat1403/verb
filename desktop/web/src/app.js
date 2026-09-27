import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import "./style.css";
import {
  agentMark,
  checksHtml,
  escapeHtml,
  sessionAction,
  stateName,
  taskName,
} from "./view.js";

const $ = (selector) => document.querySelector(selector);
const token =
  location.hash.slice(1) || sessionStorage.getItem("verb-web-token");
if (location.hash && token) {
  sessionStorage.setItem("verb-web-token", token);
  history.replaceState(null, "", location.pathname);
}

const ui = {
  state: null,
  view: "overview",
  selectedTask: null,
  terminals: new Map(),
  refreshing: null,
  toastTimer: null,
};

function toast(message, tone = "info") {
  const element = $("#toast");
  element.textContent = message;
  element.dataset.tone = tone;
  element.classList.add("visible");
  clearTimeout(ui.toastTimer);
  ui.toastTimer = setTimeout(() => element.classList.remove("visible"), 4600);
}

async function api(method, path, body) {
  const response = await fetch(path, {
    method,
    cache: "no-store",
    headers: {
      "X-Verb-Token": token || "",
      ...(body === undefined ? {} : { "Content-Type": "application/json" }),
    },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
  const value = await response.json();
  if (!response.ok)
    throw new Error(value.error || `Request failed (${response.status})`);
  return value;
}

function showView(view) {
  ui.view = view;
  document
    .querySelectorAll(".view")
    .forEach((element) =>
      element.classList.toggle("active", element.id === `view-${view}`),
    );
  document
    .querySelectorAll(".nav-item")
    .forEach((element) =>
      element.classList.toggle("active", element.dataset.view === view),
    );
  $("#breadcrumb-view").textContent =
    view.charAt(0).toUpperCase() + view.slice(1);
  if (view === "sessions") {
    requestAnimationFrame(() => {
      syncHostedTerminals();
      ui.terminals.forEach(({ fit, mount }) => {
        if (mount.clientWidth > 0 && mount.clientHeight > 0) fit.fit();
      });
    });
  }
}

function sessionItem(session, compact = false) {
  const note = session.isolated ? "Isolated checkout" : "Main checkout";
  return `<button class="session-item ${compact ? "compact" : ""}" type="button" data-session-id="${escapeHtml(session.id)}" aria-label="${escapeHtml(`${session.agent} ${session.id.slice(0, 6)} · ${sessionAction(session)}`)}">
    <span class="session-agent-icon">${agentMark(session.agent)}</span>
    <span class="session-copy"><strong>${escapeHtml(session.agent)} <small>${escapeHtml(session.id.slice(0, 6))}</small></strong><span>${escapeHtml(note)} · ${escapeHtml(stateName(session.state))}</span></span>
    ${session.attention ? `<span class="attention-badge">${session.attention}</span>` : ""}
    ${compact ? "" : `<span class="session-next">${escapeHtml(sessionAction(session))}</span>`}
    <span class="row-arrow">↗</span>
  </button>`;
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
    ? `${session.agent} ${id.slice(0, 6)}`
    : `Session ${id.slice(0, 6)}`;
}

function emptyState(icon, title, body, action) {
  return `<div class="list-empty"><span>${icon}</span><strong>${title}</strong><p>${body}</p>${action || ""}</div>`;
}

function render() {
  const state = ui.state;
  if (!state) return;
  const { project, sessions, tasks, memory } = state;
  const attention = tasks.filter(
    (task) => task.needsHelp || task.status === "needs review",
  ).length;
  $("#sidebar-project").textContent = project.name || "Project";
  $("#sidebar-branch").textContent = project.branch || "No branch";
  $("#breadcrumb-project").textContent = project.name || "Project";
  $("#top-branch").textContent = project.branch || "No branch";
  $("#top-changes").textContent = `${project.changedFiles} changed`;
  $("#nav-session-count").textContent = sessions.length;
  $("#nav-task-count").textContent = tasks.filter(
    (task) => task.status !== "done",
  ).length;
  $("#stat-sessions").textContent = sessions.length;
  $("#stat-tasks").textContent = tasks.filter(
    (task) => task.status !== "done",
  ).length;
  $("#stat-attention").textContent = attention;
  $("#session-count").textContent = sessions.length;
  $("#task-count").textContent = tasks.length;

  $("#overview-sessions").innerHTML = sessions.length
    ? sessions
        .slice(0, 3)
        .map((session) => sessionItem(session, true))
        .join("")
    : emptyState(
        "▤",
        "No sessions yet",
        "Start a CLI agent to connect it to this project.",
        '<button class="text-button" type="button" data-action="new-session">Start an agent →</button>',
      );
  $("#overview-tasks").innerHTML = tasks.length
    ? tasks
        .filter((task) => task.status !== "done")
        .slice(0, 3)
        .map((task) => taskItem(task, true))
        .join("") ||
      emptyState(
        "✓",
        "All caught up",
        "Completed tasks remain in the task history.",
      )
    : emptyState(
        "☷",
        "Nothing to track yet",
        "Give an agent a task that can survive a handoff.",
        '<button class="text-button" type="button" data-action="new-task">Create a task →</button>',
      );
  $("#session-list").innerHTML = sessions.length
    ? sessions.map((session) => sessionItem(session)).join("")
    : emptyState(
        "▤",
        "No sessions yet",
        "Start a CLI agent. It will appear here with its own durable record.",
      );
  $("#task-list").innerHTML = tasks.length
    ? tasks.map((task) => taskItem(task)).join("")
    : emptyState(
        "☷",
        "No tasks yet",
        "Create the first piece of shared work.",
      );
  $("#memory-content").textContent =
    memory ||
    "No shared notes yet. Add a decision, constraint, or handoff fact.";
  $("#memory-content").classList.toggle("empty", !memory);
  if (!tasks.some((task) => task.id === ui.selectedTask))
    ui.selectedTask = tasks[0]?.id || null;
  renderTaskDetail();
  syncHostedTerminals();
}

function renderTaskDetail() {
  const task = ui.state?.tasks.find((item) => item.id === ui.selectedTask);
  const panel = $("#task-detail");
  if (!task) {
    panel.innerHTML =
      '<div class="empty-detail"><span class="empty-icon">☷</span><h3>Select a task</h3><p>See its brief, owner, and recorded handoffs here.</p></div>';
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
  panel.innerHTML = `<div class="detail-top"><div class="section-kicker">TASK DETAIL</div><h2>${escapeHtml(task.title)}</h2><div class="detail-meta"><span class="status-pill ${escapeHtml(task.status.replaceAll(" ", "-"))}">${escapeHtml(task.needsHelp ? "Help requested" : taskName(task.status))}</span><span>${task.owner ? `Owned by ${escapeHtml(sessionLabel(task.owner))}` : "Unassigned"}</span></div></div><div class="detail-section"><span class="section-kicker">BRIEF</span><p class="task-brief">${escapeHtml(task.brief || "No brief recorded.")}</p></div><div class="detail-section"><span class="section-kicker">RECORDED HISTORY</span><div class="history-list">${history}</div></div>${buttons ? `<div class="detail-actions">${buttons}</div>` : ""}`;
}

function syncHostedTerminals() {
  if (ui.view !== "sessions") return;
  for (const session of ui.state.sessions) {
    if (session.hasTerminal && !ui.terminals.has(session.id))
      addTerminal(session);
  }
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
  const request = api("GET", "/api/state");
  ui.refreshing = request;
  try {
    ui.state = await request;
    render();
  } catch (error) {
    if (!silent) toast(error.message, "error");
  } finally {
    if (ui.refreshing === request) ui.refreshing = null;
  }
}

function renderChecks(report) {
  $("#checks-list").innerHTML = checksHtml(report);
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

async function activateSession(id) {
  const session = ui.state?.sessions.find((item) => item.id === id);
  if (!session) return;
  if (session.hasTerminal) {
    showView("sessions");
    document
      .getElementById(`terminal-${id}`)
      ?.scrollIntoView({ behavior: "smooth", block: "nearest" });
    ui.terminals.get(id)?.term.focus();
    return;
  }
  if (session.canResume) {
    try {
      const result = await api("POST", "/api/terminals", {
        agent: "resume",
        resume_id: id,
      });
      await refreshState();
      showView("sessions");
      ui.terminals.get(result.sessionId)?.term.focus();
    } catch (error) {
      toast(error.message, "error");
    }
    return;
  }
  try {
    const inbox = await api("GET", `/api/inbox/${encodeURIComponent(id)}`);
    $("#inbox-title").textContent = `${session.agent} · ${id.slice(0, 8)}`;
    $("#inbox-subtitle").textContent =
      `${stateName(session.state)} · ${inbox.contextState.replaceAll("_", " ")}`;
    $("#inbox-content").innerHTML = inbox.items.length
      ? inbox.items
          .map(
            (item) =>
              `<div class="inbox-item"><span class="inbox-kind">${escapeHtml(item.kind.replaceAll("_", " "))}</span><strong>${escapeHtml(item.title)}</strong><small>${item.newSinceFetch ? "New since last context fetch" : "Recorded attention"}</small></div>`,
          )
          .join("")
      : '<div class="list-empty"><span>✓</span><strong>Nothing needs attention</strong><p>The session’s recorded work is still in the project task history.</p></div>';
    showDialog("inbox-dialog");
  } catch (error) {
    toast(error.message, "error");
  }
}

function addTerminal(session) {
  const grid = $("#terminal-grid");
  grid.querySelector(".terminal-empty")?.remove();
  const tile = document.createElement("div");
  tile.className = "terminal-tile";
  tile.id = `terminal-${session.id}`;
  tile.innerHTML = `<div class="terminal-titlebar"><div class="window-dots"><i></i><i></i><i></i></div><span class="terminal-title">${escapeHtml(session.agent)} <small>${escapeHtml(session.id.slice(0, 6))}</small></span><span class="terminal-checkout">${session.isolated ? "ISOLATED" : "MAIN"}</span><button class="terminal-close" type="button" data-close-terminal="${escapeHtml(session.id)}" aria-label="Close terminal tile">×</button></div><div class="terminal-mount"></div><div class="terminal-bottom"><span class="terminal-state">● Connected to Verb</span><span>PTY · ${escapeHtml(session.isolated ? "isolated checkout" : "project checkout")}</span></div>`;
  grid.append(tile);
  const mount = tile.querySelector(".terminal-mount");
  const term = new Terminal({
    cursorBlink: true,
    cursorStyle: "bar",
    fontFamily: "SFMono-Regular, Menlo, Consolas, monospace",
    fontSize: 13,
    lineHeight: 1.22,
    letterSpacing: 0.2,
    scrollback: 4000,
    theme: {
      background: "#101521",
      foreground: "#e5e7f0",
      cursor: "#aa9dff",
      selectionBackground: "#665ba677",
      black: "#121623",
      red: "#ff7e91",
      green: "#9cdbba",
      yellow: "#f5cc86",
      blue: "#9cabff",
      magenta: "#c7a6ff",
      cyan: "#80d7e1",
      white: "#e7e8f0",
      brightBlack: "#687184",
    },
  });
  const fit = new FitAddon();
  term.loadAddon(fit);
  term.open(mount);
  const terminal = {
    term,
    fit,
    mount,
    tile,
    cursor: 0,
    inputQueue: Promise.resolve(),
    closed: false,
  };
  ui.terminals.set(session.id, terminal);
  term.onData((data) => {
    if (terminal.closed || terminal.ended) return;
    terminal.inputQueue = terminal.inputQueue
      .then(() => api("POST", `/api/terminals/${session.id}/input`, { data }))
      .catch(async (error) => {
        // A paired phone holds input. Taking it back is the desktop user's explicit choice.
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
      });
  });
  let resizeTimer;
  const observer = new ResizeObserver(() => {
    clearTimeout(resizeTimer);
    resizeTimer = setTimeout(() => {
      if (mount.clientWidth < 30 || mount.clientHeight < 30 || terminal.closed)
        return;
      fit.fit();
      api("POST", `/api/terminals/${session.id}/resize`, {
        rows: term.rows,
        cols: term.cols,
      }).catch(() => {});
    }, 80);
  });
  observer.observe(mount);
  terminal.observer = observer;
  pollTerminal(session.id);
}

async function pollTerminal(id) {
  const terminal = ui.terminals.get(id);
  if (!terminal || terminal.closed) return;
  try {
    const output = await api(
      "GET",
      `/api/terminals/${id}/output?after=${terminal.cursor}`,
    );
    const bytes = Uint8Array.from(atob(output.data), (character) =>
      character.charCodeAt(0),
    );
    if (output.reset) terminal.term.reset();
    if (bytes.length)
      await new Promise((resolve) => terminal.term.write(bytes, resolve));
    terminal.cursor = output.cursor;
    if (!output.running) {
      terminal.tile.querySelector(".terminal-state").textContent = output.error
        ? `○ Session stopped: ${output.error}`
        : "○ Session ended";
      terminal.tile.classList.add("ended");
      terminal.ended = true;
    }
  } catch (error) {
    terminal.tile.querySelector(".terminal-state").textContent =
      "○ Connection interrupted";
    if (!terminal.reportedError) toast(error.message, "error");
    terminal.reportedError = true;
  }
  if (!terminal.closed && !terminal.ended)
    setTimeout(() => pollTerminal(id), 150);
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
        `<option value="${escapeHtml(session.id)}">${escapeHtml(session.agent)} · ${escapeHtml(session.id.slice(0, 8))} · ${escapeHtml(stateName(session.state))}${session.isolated ? " · isolated" : ""}</option>`,
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

document.addEventListener("click", async (event) => {
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
  const terminalClose = event.target.closest("[data-close-terminal]");
  if (terminalClose) {
    const id = terminalClose.dataset.closeTerminal;
    const tile = ui.terminals.get(id);
    if (!tile) return;
    if (
      !tile.tile.classList.contains("ended") &&
      !confirm("Stop this agent process and close its terminal?")
    )
      return;
    try {
      if (!tile.tile.classList.contains("ended"))
        await api("DELETE", `/api/terminals/${id}`);
      tile.closed = true;
      tile.observer.disconnect();
      tile.term.dispose();
      tile.tile.remove();
      ui.terminals.delete(id);
      await refreshState();
    } catch (error) {
      toast(error.message, "error");
    }
    return;
  }
  const actionButton = event.target.closest("[data-action]");
  if (actionButton) {
    if (actionButton.dataset.action === "new-session")
      return showDialog("session-dialog");
    if (actionButton.dataset.action === "new-task")
      return showDialog("task-dialog");
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

$("#refresh-button").addEventListener("click", () => refreshState());
document
  .querySelectorAll('#session-dialog input[name="agent"]')
  .forEach((input) =>
    input.addEventListener("change", () => {
      $("#custom-command-fields").hidden =
        $('#session-form input[name="agent"]:checked').value !== "custom";
    }),
  );

$("#session-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const form = event.currentTarget;
  const agent = form.elements.agent.value;
  const command = form.elements.command.value.trim();
  if (agent === "custom" && !command)
    return toast("Enter a CLI executable.", "error");
  const button = form.querySelector('[type="submit"]');
  button.disabled = true;
  try {
    const result = await api("POST", "/api/terminals", {
      agent,
      isolated: form.elements.isolated.checked,
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
    await refreshState();
    showView("sessions");
    ui.terminals.get(result.sessionId)?.term.focus();
  } catch (error) {
    toast(error.message, "error");
  } finally {
    button.disabled = false;
  }
});

$("#task-form").addEventListener("submit", async (event) => {
  event.preventDefault();
  const form = event.currentTarget;
  try {
    const result = await api("POST", "/api/tasks", {
      title: form.elements.title.value,
      brief: form.elements.brief.value,
    });
    closeDialog("task-dialog");
    form.reset();
    ui.selectedTask = result.id;
    await refreshState();
    showView("tasks");
    toast(result.message);
  } catch (error) {
    toast(error.message, "error");
  }
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

if (!token || !/^[0-9a-f]{64}$/.test(token)) {
  $("#app").innerHTML =
    '<div class="locked"><div class="brand-mark">v<span>.</span></div><h1>Open Verb from its local URL</h1><p>Run <code>verb web</code> in your project and open the URL printed in that terminal. It contains this browser session’s local access key.</p></div>';
} else {
  refreshState();
  refreshChecks();
  setInterval(() => refreshState(true), 4000);
  setInterval(() => refreshChecks(), 60000);
}
