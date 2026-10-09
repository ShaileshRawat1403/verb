// The agent stream: a readable view of a Claude Code or Codex session over its terminal pane, built
// from the agent's own log by the server (src/transcript.rs) and shown only when this project has
// turned it on. The terminal keeps running underneath; one switch goes back to it.
//
// Pure helpers are exported for tests; `attachStream` wires one terminal tile.

const STREAM_AGENTS = new Set(["claude", "codex"]);
// Agents that get the Stream view for its composer, though Verb cannot read their conversation.
const COMPOSER_ONLY = new Set(["agy"]);

/** Whether a session's agent has a stream Verb can read. */
export function hasStream(agent) {
  return STREAM_AGENTS.has(agent);
}

/** Whether a session's agent gets the Stream view at all (its conversation, or just the composer). */
export function hasStreamView(agent) {
  return STREAM_AGENTS.has(agent) || COMPOSER_ONLY.has(agent);
}

/**
 * Groups items into blocks: `{kind:"you"|"agent", item}` or `{kind:"work", steps, started, ended}`
 * for each run of consecutive steps.
 */
export function groupStream(items) {
  const blocks = [];
  for (const item of items) {
    if (item.kind === "step") {
      const last = blocks[blocks.length - 1];
      if (last?.kind === "work") {
        last.steps.push(item);
        last.ended = item.at ?? last.ended;
      } else {
        blocks.push({ kind: "work", steps: [item], started: item.at, ended: item.at });
      }
    } else {
      blocks.push({ kind: item.kind, item });
    }
  }
  return blocks;
}

/** "Worked for 3 min · 6 steps", from ISO timestamps when the log has them. */
export function workSummary(block) {
  const n = block.steps.length;
  const steps = `${n} step${n === 1 ? "" : "s"}`;
  const ms = Date.parse(block.ended) - Date.parse(block.started);
  if (!Number.isFinite(ms) || ms < 1000) return `Worked · ${steps}`;
  const minutes = Math.round(ms / 60000);
  const time = minutes >= 1 ? `${minutes} min` : `${Math.round(ms / 1000)} s`;
  return `Worked for ${time} · ${steps}`;
}

/** The words for one step: "Read src/auth/link.ts", "Ran npm test". */
export function stepWords(step) {
  const verbs = { Read: "Read", Write: "Wrote", Edit: "Edited", MultiEdit: "Edited", Bash: "Ran", Grep: "Searched for", Glob: "Listed", WebFetch: "Fetched", WebSearch: "Searched the web for", Task: "Started a subagent:", Agent: "Started a subagent:" };
  const verb = verbs[step.tool] ?? step.tool ?? "Step";
  return { verb, target: step.target ?? "" };
}

/** Escaped text with `code`, ```blocks```, **bold** and paragraphs. Nothing else is interpreted. */
export function renderProse(text, escapeHtml) {
  const parts = String(text).split(/```[^\n]*\n?/);
  return parts
    .map((part, i) => {
      if (i % 2 === 1) return `<pre>${escapeHtml(part.replace(/\n$/, ""))}</pre>`;
      return part
        .split(/\n{2,}/)
        .filter((p) => p.trim())
        .map((p) => {
          let html = escapeHtml(p.trim());
          html = html.replace(/`([^`\n]+)`/g, "<code>$1</code>");
          html = html.replace(/\*\*([^*\n]+)\*\*/g, "<b>$1</b>");
          html = html.replace(/\n/g, "<br>");
          return `<p>${html}</p>`;
        })
        .join("");
    })
    .join("");
}

/**
 * The criterion an agent says it has met, and the sentence that says so: "Criterion 2 is met",
 * "criterion #1 now passes". Null when the text claims nothing.
 */
export function claimedCriterion(text) {
  const re = /criteri(?:on|a)\s*#?\s*(\d+)\b[^.\n]{0,80}?\b(?:is|are|now|has been|was)?\s*(?:met|proven|satisfied|passes|passing|done|complete|fulfilled)\b/i;
  const m = re.exec(text);
  if (!m) return null;
  const start = Math.max(text.lastIndexOf(".", m.index) + 1, text.lastIndexOf("\n", m.index) + 1, 0);
  const endDot = text.indexOf(".", m.index + m[0].length);
  const end = endDot === -1 ? text.length : endDot + 1;
  return { number: Number(m[1]), sentence: text.slice(start, end).trim().slice(0, 280) };
}

/** A compact token count: 4210 → "4.2k". */
export function compactTokens(n) {
  if (n == null) return "";
  return n >= 1000 ? `${(n / 1000).toFixed(n >= 100000 ? 0 : 1)}k` : String(n);
}

const time = (iso) => {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? "" : d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
};

/**
 * Adds the Stream view to one agent terminal tile. `deps`: { api, escapeHtml, icon, toast,
 * agentDisplayName, agentMark, settings: { get(), set(enabled) } }.
 */
export function attachStream(tile, session, terminal, deps) {
  if (!hasStreamView(session.agent)) return null;
  const readable = hasStream(session.agent);
  const { escapeHtml, icon } = deps;
  const name = deps.agentDisplayName(session.agent);
  const mount = tile.querySelector(".terminal-mount");

  const toggle = document.createElement("div");
  toggle.className = "stream-toggle";
  toggle.setAttribute("role", "group");
  toggle.setAttribute("aria-label", "View");
  toggle.innerHTML = `<button type="button" data-view-mode="stream" aria-pressed="false">Stream</button><button type="button" data-view-mode="terminal" aria-pressed="true">Terminal</button>`;
  tile.querySelector(".terminal-checkout")?.after(toggle);

  const view = document.createElement("div");
  view.className = "agent-stream";
  view.hidden = true;
  view.innerHTML = `<div class="stream-feed" aria-live="polite"></div><form class="stream-composer"><textarea rows="1" placeholder="Message ${escapeHtml(name)}" aria-label="Message ${escapeHtml(name)}"></textarea><div class="composer-row"><span class="composer-chip"><span class="stream-avatar" aria-hidden="true">${escapeHtml(deps.agentMark(session.agent))}</span>${escapeHtml(name)}</span><span class="composer-chip composer-spec" hidden></span><span class="composer-hint">Enter sends · Shift+Enter for a new line</span><span class="composer-meter" hidden></span><button type="button" class="text-button stream-off" data-stream-disable title="Stop reading agent logs for this project">Turn off stream</button><button type="submit" class="composer-send" aria-label="Send">${icon("arrow-up", { size: 16 })}</button></div></form>`;
  mount.append(view);
  const feed = view.querySelector(".stream-feed");
  const form = view.querySelector(".stream-composer");
  const input = form.querySelector("textarea");

  let mode = "terminal";
  let timer = null;
  let lastJson = "";
  let pending = null; // text sent but not yet in the log
  const dismissed = new Set(); // "spec:criterion" the person said "Not yet" to

  const setMode = (next) => {
    mode = next;
    view.hidden = mode !== "stream";
    tile.classList.toggle("stream-mode", mode === "stream");
    for (const b of toggle.querySelectorAll("button")) b.setAttribute("aria-pressed", String(b.dataset.viewMode === mode));
    clearTimeout(timer);
    if (mode === "stream") {
      refresh();
      input.focus();
    } else {
      terminal.term.focus();
    }
  };
  toggle.addEventListener("click", (event) => {
    const b = event.target.closest("[data-view-mode]");
    if (b) setMode(b.dataset.viewMode);
  });

  const optIn = () => `<div class="stream-optin">
      <h4>See this session as a conversation</h4>
      <p>Verb can show what you asked ${escapeHtml(name)}, what it answered, and the files and commands it worked on, read from the session log it keeps on this machine.</p>
      <ul>
        <li><b>Off until you turn it on</b>, for this project only.</li>
        <li><b>Read when you look</b>, never stored, and never sent to a model or anywhere else.</li>
        <li><b>Keys and passwords are hidden</b> before anything is shown.</li>
      </ul>
      <button type="button" class="primary-button" data-stream-enable>Turn on for this project</button>
    </div>`;

  const render = (data) => {
    if (!data.enabled) {
      feed.innerHTML = optIn();
      form.hidden = true;
      return;
    }
    form.hidden = false;
    const nearBottom = feed.scrollHeight - feed.scrollTop - feed.clientHeight < 80;
    const items = [...data.items];
    if (pending && !items.some((i) => i.kind === "you" && i.text?.trim() === pending.trim())) {
      items.push({ kind: "you", text: pending, pending: true });
    } else {
      pending = null;
    }
    if (!items.length) {
      feed.innerHTML = `<p class="stream-empty">${escapeHtml(data.note ?? `Nothing yet. Ask ${name} something below, or switch to Terminal.`)}</p>`;
      return;
    }
    const blocks = groupStream(items);
    const lastWork = blocks.map((b) => b.kind).lastIndexOf("work");
    feed.innerHTML = `<div class="stream-column">${blocks
      .map((b, i) => {
        if (b.kind === "you")
          return `<div class="stream-you${b.item.pending ? " pending" : ""}">${renderProse(b.item.text, escapeHtml)}</div>`;
        if (b.kind === "agent") {
          const claim = claimedCriterion(b.item.text ?? "");
          const spec = deps.workbench?.()?.current()?.spec;
          const criterion = claim && spec?.criteria?.[claim.number - 1];
          const suggest =
            criterion && !criterion.done && !dismissed.has(`${spec.id}:${claim.number}`)
              ? `<div class="stream-suggest" data-claim="${claim.number}" data-sentence="${escapeHtml(claim.sentence)}">${icon("check", { size: 16 })}<span><b>${escapeHtml(name)} says criterion ${claim.number} is met.</b> ${escapeHtml(criterion.text)}</span><button type="button" class="secondary-button small" data-prove>Mark proven</button><button type="button" class="text-button" data-dismiss-claim aria-label="Not yet">Not yet</button></div>`
              : "";
          return `<div class="stream-agent"><span class="stream-avatar" aria-hidden="true">${escapeHtml(deps.agentMark(session.agent))}</span><div class="stream-body"><div class="stream-who"><b>${escapeHtml(name)}</b><span>${escapeHtml(time(b.item.at))}</span></div>${renderProse(b.item.text, escapeHtml)}</div></div>${suggest}`;
        }
        const failed = b.steps.filter((s) => s.failed).length;
        return `<details class="stream-work"${i === lastWork && i === blocks.length - 1 ? " open" : ""}><summary>${icon("chevron-right", { size: 14 })}<span>${escapeHtml(workSummary(b))}</span>${failed ? `<span class="work-failed">${failed} failed</span>` : ""}</summary><ol>${b.steps
          .map((s) => {
            const w = stepWords(s);
            return `<li class="${s.failed ? "failed" : ""}">${escapeHtml(w.verb)}${w.target ? ` <code>${escapeHtml(w.target)}</code>` : ""}${s.failed ? " · failed" : ""}</li>`;
          })
          .join("")}</ol></details>`;
      })
      .join("")}</div>`;
    if (nearBottom) feed.scrollTop = feed.scrollHeight;
  };

  const chips = () => {
    const wb = deps.workbench?.();
    const spec = wb?.current()?.spec;
    const specChip = form.querySelector(".composer-spec");
    specChip.hidden = !spec;
    if (spec) specChip.textContent = `spec ${spec.id}`;
    const meter = wb?.meterFor(session.id);
    const box = form.querySelector(".composer-meter");
    box.hidden = !meter || meter.tokens == null;
    if (box.hidden) return;
    if (meter.percent != null) {
      box.innerHTML = `<span class="ring" style="--p:${meter.percent}"></span>${meter.percent}%`;
      box.title = `${meter.percent}% of the context window used`;
    } else {
      box.innerHTML = `${compactTokens(meter.tokens)} tokens`;
      box.title = "Tokens in context (this agent does not record its window size)";
    }
  };

  async function refresh() {
    clearTimeout(timer);
    if (mode !== "stream" || terminal.closed) return;
    chips();
    if (!readable) {
      form.hidden = false;
      const sent = pending ? `<div class="stream-column"><div class="stream-you pending">${renderProse(pending, escapeHtml)}</div></div>` : "";
      feed.innerHTML = `${sent}<p class="stream-empty">${escapeHtml(name)} keeps its conversations in a private binary format that Verb cannot read reliably, so its replies stay in the Terminal view. Messages you send from here go straight to it.</p>`;
      return;
    }
    try {
      const data = await deps.api("GET", `/api/terminals/${session.id}/stream`);
      const json = JSON.stringify(data) + (pending ?? "");
      if (json !== lastJson) {
        lastJson = json;
        render(data);
      }
    } catch (error) {
      feed.innerHTML = `<p class="stream-empty">${escapeHtml(error.message)}</p>`;
    }
    if (!document.hidden) timer = setTimeout(refresh, 2500);
    else timer = setTimeout(refresh, 10000);
  }

  form.addEventListener("click", async (event) => {
    if (!event.target.closest("[data-stream-disable]")) return;
    await deps.settings.set(false);
    lastJson = "";
    setMode("terminal");
    deps.toast("The agent stream is off for this project. Verb no longer reads agent logs for it.");
  });
  feed.addEventListener("click", (event) => {
    const card = event.target.closest(".stream-suggest");
    if (!card) return;
    const wb = deps.workbench?.();
    const spec = wb?.current()?.spec;
    const number = Number(card.dataset.claim);
    if (event.target.closest("[data-dismiss-claim]")) {
      if (spec) dismissed.add(`${spec.id}:${number}`);
      card.remove();
    } else if (event.target.closest("[data-prove]")) {
      // The person still reviews and saves the evidence; this only fills in what the agent said.
      wb?.suggestProof(number - 1, `${name} said: "${card.dataset.sentence}"`);
    }
  });
  feed.addEventListener("click", async (event) => {
    if (!event.target.closest("[data-stream-enable]")) return;
    try {
      await deps.settings.set(true);
      lastJson = "";
      refresh();
      deps.toast("The agent stream is on for this project. Turn it off from any stream.");
    } catch (error) {
      deps.toast(error.message, "error");
    }
  });

  const send = () => {
    const text = input.value.trim();
    if (!text) return;
    // A bracketed paste lands as one message even with new lines; Enter then sends it.
    terminal.term.paste(text);
    terminal.term.input("\r", true);
    pending = text;
    input.value = "";
    input.style.height = "";
    lastJson = "";
    refresh();
  };
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    send();
  });
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && !event.shiftKey && !event.isComposing) {
      event.preventDefault();
      send();
    }
  });
  input.addEventListener("input", () => {
    input.style.height = "";
    input.style.height = `${Math.min(input.scrollHeight, 160)}px`;
  });

  // Agent sessions open as a stream when the project has it on; otherwise as the terminal.
  deps.settings.get().then((enabled) => {
    if (enabled && readable && !terminal.closed) setMode("stream");
  });

  return {
    dispose: () => clearTimeout(timer),
    setMode,
  };
}
