// One conversation panel for one hosted agent terminal. The Sessions tile and the Studio work
// thread both mount this; neither keeps a second copy of the conversation or its rules.
//
// Two channels never look alike: this panel is Chat (one whole message, checked by the host and
// confirmed from the agent's own log). The terminal is always one step away for everything chat
// may not do: interrupting, answering a permission prompt, recovering.
import {
  agentTime,
  attemptKey,
  claimedCriterion,
  compactTokens,
  composerBlock,
  continuityMark,
  deliveryLine,
  deviceLine,
  groupStream,
  isStale,
  newMessageId,
  previewableTarget,
  readinessChip,
  reconcile,
  renderProse,
  shortId,
  stepWords,
  streamDevices,
  verbTime,
  workSummary,
} from "./conversation-model.js";

import { conversationPrivacy } from "./conversation-privacy.js";

const VISIBLE_POLL = 1200;
const HIDDEN_POLL = 10000;
const OFFLINE_POLL = 2500;

const storage = {
  get(key) {
    try {
      return JSON.parse(sessionStorage.getItem(key) ?? "null");
    } catch {
      return null;
    }
  },
  set(key, value) {
    try {
      if (value == null) sessionStorage.removeItem(key);
      else sessionStorage.setItem(key, JSON.stringify(value));
    } catch {
      /* Storage unavailable: the draft lives only in this page. */
    }
  },
};

/** This tab's identity for stream acknowledgements: random, kept for the tab's life. */
export function streamClient() {
  let client = storage.get("verb.stream-client");
  if (!client?.id) {
    client = { id: newMessageId().replace(/^m-/, "c-") };
    storage.set("verb.stream-client", client);
  }
  const ua = navigator.userAgent;
  const browser = /Edg\//.test(ua) ? "Edge" : /Chrome\//.test(ua) ? "Chrome" : /Firefox\//.test(ua) ? "Firefox" : /Safari\//.test(ua) ? "Safari" : "Browser";
  const platform = /Android/.test(ua) ? "Android" : /iPhone|iPad/.test(ua) ? "iOS" : /Mac/.test(ua) ? "macOS" : /Linux/.test(ua) ? "Linux" : /Windows/.test(ua) ? "Windows" : "";
  return { id: client.id, label: platform ? `${browser} on ${platform}` : browser };
}

/**
 * Mounts the panel into `container` for terminal `terminalId`.
 * deps: { api, escapeHtml, icon, toast, agentName, agentMark, openTerminal(), openFile?(path),
 *         workbench?(), sessionShort }
 */
export function mountConversation(container, terminalId, deps) {
  const { escapeHtml: esc, icon } = deps;
  const name = deps.agentName;
  const outboxKey = `verb.chat.${terminalId}`;
  const draftKey = `verb.draft.${terminalId}`;
  const client = streamClient();
  const ids = `conv-${terminalId.replace(/[^a-z0-9]/gi, "").slice(0, 12)}-${Math.random().toString(36).slice(2, 7)}`;

  container.classList.add("conv");
  container.innerHTML = `
    <header class="conv-head">
      <dl class="conv-identity" aria-label="Session identity"></dl>
      <div class="conv-status" role="status" aria-live="polite"></div>
    </header>
    <div class="conv-banner" role="status" aria-live="polite" hidden></div>
    <div class="conv-feed" id="${ids}-feed" tabindex="0" aria-label="Conversation with ${esc(name)}"></div>
    <form class="conv-composer" aria-label="Chat with ${esc(name)}">
      <div class="conv-channel"><span class="conv-tag">Chat</span><span>Typing into: <b>Chat</b> · sends one whole message, confirmed from ${esc(name)}'s log</span></div>
      <label class="visually-hidden" for="${ids}-input">Message ${esc(name)}</label>
      <textarea id="${ids}-input" rows="2" placeholder="Message ${esc(name)}" aria-describedby="${ids}-block"></textarea>
      <p class="conv-block" id="${ids}-block"></p>
      <p class="conv-notice" role="alert" hidden></p>
      <div class="conv-actions">
        <span class="conv-chips"></span>
        <button type="button" class="text-button" data-conv="terminal"><span class="conv-tag conv-tag-terminal">Terminal</span> Open terminal</button>
        <button type="button" class="text-button" data-conv="stream-off">Turn off stream</button>
        <button type="submit" class="primary-button conv-send">Send</button>
      </div>
    </form>
    <p class="visually-hidden" aria-live="polite" data-conv-announce></p>`;

  const $ = (s) => container.querySelector(s);
  const feed = $(".conv-feed");
  const input = $("textarea");
  const send = $(".conv-send");

  const state = {
    view: null,
    online: true,
    offlineSince: null,
    tries: 0,
    known: null, // newest stream generation applied here
    acked: null,
    lastAttempt: null,
    banner: null,
    status: null,
    resendConfirm: new Set(),
    dismissedClaims: new Set(),
    timer: null,
    running: false,
    disposed: false,
    lastFeed: "",
    lastAnnounced: new Map(),
    clearedAt: null,
  };

  // This browser's own messages. A request that was in flight when the page went away is "lost":
  // only the host can say whether it was recorded.
  const outbox = new Map();
  for (const entry of storage.get(outboxKey) ?? []) {
    if (entry.state === "posting") entry.state = "lost";
    outbox.set(entry.id, entry);
  }
  input.value = storage.get(draftKey) ?? "";
  const persist = () => {
    storage.set(outboxKey, [...outbox.values()].slice(-16));
    storage.set(draftKey, input.value || null);
  };

  const announce = (text) => {
    const region = $("[data-conv-announce]");
    region.textContent = "";
    requestAnimationFrame(() => (region.textContent = text));
  };
  const notice = (text) => {
    const el = $(".conv-notice");
    el.hidden = !text;
    el.textContent = text ?? "";
  };

  // ---------------------------------------------------------------------------------- render

  function renderHead() {
    const view = state.view;
    const identity = view?.identity;
    const cont = continuityMark(view?.continuity);
    const identityHtml = identity
      ? `<div><dt>Session</dt><dd><code>${esc(identity.session.slice(0, 8))}</code></dd></div>
         <div><dt>Conversation</dt><dd>${identity.conversation ? `<code>${esc(shortId(identity.conversation))}</code>` : "<i>not identified yet</i>"}</dd></div>
         <div><dt>Attempt</dt><dd>#${esc(String(identity.attempt))} · started ${esc(verbTime(identity.attemptStartedAt) ?? "time unavailable")}</dd></div>
         <div class="conv-continuity" data-state="${esc(view.continuity?.state ?? "pending")}" title="${esc(cont.reason)}"><dt>Continuity</dt><dd>${cont.mark ? `<span aria-hidden="true">${esc(cont.mark)}</span> ` : ""}${esc(cont.label)}</dd></div>
         ${view.ended ? `<div><dt>Process</dt><dd>ended</dd></div>` : ""}`
      : view && !view.available
        ? `<div><dt>Session</dt><dd><code>${esc(terminalId.slice(0, 8))}</code></dd></div><div><dt>Process</dt><dd>ended</dd></div>`
        : `<div><dt>Session</dt><dd><code>${esc(terminalId.slice(0, 8))}</code></dd></div>`;
    paint($(".conv-identity"), identityHtml);

    let chip;
    if (!state.online) chip = { label: "Reconnecting", tone: "unknown", basedOn: `This browser lost its connection to Verb${state.offlineSince ? ` at ${verbTime(state.offlineSince)}` : ""}.` };
    else if (!view) chip = { label: "Reading…", tone: "unknown", basedOn: "Nothing read yet." };
    else if (!view.available) chip = { label: "Agent stopped", tone: "stopped", basedOn: view.reason };
    else chip = readinessChip(view.readiness);
    paint($(".conv-status"), `<span class="conv-chip" data-tone="${esc(chip.tone)}">${esc(chip.label)}</span><small class="conv-based"><b>Based on:</b> ${esc(chip.basedOn)}</small>`);
  }

  function renderBanner() {
    const el = $(".conv-banner");
    if (!state.online) {
      el.hidden = false;
      el.dataset.tone = "unknown";
      el.textContent = `Reconnecting · try ${state.tries}. Showing what was last confirmed${state.view?.lastReadAt ? ` as of ${verbTime(state.view.lastReadAt)}` : ""}. Your draft is kept.`;
      return;
    }
    if (state.banner && state.banner.until > Date.now()) {
      el.hidden = false;
      el.dataset.tone = state.banner.tone;
      el.textContent = state.banner.text;
      return;
    }
    el.hidden = true;
  }

  function messageBubble(message, { fromLog = false } = {}) {
    const line = deliveryLine(message, name);
    const buttons = line.actions
      .map((action) => {
        if (action === "check") return `<button type="button" class="secondary-button small" data-msg="check" data-id="${esc(message.id)}">Check the conversation</button>`;
        if (action === "resend") {
          const confirming = state.resendConfirm.has(message.id);
          if (!message.text) return "";
          return `<button type="button" class="secondary-button small${confirming ? " danger" : ""}" data-msg="resend" data-id="${esc(message.id)}">${confirming ? "Send again — it may arrive twice" : "Send again (may duplicate)"}</button>`;
        }
        if (action === "discard") return `<button type="button" class="text-button" data-msg="discard" data-id="${esc(message.id)}">Discard</button>`;
        if (action === "edit") return message.text ? `<button type="button" class="text-button" data-msg="edit" data-id="${esc(message.id)}">Edit and send again</button>` : "";
        if (action === "terminal") return `<button type="button" class="text-button" data-conv="terminal">Open in terminal</button>`;
        return "";
      })
      .join("");
    const text = message.text
      ? renderProse(message.text, esc)
      : `<p class="conv-muted">${message.fromElsewhere ? `Sent from ${message.origin === "phone" ? "the paired phone" : "another browser"}.` : "Message text not shown."}</p>`;
    return `<div class="conv-you conv-delivery" data-state="${esc(message.state)}"${fromLog ? "" : ` data-id="${esc(message.id)}"`}>
      ${fromLog ? "" : text}
      <div class="conv-delivery-line" data-tone="${esc(line.tone)}"><span>${esc(line.label)}</span>${line.checked ? `<small>${esc(line.checked)} · no proof either way</small>` : ""}</div>
      ${line.detail ? `<p class="conv-delivery-detail">${esc(line.detail)}</p>` : ""}
      ${buttons ? `<div class="conv-delivery-actions">${buttons}</div>` : ""}
    </div>`;
  }

  function renderFeed() {
    const view = state.view;
    const messages = reconcile(outbox, view);
    const parts = [];

    if (!state.online && !view) {
      parts.push(`<p class="conv-empty">Reconnecting to Verb…</p>`);
    } else if (view && !view.available) {
      parts.push(`<div class="conv-card" data-tone="stopped"><h4>${esc(name)} stopped</h4><p>${esc(view.reason)}</p><p>Its session record says whether it can be resumed. Resume or start a new conversation from the session's terminal.</p><button type="button" class="secondary-button" data-conv="terminal">Open the session</button></div>`);
    } else if (view && !view.stream?.enabled) {
      parts.push(streamOffCard());
    } else if (view?.readiness?.state === "unsupported") {
      parts.push(`<div class="conv-card" data-tone="off"><h4>${esc(name)}'s conversation can't be shown here</h4><p>${esc(view.readiness.reason)}</p><p>The session's status is still tracked. Work in the terminal.</p><button type="button" class="primary-button" data-conv="terminal">Open terminal</button></div>`);
    } else if (view) {
      if (view.earlierOmitted) parts.push(`<p class="conv-note">Earlier history is not shown: Verb reads at most the last part of a long session log.</p>`);
      if (view.readProblem) parts.push(`<div class="conv-card" data-tone="problem"><h4>Conversation can't be read</h4><p>${esc(view.readProblem)}</p><p>Showing what Verb last read${view.lastReadAt ? `, up to ${esc(verbTime(view.lastReadAt))}` : ""}.</p><button type="button" class="secondary-button small" data-conv="reread">Try reading again</button> <button type="button" class="text-button" data-conv="terminal">Open in terminal</button></div>`);
      if (view.continuity?.state === "not_established") parts.push(`<div class="conv-card conv-divider" data-tone="unknown"><h4><span aria-hidden="true">?</span> Continuity not established</h4><p>${esc(view.continuity.reason)}</p><p>Nothing from this attempt is joined to earlier history. Chat stays closed until the evidence settles it.</p><button type="button" class="secondary-button small" data-conv="reread">Check again</button> <button type="button" class="text-button" data-conv="terminal">Open in terminal</button></div>`);
      const received = new Map(messages.filter((m) => m.state === "received" && m.turnId).map((m) => [m.turnId, m]));
      const items = view.items ?? [];
      if (!items.length && !messages.length) parts.push(`<p class="conv-empty">No messages yet. ${view.canSend ? `Ask ${esc(name)} something below.` : "The conversation appears here once the agent writes it."}</p>`);
      const blocks = groupStream(items);
      blocks.forEach((b, i) => {
        if (b.kind === "you") {
          const mine = b.item.id && received.get(b.item.id);
          parts.push(`<div class="conv-you"><div class="conv-who"><b>You</b><span>${timeHtml(b.item.at)}</span></div>${renderProse(b.item.text ?? "", esc)}${mine ? `<div class="conv-delivery-line" data-tone="ok"><span>${esc(deliveryLine(mine, name).label)}</span></div>` : ""}</div>`);
        } else if (b.kind === "agent") {
          parts.push(`<div class="conv-agent"><span class="conv-avatar" aria-hidden="true">${esc(deps.agentMark)}</span><div class="conv-body"><div class="conv-who"><b>${esc(name)}</b><span>${timeHtml(b.item.at)}</span></div>${renderProse(b.item.text ?? "", esc)}</div></div>${claimCard(b.item.text ?? "")}`);
        } else {
          const failed = b.steps.filter((s) => s.failed).length;
          const last = i === blocks.length - 1;
          const still = last && view.readiness?.state === "working";
          parts.push(`<details class="conv-work"${last ? " open" : ""}><summary>${icon("chevron-right", { size: 14 })}<span>${esc(workSummary(b))}</span>${still ? `<span class="conv-running">running</span>` : ""}${failed ? `<span class="conv-failed">${failed} failed</span>` : ""}</summary><ol>${b.steps.map(stepHtml).join("")}</ol></details>`);
        }
      });
      if (view.readiness?.state === "working" && blocks.at(-1)?.kind === "agent") parts.push(`<p class="conv-note" aria-hidden="true">Still writing…</p>`);
    }
    for (const message of messages) {
      if (message.state === "received" && message.turnId && (view?.items ?? []).some((i) => i.id === message.turnId)) continue;
      if (message.replaced) {
        parts.push(`<div class="conv-you conv-delivery" data-state="replaced">${message.text ? renderProse(message.text, esc) : ""}<div class="conv-delivery-line"><span>Replaced by a new attempt</span></div></div>`);
        continue;
      }
      parts.push(messageBubble(message));
    }
    if (view?.available && view.stream?.enabled && (view.readiness?.state === "approval" || view.permissionSign)) {
      parts.push(`<div class="conv-card conv-approval" data-tone="attention" role="group" aria-label="Permission prompt"><h4>${esc(name)} looks like it is asking for permission</h4><p>${esc(view.readiness?.basedOn?.join(" ") ?? "")}</p><p>Verb can't answer a permission prompt from chat. Read it and answer in the terminal, where you can also press Esc or Ctrl-C to decline.</p><p class="conv-muted">Choosing "don't ask again" there changes ${esc(name)}'s own permissions for this project.</p><button type="button" class="primary-button" data-conv="terminal">Open in terminal to approve</button></div>`);
    }
    const html = parts.join("");
    if (html !== state.lastFeed) {
      const nearBottom = feed.scrollHeight - feed.scrollTop - feed.clientHeight < 80;
      feed.innerHTML = html;
      state.lastFeed = html;
      if (nearBottom) feed.scrollTop = feed.scrollHeight;
    }
    for (const message of messages) {
      const before = state.lastAnnounced.get(message.id);
      if (before && before !== message.state) announce(`Your message: ${deliveryLine(message, name).label}`);
      state.lastAnnounced.set(message.id, message.state);
    }
  }

  function timeHtml(iso) {
    const at = agentTime(iso);
    return at ? esc(at) : `<i class="conv-muted">time unavailable</i>`;
  }

  function stepHtml(step) {
    const w = stepWords(step);
    const file = deps.openFile ? previewableTarget(step) : null;
    const target = w.target ? (file ? `<button type="button" class="conv-file" data-preview="${esc(file)}" title="Open in the preview pane"><code>${esc(w.target)}</code></button>` : `<code>${esc(w.target)}</code>`) : "";
    return `<li class="${step.failed ? "failed" : ""}">${esc(w.verb)} ${target}${step.failed ? ` <span class="conv-failed">failed</span>` : ""}</li>`;
  }

  function claimCard(text) {
    const wb = deps.workbench?.();
    const claim = claimedCriterion(text);
    const spec = wb?.current()?.spec;
    const criterion = claim && spec?.criteria?.[claim.number - 1];
    if (!criterion || criterion.done || state.dismissedClaims.has(`${spec.id}:${claim.number}`)) return "";
    return `<div class="conv-suggest" data-claim="${claim.number}" data-sentence="${esc(claim.sentence)}">${icon("check", { size: 16 })}<span><b>${esc(name)} says criterion ${claim.number} is met.</b> ${esc(criterion.text)} <small>A claim, not proof: review the evidence before recording it.</small></span><button type="button" class="secondary-button small" data-prove>Review as evidence</button><button type="button" class="text-button" data-dismiss-claim>Not yet</button></div>`;
  }

  function streamOffCard() {
    // Devices matter only once the stream has been on: a project that never turned it on has
    // nothing to clear anywhere.
    const everOn = state.clearedAt || (state.status?.generation ?? state.view?.stream?.generation ?? 0) > 0;
    const rows = everOn ? streamDevices(state.status, state.view?.phone, client.id) : [];
    const devices = rows.length
      ? `<ul class="conv-devices" aria-label="Where the conversation was showing">${rows
          .map((r) => `<li data-state="${esc(r.state)}"><b>${esc(r.name)}</b> <span>${esc(r.self ? (state.clearedAt ? `Stream turned off on this device · ${verbTime(state.clearedAt)}` : "Not showing the conversation") : deviceLine(r))}</span></li>`)
          .join("")}</ul><p class="conv-muted">A device that has not confirmed clears when it next reconnects. Closing it also works.</p>`
      : "";
    return `<div class="conv-card" data-tone="off"><h4>Stream off</h4>
      <p>Verb shows the session's state and Git changes, but not the conversation. ${esc(name)}'s own log is untouched.</p>
      ${devices}
      <p><b>Turning it on</b> lets Verb read, for this project only, what you asked ${esc(name)}, its answers and the files and commands it worked on, from the log it keeps on this machine: read when shown, never stored, never sent to a model, with keys and passwords hidden. Sending from chat needs it, because delivery is confirmed from that log.</p>
      <button type="button" class="primary-button" data-conv="stream-on">Turn on for this project</button></div>`;
  }

  function renderComposer() {
    const view = state.view;
    const block = composerBlock(view, { online: state.online, localPending: [...outbox.values()].some((d) => ["posting", "sending", "sent", "unknown", "lost"].includes(d.state)) });
    const unsupported = view?.sendContract === "unsupported" || view?.readiness?.state === "unsupported" || (view && !view.available);
    container.classList.toggle("conv-terminal-only", Boolean(unsupported));
    send.disabled = Boolean(block);
    send.setAttribute("aria-disabled", String(Boolean(block)));
    paintText($(".conv-block"), block ?? "");
    $(".conv-block").hidden = !block;
    $('[data-conv="stream-off"]').hidden = !view?.stream?.enabled;
    const wb = deps.workbench?.();
    const spec = wb?.current()?.spec;
    const meter = wb?.meterFor?.(terminalId);
    const chips = [];
    if (spec) chips.push(`<span class="composer-chip">spec ${esc(spec.id)}</span>`);
    if (meter?.tokens != null) chips.push(`<span class="composer-chip" title="${meter.percent != null ? `${meter.percent}% of the context window used` : "Tokens in context"}">${meter.percent != null ? `${meter.percent}% context` : `${esc(compactTokens(meter.tokens))} tokens`}</span>`);
    paint($(".conv-chips"), chips.join(""));
  }

  function render() {
    if (state.disposed) return;
    renderHead();
    renderBanner();
    renderFeed();
    renderComposer();
  }

  function paint(el, html) {
    if (el.innerHTML !== html) el.innerHTML = html;
  }
  function paintText(el, text) {
    if (el.textContent !== text) el.textContent = text;
  }

  // ---------------------------------------------------------------------------- the host

  function wentOffline() {
    if (state.online) {
      state.online = false;
      state.offlineSince = Date.now();
      state.tries = 0;
    }
    state.tries += 1;
  }

  function cameBack(view) {
    if (state.online) return;
    state.online = true;
    const same = attemptKey(view) && attemptKey(view) === state.lastAttempt;
    state.banner = {
      tone: same ? "ok" : "attention",
      until: Date.now() + 8000,
      text: !view?.available
        ? "Reconnected. The agent's process has ended."
        : same
          ? "Reconnected. Same process, same conversation."
          : `Reconnected. The agent restarted: this is attempt #${view.identity?.attempt}.`,
    };
    announce(state.banner.text);
  }

  /** Applies a host response, unless it began before the stream last changed here. */
  function accept(view) {
    if (state.disposed || !view || isStale(view, state.known)) return false;
    if (!conversationPrivacy.apply(view.stream)) return false;
    const generation = view.stream?.generation;
    if (Number.isFinite(generation) && (state.known == null || generation > state.known)) {
      const wasOn = state.view?.stream?.enabled;
      state.known = generation;
      if (wasOn && !view.stream.enabled) clearContent();
    }
    cameBack(view);
    // An ended process takes its engine with it; the identities it had are still true history.
    if (!view.available && state.view?.identity) view = { ...view, identity: state.view.identity, continuity: state.view.continuity, ended: true };
    state.view = view;
    if (view.available && attemptKey(view)) state.lastAttempt = attemptKey(view);
    reconcile(outbox, view);
    persist();
    render();
    acknowledge();
    return true;
  }

  /** Conversation text leaves this page; the person's own unsent draft stays. */
  function clearContent() {
    for (const message of outbox.values()) delete message.text;
    state.lastFeed = "";
    if (state.view) state.view = { ...state.view, items: [], deliveries: (state.view.deliveries ?? []).map((d) => {
      const { text, ...record } = d;
      return record;
    }) };
    persist();
  }

  const unsubscribePrivacy = conversationPrivacy.subscribe((setting) => {
    if (Number.isFinite(setting.generation)) state.known = Math.max(state.known ?? 0, setting.generation);
    if (!setting.enabled) {
      clearContent();
      // Clear records belonging to panels that were previously disposed, too. Drafts are separate.
      try {
        for (const key of Object.keys(sessionStorage).filter((key) => key.startsWith("verb.chat."))) {
          const records = storage.get(key);
          if (Array.isArray(records)) storage.set(key, records.map(({ text, ...record }) => record));
        }
      } catch { /* Storage may be unavailable. Mounted content is still cleared. */ }
    }
    if (state.view) state.view = { ...state.view, stream: { ...state.view.stream, ...setting } };
    state.lastFeed = "";
    render();
  });

  async function acknowledge() {
    const generation = state.known;
    if (!Number.isFinite(generation)) return;
    // A new generation is acknowledged at once; the same one is repeated now and then, so other
    // devices can see when this browser was last showing anything.
    const fresh = generation !== state.acked;
    if (!fresh && Date.now() - (state.ackedAt ?? 0) < 30000) return;
    state.acked = generation;
    state.ackedAt = Date.now();
    try {
      state.status = await deps.api("POST", "/api/stream/ack", { clientId: client.id, label: client.label, generation });
    } catch {
      state.acked = null; // Try again on the next change.
    }
  }

  async function poll() {
    clearTimeout(state.timer);
    if (state.disposed) return;
    if (!state.running) {
      // Pausing content polling must not pause privacy revocation for a hidden panel.
      try {
        const setting = await deps.api("GET", "/api/stream");
        if (!state.disposed && conversationPrivacy.apply(setting)) {
          state.status = setting;
          render();
          acknowledge();
        }
      } catch { /* Offline clients never claim a new acknowledgement. */ }
      if (!state.disposed && !state.running) state.timer = setTimeout(poll, VISIBLE_POLL);
      return;
    }
    try {
      const view = await deps.api("GET", `/api/terminals/${encodeURIComponent(terminalId)}/conversation`);
      accept(view);
      if (view && !view.stream?.enabled) {
        state.status = await deps.api("GET", "/api/stream").catch(() => state.status);
        render();
      }
      notice(null);
    } catch (error) {
      if (!error.status) wentOffline();
      else if (error.status === 400 && /not hosted/.test(error.message)) {
        state.view = { available: false, reason: "This terminal is no longer hosted here.", stream: state.view?.stream };
        reconcile(outbox, state.view);
      } else notice(error.message);
      render();
    }
    if (state.disposed || !state.running) return;
    const delay = !state.online ? OFFLINE_POLL : document.hidden ? HIDDEN_POLL : VISIBLE_POLL;
    state.timer = setTimeout(poll, delay);
  }

  async function sendMessage(text, { replacing } = {}) {
    const block = composerBlock(state.view, { online: state.online, localPending: [...outbox.values()].some((d) => ["posting", "sending", "sent", "unknown", "lost"].includes(d.state)) });
    if (block && !replacing) {
      notice(block);
      return false;
    }
    const id = newMessageId();
    outbox.set(id, { id, text, state: "posting", attempt: attemptKey(state.view), createdAt: Date.now() });
    persist();
    render();
    try {
      const view = await deps.api("POST", `/api/terminals/${encodeURIComponent(terminalId)}/chat`, { messageId: id, text });
      const entry = outbox.get(id);
      if (entry) entry.seenByHost = true;
      accept(view);
      return true;
    } catch (error) {
      const entry = outbox.get(id);
      if (!error.status) {
        // The request may or may not have reached Verb. Only the host can say; never resend.
        if (entry) entry.state = "lost";
        wentOffline();
      } else {
        // Refused: nothing was typed. The words go back where they came from.
        outbox.delete(id);
        if (!conversationPrivacy.blocked && !input.value.trim()) input.value = text;
        notice(`Not sent: ${error.message}`);
        announce(`Not sent: ${error.message}`);
      }
      persist();
      render();
      return false;
    }
  }

  async function messageAction(action, id) {
    const message = outbox.get(id);
    const path = `/api/terminals/${encodeURIComponent(terminalId)}/chat/${encodeURIComponent(id)}`;
    if (action === "check") {
      try {
        accept(await deps.api("POST", `${path}/check`));
      } catch (error) {
        if (!error.status) wentOffline();
        else if (message) {
          message.state = "unknown";
          message.checkedAt = Date.now();
          message.reason = error.message;
        }
        persist();
        render();
      }
    } else if (action === "discard") {
      if (message?.seenByHost || message?.state === "lost") await deps.api("POST", `${path}/discard`).catch(() => {});
      if (message) {
        const mayHave = ["sent", "unknown", "lost"].includes(message.state);
        message.state = "discarded";
        message.reason = mayHave ? "Discarded by you · may still have reached the agent" : "Discarded by you.";
      }
      persist();
      render();
    } else if (action === "resend") {
      if (!message?.text) return;
      if (!state.resendConfirm.has(id)) {
        state.resendConfirm.add(id);
        render();
        feed.querySelector(`[data-msg="resend"][data-id="${CSS.escape(id)}"]`)?.focus();
        return;
      }
      state.resendConfirm.delete(id);
      // The earlier one is set aside first: the host never lets a new message overtake it.
      await deps.api("POST", `${path}/discard`).catch(() => {});
      message.replaced = true;
      message.state = "discarded";
      persist();
      await poll();
      await sendMessage(message.text, { replacing: true });
    } else if (action === "edit") {
      if (message?.text) {
        input.value = message.text;
        outbox.delete(id);
        persist();
        render();
        input.focus();
      }
    }
  }

  async function setStream(enabled) {
    if (!enabled) {
      // Clear first, then ask: the content leaves this screen whatever the answer.
      conversationPrivacy.hide();
      state.clearedAt = Date.now();
      if (state.view) state.view = { ...state.view, stream: { ...state.view.stream, enabled: false } };
      render();
    }
    try {
      const status = await deps.api("POST", "/api/stream", { enabled, clientId: client.id, label: client.label });
      conversationPrivacy.apply(status, { explicit: true });
      state.status = status;
      state.known = Math.max(state.known ?? 0, status.generation);
      state.acked = enabled ? null : status.generation;
      if (enabled) state.clearedAt = null;
      deps.onStreamChange?.(status);
      deps.toast(enabled ? "The stream is on for this project." : "The stream is off for this project. Verb no longer reads agent logs for it.");
    } catch (error) {
      notice(enabled ? `Could not turn the stream on: ${error.message}` : `This browser hid the conversation, but the stream is still on: ${error.message}`);
    }
    await poll();
  }

  // ---------------------------------------------------------------------------------- events

  container.addEventListener("click", async (event) => {
    const conv = event.target.closest("[data-conv]");
    if (conv) {
      const action = conv.dataset.conv;
      if (action === "terminal") deps.openTerminal();
      else if (action === "stream-off") setStream(false);
      else if (action === "stream-on") setStream(true);
      else if (action === "reread") poll();
      return;
    }
    const msg = event.target.closest("[data-msg]");
    if (msg) {
      messageAction(msg.dataset.msg, msg.dataset.id);
      return;
    }
    const file = event.target.closest("[data-preview]");
    if (file) {
      deps.openFile?.(file.dataset.preview);
      return;
    }
    const card = event.target.closest(".conv-suggest");
    if (card) {
      const wb = deps.workbench?.();
      const spec = wb?.current()?.spec;
      const number = Number(card.dataset.claim);
      if (event.target.closest("[data-dismiss-claim]")) {
        if (spec) state.dismissedClaims.add(`${spec.id}:${number}`);
        state.lastFeed = "";
        render();
      } else if (event.target.closest("[data-prove]")) {
        wb?.suggestProof(number - 1, `${name} said: "${card.dataset.sentence}"`);
      }
    }
  });

  $(".conv-composer").addEventListener("submit", async (event) => {
    event.preventDefault();
    const text = input.value.trim();
    if (!text) {
      input.focus();
      return;
    }
    const before = input.value;
    const generation = state.known;
    input.value = "";
    persist();
    const sent = await sendMessage(text);
    if (!sent && !input.value && !conversationPrivacy.blocked && generation === state.known) input.value = before;
    persist();
  });
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && !event.shiftKey && !event.isComposing) {
      event.preventDefault();
      $(".conv-composer").requestSubmit();
    }
  });
  input.addEventListener("input", () => {
    input.style.height = "";
    input.style.height = `${Math.min(input.scrollHeight, 180)}px`;
    persist();
  });
  container.addEventListener("keydown", (event) => {
    if (event.ctrlKey && event.key === "`") {
      event.preventDefault();
      deps.openTerminal();
    }
  });
  const onVisible = () => {
    if (!document.hidden && state.running) poll();
  };
  document.addEventListener("visibilitychange", onVisible);
  const onOnline = () => state.running && poll();
  window.addEventListener("online", onOnline);

  render();

  return {
    start() {
      if (state.running) return;
      state.running = true;
      poll();
    },
    stop() {
      state.running = false;
      clearTimeout(state.timer);
      poll();
    },
    refresh: () => poll(),
    focus: () => input.focus(),
    dispose() {
      unsubscribePrivacy();
      clearContent();
      container.replaceChildren();
      state.disposed = true;
      state.running = false;
      clearTimeout(state.timer);
      document.removeEventListener("visibilitychange", onVisible);
      window.removeEventListener("online", onOnline);
    },
  };
}
