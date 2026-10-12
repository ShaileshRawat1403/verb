// Maps stream events to xterm views. Acknowledgement follows rendering, never receipt.
export function createTerminalStreamView({
  getUi,
  pollTerminal,
  markDrawn,
  hasVisibleText,
  showSilent,
  refreshState,
  agentDisplayName,
  escapeHtml,
  sendStreamAttach,
  sendStreamAck,
}) {
  const textDecoder = new TextDecoder();
  function connected() {
    getUi().terminals.forEach((terminal, id) => {
      terminal.streaming = true;
      sendStreamAttach(id, terminal.term.rows, terminal.term.cols);
      const state = terminal.tile.querySelector(".terminal-state");
      if (state && !terminal.ended) {
        state.textContent = "Connected to Verb (streaming)";
        state.dataset.tone = "";
      }
    });
  }
  function disconnected() {
    getUi().terminals.forEach((terminal, id) => {
      terminal.streaming = false;
      const state = terminal.tile.querySelector(".terminal-state");
      if (state && !terminal.ended) {
        state.textContent = "Reconnecting stream…";
        state.dataset.tone = "warn";
      }
      pollTerminal(id);
    });
  }
  function markTerminalActivity(id) {
    if (
      id === getUi().focusedTerminal &&
      !document.hidden &&
      getUi().view === "sessions"
    )
      return;
    const terminal = getUi().terminals.get(id);
    if (!terminal) return;
    terminal.tile.classList.add("has-activity");
  }

  function handleStreamBinary(buffer) {
    const bytes = new Uint8Array(buffer);
    if (bytes.length < 2) return;
    const idLen = bytes[0];
    if (bytes.length < 1 + idLen) return;
    const id = textDecoder.decode(bytes.subarray(1, 1 + idLen));
    const payload = bytes.subarray(1 + idLen);
    const terminal = getUi().terminals.get(id);
    if (!terminal || terminal.closed || terminal.ended) return;

    terminal.streaming = true;
    if (payload.length) markDrawn(terminal);
    terminal.unackedBytes = (terminal.unackedBytes || 0) + payload.length;
    markTerminalActivity(id);

    terminal.term.write(payload, () => {
      terminal.unackedBytes = Math.max(
        0,
        terminal.unackedBytes - payload.length,
      );
      terminal.pendingAck = (terminal.pendingAck || 0) + payload.length;
      if (terminal.pendingAck >= 16384) {
        clearTimeout(terminal.ackTimer);
        terminal.ackTimer = null;
        sendStreamAck(id, terminal.pendingAck);
        terminal.pendingAck = 0;
      } else if (!terminal.ackTimer) {
        terminal.ackTimer = setTimeout(() => {
          terminal.ackTimer = null;
          if (terminal.pendingAck > 0 && !terminal.closed && !terminal.ended) {
            sendStreamAck(id, terminal.pendingAck);
            terminal.pendingAck = 0;
          }
        }, 120);
      }
    });
  }

  function handleStreamControl(msg) {
    if (msg.v === 1) {
      return;
    }
    if (msg.type === "attached") {
      const { id, screen, running, controller, phoneConnected } = msg;
      const terminal = getUi().terminals.get(id);
      if (!terminal || terminal.closed) return;
      terminal.streaming = true;
      if (screen) {
        const raw = Uint8Array.from(atob(screen), (c) => c.charCodeAt(0));
        terminal.term.reset();
        terminal.term.write(raw);
        if (raw.length) markDrawn(terminal);
      }
      const phoneControls = controller === "phone";
      const hasPhone = phoneControls || Boolean(phoneConnected);
      terminal.tile.querySelector("[data-phone-terminal]").hidden = !hasPhone;
      terminal.tile.querySelector("[data-take-terminal]").hidden =
        !phoneControls;
      terminal.tile.querySelector(".terminal-state").textContent = phoneControls
        ? "Phone controls input"
        : phoneConnected
          ? "Desktop controls input · phone connected"
          : "Connected to Verb (streaming)";
      if (!running) {
        terminal.tile.querySelector(".terminal-state").textContent =
          "Session ended";
        terminal.tile.classList.add("ended");
        terminal.ended = true;
      }
    } else if (msg.type === "exit") {
      const { id, code } = msg;
      const terminal = getUi().terminals.get(id);
      if (!terminal) return;
      terminal.tile.querySelector(".terminal-state").textContent =
        code === 0 ? "Session finished" : `Session exited (${code})`;
      terminal.tile.classList.add("ended");
      terminal.ended = true;
      if (
        !hasVisibleText(terminal) &&
        terminal.agent &&
        terminal.agent !== "shell"
      ) {
        showSilent(
          terminal,
          `${agentDisplayName(terminal.agent)} exited with code ${code} without printing anything. Open a Terminal here and run it by hand to see why.`,
        );
      }
      const titleEl = terminal.tile.querySelector(".terminal-title");
      if (titleEl && !titleEl.querySelector(".terminal-exit-badge")) {
        const badge = document.createElement("span");
        badge.className = `terminal-exit-badge ${code === 0 ? "exit-ok" : "exit-err"}`;
        badge.textContent = `exit ${code}`;
        titleEl.appendChild(badge);
      }
      refreshState(true);
    } else if (msg.type === "title") {
      const { id, title } = msg;
      const terminal = getUi().terminals.get(id);
      if (!terminal || !title) return;
      const titleEl = terminal.tile.querySelector(".terminal-title");
      if (titleEl) {
        titleEl.innerHTML = `${escapeHtml(title)} <small>${escapeHtml(id.slice(0, 6))}</small>`;
      }
    } else if (msg.type === "bell") {
      const { id } = msg;
      const terminal = getUi().terminals.get(id);
      if (!terminal) return;
      terminal.tile.classList.add("terminal-bell");
      setTimeout(() => terminal.tile.classList.remove("terminal-bell"), 500);
    }
  }

  return {
    connected,
    disconnected,
    binary: handleStreamBinary,
    control: handleStreamControl,
  };
}
