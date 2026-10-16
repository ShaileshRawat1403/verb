// Talk: the conversation view for Antigravity (server: src/talk.rs). agy answers in its JSON mode,
// where it cannot ask permission; anything that needs it becomes a card that continues the same
// conversation in agy's terminal, where the person approves each action.

import { claimedCriterion, groupStream, renderProse, stepWords, workSummary } from "./stream-ui.js";

/** The state a talk's tab shows: working, waiting on a permission, or idle. */
export function talkTone(talk) {
  if (!talk) return "ok";
  if (talk.working) return "running";
  const last = talk.items?.[talk.items.length - 1];
  return last?.kind === "permission" ? "waiting" : "ok";
}

/** The newest permission agy was refused, if the talk is stopped on one. */
export function pendingPermission(talk) {
  const last = talk?.items?.[talk.items.length - 1];
  return last?.kind === "permission" ? last : null;
}

/**
 * Creates the tile for one talk. `deps`: { api, escapeHtml, icon, toast, workbench(), onChange(),
 * openTerminal(sessionId), remove(id) }. Returns { id, tile, data(), dispose() }.
 */
export function createTalk(id, deps) {
  const { escapeHtml, icon } = deps;
  const tile = document.createElement("div");
  tile.className = "terminal-tile talk-tile";
  tile.dataset.talkId = id;
  tile.innerHTML = `<div class="terminal-titlebar"><span class="pane-agent" aria-hidden="true">AG</span><span class="terminal-title">Antigravity <small>Talk</small></span><span class="grow"></span><button type="button" class="secondary-button small" data-talk-terminal title="Continue this conversation in Antigravity's terminal, where it can ask you before running commands or editing files">${icon("terminal", { size: 14 })}Continue in terminal</button><button type="button" class="terminal-close" data-talk-end title="End this talk. The conversation stays in Antigravity's history.">End</button></div>
    <div class="talk-body agent-stream"><div class="stream-feed" aria-live="polite"></div><form class="stream-composer"><textarea rows="1" placeholder="Message Antigravity" aria-label="Message Antigravity"></textarea><div class="composer-row"><span class="composer-chip"><span class="stream-avatar" aria-hidden="true">AG</span>Antigravity</span><span class="composer-chip composer-spec" hidden></span><span class="composer-hint">Talk reads and plans · it asks in the terminal before acting</span><span class="composer-meter" hidden></span><button type="submit" class="composer-send" aria-label="Send">${icon("arrow-up", { size: 16 })}</button></div></form></div>`;
  const feed = tile.querySelector(".stream-feed");
  const form = tile.querySelector(".stream-composer");
  const input = form.querySelector("textarea");
  const send = form.querySelector(".composer-send");
  let data = null;
  let lastJson = "";
  let timer = null;
  let closed = false;
  const dismissed = new Set();

  const render = () => {
    const spec = deps.workbench?.()?.current()?.spec;
    const specChip = form.querySelector(".composer-spec");
    specChip.hidden = !data?.spec_id;
    if (data?.spec_id) specChip.textContent = `spec ${data.spec_id}`;
    const meter = form.querySelector(".composer-meter");
    meter.hidden = !data?.tokens;
    if (data?.tokens) meter.textContent = `${data.tokens >= 1000 ? `${(data.tokens / 1000).toFixed(1)}k` : data.tokens} tokens`;
    send.innerHTML = data?.working ? icon("square", { size: 12 }) : icon("arrow-up", { size: 16 });
    send.setAttribute("aria-label", data?.working ? "Stop" : "Send");
    send.title = data?.working ? "Stop Antigravity's answer" : "Send";

    const nearBottom = feed.scrollHeight - feed.scrollTop - feed.clientHeight < 80;
    // A refusal agy moved past is already a failed step; keeping it would split one turn's work.
    const all = data?.items ?? [];
    const items = all.filter((it, i) => it.kind !== "permission" || (i === all.length - 1 && !data.working));
    const blocks = groupStream(items);
    const html = blocks
      .map((b, i) => {
        if (b.kind === "you") {
          // The opening brief is long and Verb's own words: fold it.
          const text = b.item.text ?? "";
          return i === 0 && text.startsWith("Work on spec")
            ? `<details class="stream-brief"><summary>Verb gave Antigravity the spec brief</summary>${renderProse(text, escapeHtml)}</details>`
            : `<div class="stream-you">${renderProse(text, escapeHtml)}</div>`;
        }
        if (b.kind === "agent") {
          const claim = claimedCriterion(b.item.text ?? "");
          const criterion = claim && spec && spec.id === data.spec_id ? spec.criteria?.[claim.number - 1] : null;
          const suggest =
            criterion && !criterion.done && !dismissed.has(claim.number)
              ? `<div class="stream-suggest" data-claim="${claim.number}" data-sentence="${escapeHtml(claim.sentence)}">${icon("check", { size: 16 })}<span><b>Antigravity says criterion ${claim.number} is met.</b> ${escapeHtml(criterion.text)}</span><button type="button" class="secondary-button small" data-prove>Mark proven</button><button type="button" class="text-button" data-dismiss-claim>Not yet</button></div>`
              : "";
          return `<div class="stream-agent"><span class="stream-avatar" aria-hidden="true">AG</span><div class="stream-body"><div class="stream-who"><b>Antigravity</b></div>${renderProse(b.item.text ?? "", escapeHtml)}</div></div>${suggest}`;
        }
        if (b.kind === "permission") {
          // Offered only when agy's turn ended on it; a refusal it moved past is just a failed step.
          if (data.working || i !== blocks.length - 1) return "";
          const words = stepWords(b.item);
          return `<div class="talk-permission">${icon("terminal", { size: 16 })}<span><b>Antigravity wants to ${escapeHtml(words.verb === "Ran" ? "run" : words.verb.toLowerCase())}</b>${words.target ? ` <code>${escapeHtml(words.target)}</code>` : ""}. It can't ask you from here.</span><button type="button" class="primary-button small" data-talk-terminal>Continue in terminal to approve</button></div>`;
        }
        if (b.kind === "note") return `<p class="stream-empty talk-note">${escapeHtml(b.item.text ?? "")}</p>`;
        if (b.kind === "work") {
          const failed = b.steps.filter((s) => s.failed).length;
          const open = data.working && i === blocks.length - 1;
          return `<details class="stream-work"${open ? " open" : ""}><summary>${icon("chevron-right", { size: 14 })}<span>${escapeHtml(workSummary(b))}</span>${failed ? `<span class="work-failed">${failed} failed</span>` : ""}</summary><ol>${b.steps
            .map((s) => {
              const w = stepWords(s);
              return `<li class="${s.failed ? "failed" : ""}">${escapeHtml(w.verb)}${w.target ? ` <code>${escapeHtml(w.target)}</code>` : ""}${s.failed ? " · failed" : ""}</li>`;
            })
            .join("")}</ol></details>`;
        }
        return "";
      })
      .join("");
    const thinking = data?.working ? '<p class="talk-thinking"><span></span>Antigravity is working…</p>' : "";
    feed.innerHTML = html || thinking ? `<div class="stream-column">${html}${thinking}</div>` : `<p class="stream-empty">Ask Antigravity about this spec.</p>`;
    if (nearBottom) feed.scrollTop = feed.scrollHeight;
  };

  async function refresh() {
    clearTimeout(timer);
    if (closed) return;
    try {
      const next = await deps.api("GET", `/api/talk/${encodeURIComponent(id)}`);
      const json = JSON.stringify(next);
      if (json !== lastJson) {
        lastJson = json;
        const toneChanged = talkTone(next) !== talkTone(data);
        data = next;
        render();
        if (toneChanged) deps.onChange?.();
      }
    } catch (error) {
      if (/no such talk/.test(error.message)) {
        closed = true;
        deps.remove?.(id);
        return;
      }
    }
    timer = setTimeout(refresh, data?.working ? 700 : document.hidden ? 10000 : 3000);
  }

  const submit = async () => {
    if (data?.working) {
      await deps.api("POST", `/api/talk/${encodeURIComponent(id)}/stop`).catch(() => {});
      refresh();
      return;
    }
    const text = input.value.trim();
    if (!text) return;
    try {
      await deps.api("POST", `/api/talk/${encodeURIComponent(id)}/message`, { text });
      input.value = "";
      input.style.height = "";
      refresh();
    } catch (error) {
      deps.toast(error.message, "error");
    }
  };
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    submit();
  });
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter" && !event.shiftKey && !event.isComposing) {
      event.preventDefault();
      if (!data?.working) submit();
    }
  });
  input.addEventListener("input", () => {
    input.style.height = "";
    input.style.height = `${Math.min(input.scrollHeight, 160)}px`;
  });

  tile.addEventListener("click", async (event) => {
    if (event.target.closest("[data-talk-terminal]")) {
      try {
        const { sessionId } = await deps.api("POST", `/api/talk/${encodeURIComponent(id)}/terminal`);
        deps.openTerminal(sessionId);
        deps.toast("Antigravity's terminal continues this conversation. Approve its actions there.");
      } catch (error) {
        deps.toast(error.message, "error");
      }
      return;
    }
    if (event.target.closest("[data-talk-end]")) {
      await deps.api("DELETE", `/api/talk/${encodeURIComponent(id)}`).catch(() => {});
      closed = true;
      deps.remove?.(id);
      return;
    }
    const card = event.target.closest(".stream-suggest");
    if (card && event.target.closest("[data-dismiss-claim]")) {
      dismissed.add(Number(card.dataset.claim));
      card.remove();
    } else if (card && event.target.closest("[data-prove]")) {
      deps.workbench?.()?.suggestProof(Number(card.dataset.claim) - 1, `Antigravity said: "${card.dataset.sentence}"`);
    }
  });

  refresh();
  return {
    id,
    tile,
    data: () => data,
    refresh,
    dispose() {
      closed = true;
      clearTimeout(timer);
    },
  };
}
