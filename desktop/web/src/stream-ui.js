// The Stream view of one agent terminal tile: the conversation panel (conversation-ui.js) over the
// terminal pane. The terminal keeps running underneath; one switch goes back to it.
import { mountConversation } from "./conversation-ui.js";
import { hasStream } from "./conversation-model.js";

export {
  claimedCriterion,
  compactTokens,
  groupStream,
  hasStream,
  renderProse,
  stepWords,
  workSummary,
} from "./conversation-model.js";

/** Whether a session's agent gets the Stream view. Agents Verb can't read keep the terminal only. */
export function hasStreamView(agent) {
  return hasStream(agent);
}

/**
 * Adds the Stream view to one agent terminal tile. `deps`: { api, escapeHtml, icon, toast,
 * agentDisplayName, agentMark, settings: { get(), set(enabled) }, workbench }.
 */
export function attachStream(tile, session, terminal, deps) {
  if (!hasStreamView(session.agent)) return null;
  const { escapeHtml } = deps;
  const mount = tile.querySelector(".terminal-mount");

  const toggle = document.createElement("div");
  toggle.className = "stream-toggle";
  toggle.setAttribute("role", "group");
  toggle.setAttribute("aria-label", "View");
  toggle.innerHTML = `<button type="button" data-view-mode="stream" aria-pressed="false">Chat</button><button type="button" data-view-mode="terminal" aria-pressed="true">Terminal</button>`;
  tile.querySelector(".terminal-checkout")?.after(toggle);

  const view = document.createElement("section");
  view.className = "agent-stream";
  view.hidden = true;
  view.setAttribute("aria-label", `Chat with ${deps.agentDisplayName(session.agent)}`);
  mount.append(view);

  let mode = "terminal";
  const setMode = (next) => {
    mode = next;
    view.hidden = mode !== "stream";
    tile.classList.toggle("stream-mode", mode === "stream");
    for (const b of toggle.querySelectorAll("button")) b.setAttribute("aria-pressed", String(b.dataset.viewMode === mode));
    if (mode === "stream") {
      panel.start();
      panel.focus();
    } else {
      panel.stop();
      terminal.term.focus();
    }
  };

  const panel = mountConversation(view, session.id, {
    api: deps.api,
    escapeHtml,
    icon: deps.icon,
    toast: deps.toast,
    agentName: deps.agentDisplayName(session.agent),
    agentMark: deps.agentMark(session.agent),
    workbench: deps.workbench,
    openTerminal: () => setMode("terminal"),
    onStreamChange: (status) => {
      deps.settings.value = status.enabled;
    },
  });

  toggle.addEventListener("click", (event) => {
    const b = event.target.closest("[data-view-mode]");
    if (b) setMode(b.dataset.viewMode);
  });

  // Agent sessions open as chat when the project has the stream on; otherwise as the terminal.
  deps.settings.get().then((enabled) => {
    if (enabled && !terminal.closed) setMode("stream");
  });

  return {
    dispose: () => panel.dispose(),
    setMode,
  };
}
