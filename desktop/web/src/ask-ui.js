// Ask Verb: questions answered from evidence (POST /api/ask), with clickable sources.

import { icon } from "./icons.js";

const STARTERS = ["Where are we?", "What's left?", "What changed today?", "Who is working on what?", "What is this project?"];

export function initAsk(deps) {
  const { api, toast, showDialog, escapeHtml, $ } = deps;
  const thread = [];

  function renderSuggestions(list) {
    $("#ask-suggestions").innerHTML = list
      .map((q) => `<button type="button" class="chip-button" data-ask="${escapeHtml(q)}">${escapeHtml(q)}</button>`)
      .join("");
  }

  function sourceButton(s) {
    const name = { spec: "file-check", file: "file-text", commit: "commit", session: "terminal", audit: "history" }[s.kind] ?? "file";
    return `<button type="button" class="source-chip" data-source-kind="${escapeHtml(s.kind)}" data-source-target="${escapeHtml(s.target)}" title="${escapeHtml(s.kind === "commit" ? "Copy commit id" : `Open ${s.label}`)}">${icon(name, { size: 13 })}${escapeHtml(s.label)}</button>`;
  }

  function render() {
    $("#ask-thread").innerHTML = thread
      .map(
        ({ question, answer, error }) => `<article class="ask-turn">
          <div class="ask-q">${escapeHtml(question)}</div>
          ${
            error
              ? `<div class="ask-a error">${escapeHtml(error)}</div>`
              : `<div class="ask-a">
                  <p class="ask-summary">${escapeHtml(answer.summary)}</p>
                  ${answer.points.length ? `<ul class="ask-points">${answer.points.map((p) => `<li>${escapeHtml(p)}</li>`).join("")}</ul>` : ""}
                  ${answer.sources.length ? `<div class="ask-sources"><span>Sources</span>${answer.sources.map(sourceButton).join("")}</div>` : ""}
                </div>`
          }
        </article>`,
      )
      .join("");
    const last = thread.at(-1);
    renderSuggestions(last?.answer?.suggestions?.length ? last.answer.suggestions : STARTERS);
    $("#ask-thread").lastElementChild?.scrollIntoView({ block: "nearest" });
  }

  async function ask(question) {
    const q = question.trim();
    if (!q) return;
    open();
    $("#ask-input").value = "";
    const turn = { question: q, answer: null };
    thread.push(turn);
    if (thread.length > 6) thread.shift();
    try {
      turn.answer = await api("POST", "/api/ask", { question: q });
    } catch (error) {
      turn.error = error.message;
    }
    render();
    $("#ask-input").focus();
  }

  function open() {
    if (!$("#ask-dialog").open) showDialog("ask-dialog");
    if (!thread.length) render();
    $("#ask-input").focus();
  }

  async function followSource(kind, target) {
    if (kind === "commit") {
      try {
        await navigator.clipboard.writeText(target);
        toast(`Copied commit ${target}`);
      } catch {
        toast(target);
      }
      return;
    }
    $("#ask-dialog").close();
    if (kind === "spec") deps.extras.selectSpec?.(target);
    else if (kind === "file") deps.extras.openFile?.(target);
    else if (kind === "session") deps.showView("sessions");
  }

  $("#ask-form").addEventListener("submit", (event) => {
    event.preventDefault();
    ask($("#ask-input").value);
  });
  document.addEventListener("click", (event) => {
    const suggestion = event.target.closest("[data-ask]");
    if (suggestion) return ask(suggestion.dataset.ask);
    const source = event.target.closest("[data-source-kind]");
    if (source) return followSource(source.dataset.sourceKind, source.dataset.sourceTarget);
    if (event.target.closest('[data-action="ask"]')) open();
  });
  document.addEventListener("keydown", (event) => {
    if (event.altKey && !event.metaKey && !event.ctrlKey && event.code === "KeyA" && !document.querySelector("dialog[open]")) {
      event.preventDefault();
      open();
    }
  });

  return {
    ask,
    open,
    /** A palette entry that asks whatever was typed. */
    commands(query) {
      const q = query.trim();
      return q.length >= 3 ? [{ title: `Ask Verb: ${q}`, hint: "Alt+A", run: () => ask(q), ask: true }] : [];
    },
  };
}
