// Command blocks: each command in a shell terminal becomes a block with a status edge, its exit code
// and duration, and actions when it fails. Built only from the shell-integration markers Verb's own
// integration emits (OSC 633: A prompt, B input, C running, D;exit finished, E;command line), so a
// block is never inferred from screen text. Shells without integration simply show no blocks.

/** Parses one OSC 633 payload (everything after `633;`). Unknown payloads return null. */
export function parseShellMark(data) {
  const kind = data.charAt(0);
  if (data.length > 1 && data.charAt(1) !== ";") return null;
  const rest = data.length > 2 ? data.slice(2) : "";
  if (kind === "A" || kind === "B" || kind === "C") return { kind };
  if (kind === "D") {
    const exit = rest === "" ? null : Number.parseInt(rest, 10);
    return { kind, exit: Number.isFinite(exit) ? exit : null };
  }
  if (kind === "E") return { kind, command: unescapeCommand(rest.split(";")[0]) };
  return null;
}

/** Undoes the integration scripts' escaping of `\`, `;` and newlines. */
export function unescapeCommand(text) {
  return text.replace(/\\x([0-9a-fA-F]{2})|\\\\/g, (match, hex) =>
    hex ? String.fromCharCode(Number.parseInt(hex, 16)) : "\\",
  );
}

/** "0.4 s", "12 s", "2 min 5 s". */
export function formatDuration(ms) {
  if (!Number.isFinite(ms) || ms < 0) return "";
  if (ms < 10_000) return `${(ms / 1000).toFixed(1)} s`;
  const seconds = Math.round(ms / 1000);
  if (seconds < 60) return `${seconds} s`;
  const minutes = Math.floor(seconds / 60);
  return `${minutes} min${seconds % 60 ? ` ${seconds % 60} s` : ""}`;
}

/** The words shown on a finished block. */
export function blockMeta(exit, ms) {
  const time = ms >= 100 ? formatDuration(ms) : "";
  if (exit === null || exit === undefined) return time;
  if (exit === 0) return time;
  return time ? `exit ${exit} · ${time}` : `exit ${exit}`;
}

/** The note handed to an agent about a failed block: facts only, never pressed Enter on. */
export function failureNote(command, exit, output) {
  const trimmed = output.trim().split("\n").slice(-30).join("\n");
  return `\`${command}\` failed with exit ${exit}. Its last output:\n\n${trimmed}\n\nPlease look into it.`;
}

/**
 * Attaches blocks to an xterm.js terminal. `actions` is `{ copy(text), handOff(note) | null,
 * canHandOff() }`. Returns `{ previous(), next(), dispose() }` for jumping between commands.
 */
export function attachCommandBlocks(term, actions) {
  const blocks = [];
  let prompt = null; // marker at the start of the current prompt
  let command = "";
  let startedAt = 0;
  let running = false;
  const disposables = [];

  const lineText = (y) => term.buffer.active.getLine(y)?.translateToString(true) ?? "";

  const finish = (exit) => {
    if (!running || !prompt || prompt.isDisposed) {
      running = false;
      return;
    }
    running = false;
    const buffer = term.buffer.active;
    const cursorLine = buffer.baseY + buffer.cursorY;
    const end = buffer.cursorX === 0 ? cursorLine - 1 : cursorLine;
    const height = Math.max(1, end - prompt.line + 1);
    const ms = Date.now() - startedAt;
    const status = exit === null ? "unknown" : exit === 0 ? "passed" : "failed";
    const block = { marker: prompt, command, exit, ms, status, height };
    blocks.push(block);

    const edge = term.registerDecoration({ marker: prompt, x: 0, width: 1, height, layer: "bottom" });
    edge?.onRender((el) => {
      // Added to, never replacing, xterm's own class: that one positions the element on its rows.
      el.classList.add("cmd-edge", status);
    });
    const metaWidth = Math.min(term.cols, status === "failed" && command ? 34 : 14);
    const meta = term.registerDecoration({
      marker: prompt,
      anchor: "right",
      x: 0,
      width: metaWidth,
      height: 1,
      layer: "top",
    });
    meta?.onRender((el) => {
      // Re-checked on every render (resizes re-render): when the command line leaves too little
      // room, the label shrinks to its words and shows its buttons only on hover.
      const free = term.cols - lineText(prompt.line).trimEnd().length;
      el.classList.toggle("compact", free < metaWidth + 2);
      if (el.dataset.ready) return;
      el.dataset.ready = "1";
      el.classList.add("cmd-meta", status);
      const words = document.createElement("span");
      words.textContent = blockMeta(exit, ms);
      el.append(words);
      if (status !== "failed") return;
      const output = () => {
        const lines = [];
        for (let y = prompt.line + 1; y <= prompt.line + height - 1; y += 1) lines.push(lineText(y));
        return lines.join("\n");
      };
      const button = (label, title, onClick) => {
        const b = document.createElement("button");
        b.type = "button";
        b.textContent = label;
        b.title = title;
        b.addEventListener("mousedown", (event) => event.preventDefault());
        b.addEventListener("click", (event) => {
          event.stopPropagation();
          onClick();
        });
        el.append(b);
      };
      button("Copy", "Copy the command and its output", () =>
        actions.copy(`$ ${command}\n${output()}`),
      );
      if (command && actions.canHandOff()) {
        button("Ask agent", "Paste a note about this failure into the agent's prompt, without sending it", () =>
          actions.handOff(failureNote(command, exit, output())),
        );
      }
    });
    disposables.push(edge, meta);
  };

  const handler = term.parser.registerOscHandler(633, (data) => {
    const mark = parseShellMark(data);
    if (!mark) return false;
    if (mark.kind === "A") {
      if (running) finish(null);
      prompt = term.registerMarker(0);
    } else if (mark.kind === "E") {
      command = mark.command;
    } else if (mark.kind === "C") {
      running = true;
      startedAt = Date.now();
    } else if (mark.kind === "D") {
      finish(mark.exit);
      command = "";
    }
    // Returning false lets any other handler see the sequence too; xterm draws nothing for it.
    return false;
  });

  const jump = (direction) => {
    const buffer = term.buffer.active;
    const top = buffer.viewportY;
    const lines = blocks.filter((b) => !b.marker.isDisposed).map((b) => b.marker.line);
    const target =
      direction < 0 ? [...lines].reverse().find((line) => line < top) : lines.find((line) => line > top);
    if (target !== undefined) term.scrollToLine(target);
    else if (direction > 0) term.scrollToBottom();
  };

  return {
    blocks,
    previous: () => jump(-1),
    next: () => jump(1),
    dispose() {
      handler.dispose();
      for (const d of disposables) d?.dispose();
    },
  };
}
