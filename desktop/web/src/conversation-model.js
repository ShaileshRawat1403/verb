// The conversation's words and rules, with no DOM: what each state is called, what it is based on,
// and how this browser reconciles the messages it sent with what the host says happened to them.
// Every rule here mirrors the host (src/conversation.rs); the host is the authority.

const STREAM_AGENTS = new Set(["claude", "codex"]);

/** Whether a session's agent has a conversation Verb can read. */
export function hasStream(agent) {
  return STREAM_AGENTS.has(agent);
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

/** "Worked for 3 min · 6 steps", from the agent's own timestamps when the log has them. */
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

/** A step that names a project file the preview pane can open. */
export function previewableTarget(step) {
  if (!["Read", "Write", "Edit", "MultiEdit"].includes(step.tool)) return null;
  const target = String(step.target ?? "");
  if (!target || target.startsWith("/") || target.includes("..") || /[\n,]/.test(target)) return null;
  return target;
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

// ------------------------------------------------------------------------------------- times

/**
 * The agent's own time for something, or `null` when the log gave none it could use. Callers show
 * null as "time unavailable" — never as the time it was read.
 */
export function agentTime(iso, { seconds = false } = {}) {
  if (typeof iso !== "string" || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{1,9})?Z$/.test(iso)) return null;
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return null;
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", ...(seconds ? { second: "2-digit" } : {}) });
}

/** Verb's own clock (milliseconds), for things Verb itself did: sent, checked, cleared. */
export function verbTime(millis, { seconds = true } = {}) {
  if (!Number.isFinite(millis) || millis <= 0) return null;
  return new Date(millis).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", ...(seconds ? { second: "2-digit" } : {}) });
}

// --------------------------------------------------------------------------------- readiness

const READINESS = {
  ready: { label: "Waiting for you", tone: "ok" },
  working: { label: "Working", tone: "busy" },
  approval: { label: "Looks like a permission prompt", tone: "attention" },
  unknown: { label: "Status unknown", tone: "unknown" },
  ended: { label: "Agent stopped", tone: "stopped" },
  unidentified: { label: "Conversation not identified", tone: "unknown" },
  unreadable: { label: "Conversation can't be read", tone: "problem" },
  stream_off: { label: "Stream off", tone: "off" },
  unsupported: { label: "Terminal only", tone: "off" },
};

/** The chip for the agent's state, and the evidence line under it. */
export function readinessChip(readiness) {
  const known = READINESS[readiness?.state] ?? READINESS.unknown;
  const basedOn = readiness?.basedOn?.length ? readiness.basedOn.join(" ") : "No evidence yet.";
  return { ...known, reason: readiness?.reason ?? "", basedOn };
}

/** Why the composer is closed, in the person's terms; null when it is open. */
export function composerBlock(view, { online = true, localPending = false } = {}) {
  if (!online) return "You're offline from Verb. Your draft is kept; it can be sent once Verb is reachable again.";
  if (!view) return "Reading the conversation…";
  if (!view.available) return view.reason ?? "This conversation is not available here.";
  if (view.sendContract === "unsupported") return view.sendNote ?? "Send from the terminal.";
  if (!view.stream?.enabled) return "Sending needs the stream on, because Verb confirms delivery from the agent's log.";
  const unresolved = (view.deliveries ?? []).find((d) => ["sending", "sent", "unknown"].includes(d.state));
  if (unresolved) return "Waiting until the earlier message is confirmed or resolved, so a new one can't overtake it.";
  if (localPending) return "An earlier message has uncertain delivery. Check or discard it before sending another.";
  if (!view.canSend) return view.readiness?.reason || "The agent isn't ready for a message.";
  return null;
}

// -------------------------------------------------------------------------------- continuity

const CONTINUITY = {
  pending: { label: "Identifying conversation…", mark: "…" },
  first_attempt: { label: "First attempt", mark: "" },
  resumed: { label: "Conversation resumed", mark: "" },
  new_conversation: { label: "New conversation", mark: "+" },
  not_established: { label: "Continuity not established", mark: "?" },
};

export function continuityMark(continuity) {
  return { ...(CONTINUITY[continuity?.state] ?? CONTINUITY.pending), reason: continuity?.reason ?? "" };
}

/** A conversation id shown short enough to read: "3f1c…a92e". */
export function shortId(id) {
  if (!id) return null;
  return id.length <= 12 ? id : `${id.slice(0, 4)}…${id.slice(-4)}`;
}

/** The execution attempt's identity. A change means the process restarted. */
export function attemptKey(view) {
  const identity = view?.identity;
  return identity ? `${identity.attempt}:${identity.attemptStartedAt}` : null;
}

// ---------------------------------------------------------------------------------- delivery

/** A new message id: random, so a retried request can be recognised by the host as the same one. */
export function newMessageId(random = () => crypto.getRandomValues(new Uint8Array(12))) {
  return `m-${Array.from(random(), (b) => b.toString(16).padStart(2, "0")).join("")}`;
}

/**
 * Brings this browser's own record of its messages up to date with the host's view.
 *
 * `outbox`: Map id → { id, text, state, attempt, reason? } — what this browser sent.
 *   state "posting": the request is in flight; "lost": the request failed without an answer.
 * The host is authoritative for any message it lists. For one it does not list:
 *   - still posting: unchanged;
 *   - lost, same attempt: a snapshot cannot fence a delayed request — unknown;
 *   - otherwise, with a different attempt: the process restarted, so it is unknown;
 *   - otherwise: the host no longer knows it — unknown.
 * Returns the messages to show, oldest first.
 */
export function reconcile(outbox, view, now = Date.now()) {
  const server = new Map((view?.deliveries ?? []).map((d) => [d.id, d]));
  const attempt = attemptKey(view);
  for (const [id, local] of outbox) {
    const known = server.get(id);
    if (known) {
      local.state = known.state;
      local.reason = known.reason ?? null;
      local.sentAt = known.sentAt ?? local.sentAt;
      local.receivedAt = known.receivedAt ?? null;
      local.turnId = known.turnId ?? null;
      local.checkedAt = known.checkedAt ?? null;
      local.attempt = local.attempt ?? attempt;
      local.seenByHost = true;
      continue;
    }
    if (local.state === "posting" || local.replaced || ["received", "failed", "discarded"].includes(local.state)) continue;
    if (!view?.available) {
      if (local.seenByHost || local.state === "lost") {
        local.state = "unknown";
        local.reason = "The agent's process ended before Verb confirmed this message.";
      }
      continue;
    }
    if (local.state === "lost" && !local.seenByHost && local.attempt === attempt) {
      local.state = "unknown";
      local.reason = "Verb has no receipt yet. The original request may still arrive; do not assume it was not sent.";
    } else if (local.attempt && attempt && local.attempt !== attempt) {
      local.state = "unknown";
      local.reason = "The agent restarted before Verb confirmed this message.";
    } else if (local.seenByHost || local.state === "lost") {
      local.state = "unknown";
      local.reason = "Verb no longer has a record of this message.";
    }
  }
  const shown = [...outbox.values()];
  for (const delivery of server.values()) {
    if (outbox.has(delivery.id)) continue;
    // Another device's messages: shown while they need attention, not forever.
    const open = ["sending", "sent", "unknown"].includes(delivery.state);
    const recent = now - (delivery.createdAt ?? 0) < 10 * 60 * 1000;
    if (open || (delivery.state === "failed" && recent)) shown.push({ ...delivery, fromElsewhere: true });
  }
  return shown.sort((a, b) => (a.createdAt ?? 0) - (b.createdAt ?? 0) || 0);
}

/** The line under a sent message, and which recovery actions it offers. */
export function deliveryLine(message, agentName, now = Date.now()) {
  const actions = [];
  let label;
  let tone = "neutral";
  switch (message.state) {
    case "posting":
    case "sending":
      label = "Sending…";
      break;
    case "sent": {
      label = "Sent to the terminal · waiting for the agent's log to show it";
      if (message.sentAt && now - message.sentAt > 15000) actions.push("check");
      actions.push("terminal");
      break;
    }
    case "received": {
      const at = agentTime(message.receivedAt, { seconds: true });
      label = `Received · in the agent's log ${at ?? "(time unavailable)"}`;
      tone = "ok";
      break;
    }
    case "failed":
      label = message.reason ?? "Not sent.";
      tone = "problem";
      actions.push("edit", "terminal");
      break;
    case "unknown":
    case "lost":
      label = `Delivery unknown. It may or may not have reached ${agentName}.`;
      tone = "attention";
      actions.push("check", "resend", "discard", "terminal");
      break;
    case "discarded":
      label = message.reason ?? "Discarded by you.";
      break;
    default:
      label = "Status unknown";
  }
  const checked = message.checkedAt ? `Checked ${verbTime(message.checkedAt)}` : null;
  return { label, tone, actions, detail: message.state === "received" ? null : message.reason ?? null, checked };
}

// ------------------------------------------------------------------------------------ stream

/**
 * Drops a response from before the stream changed. `known` is the newest generation this browser
 * has applied; a response naming an older one began before the change and must not be shown.
 */
export function isStale(response, known) {
  const generation = response?.stream?.generation;
  return Number.isFinite(known) && Number.isFinite(generation) && generation < known;
}

/**
 * Every place conversation text may still be showing, and whether each has confirmed it cleared.
 * `status`: GET /api/stream. `phone`: the conversation view's phone entry. `self`: this browser's id.
 */
export function streamDevices(status, phone, selfId) {
  const generation = status?.generation ?? 0;
  const rows = [];
  for (const client of status?.clients ?? []) {
    const self = client.id === selfId;
    const current = client.ackedGeneration >= generation;
    rows.push({
      name: self ? "This browser" : client.label,
      self,
      state: self ? "cleared" : current ? "acknowledged" : "not_confirmed",
      at: client.ackedAt,
      seenAt: client.seenAt,
    });
  }
  if (!rows.some((r) => r.self)) rows.unshift({ name: "This browser", self: true, state: "cleared", at: null });
  if (phone?.paired) {
    const current = (phone.ackedGeneration ?? -1) >= generation;
    rows.push({ name: "Paired phone", self: false, state: current ? "acknowledged" : "not_confirmed", at: current ? phone.ackedAt : null, connected: phone.connected });
  }
  return rows;
}

export function deviceLine(row) {
  const at = verbTime(row.at);
  if (row.state === "cleared") return `Stream turned off on this device${at ? ` · ${at}` : ""}`;
  if (row.state === "acknowledged") return `acknowledged · cleared${at ? ` ${at}` : ""}`;
  const seen = verbTime(row.seenAt, { seconds: false });
  return `not yet confirmed cleared${row.connected === false ? " · not connected" : ""}${seen ? ` · last seen ${seen}` : ""}`;
}
