import { STAGES } from "./workbench.js";

const title = value => value[0].toUpperCase() + value.slice(1);
const time = value => Date.parse(value) || 0;

export function specSignals(spec, summary) {
  const signals = [];
  for (const proof of summary?.sdlc?.criteria ?? []) {
    const check = proof.observedCheck;
    if (check?.result === "failed" && check.freshness === "current")
      signals.push({ label: "Failed check", detail: check.label, criterion: proof.index });
    if (proof.recorded && proof.freshness === "stale")
      signals.push({ label: "Stale proof", detail: proof.label, criterion: proof.index });
  }
  const stage = (spec.audit ?? []).findLast(entry => entry.action.startsWith("stage "));
  if (stage?.action.includes(" (warning: "))
    signals.push({ label: "Recorded gate warning", detail: stage.action });
  return signals.map(signal => ({ ...signal, specId: spec.id, specTitle: spec.title, workItemId: spec.work_item }));
}

export function boardRows(items, specs, signals, summaries) {
  const groups = new Map(items.map(item => [item.id, []]));
  const unlinked = [];
  const needs = new Set(signals.map(signal => signal.workItemId).filter(Boolean));
  for (const spec of specs) (groups.get(spec.work_item) ?? unlinked).push(spec);
  const rows = items.map(item => {
    const linked = groups.get(item.id);
    const counts = Object.fromEntries(STAGES.map(stage => [stage, 0]));
    let latest = 0;
    for (const spec of linked) {
      if (Object.hasOwn(counts, spec.stage)) counts[spec.stage]++;
      latest = Math.max(latest, time(summaries(spec)?.sdlc?.activityAt));
      for (const entry of spec.audit ?? []) latest = Math.max(latest, time(entry.at));
    }
    return { item, linked, counts, latest, needs: needs.has(item.id),
      state: linked.length && counts.ship === linked.length ? "Complete"
        : counts.plan + counts.build + counts.verify + counts.review ? "In progress" : item.draft ? "Draft" : "Not started" };
  }).sort((a, b) => Number(b.needs) - Number(a.needs) || b.latest - a.latest || a.item.id.localeCompare(b.item.id));
  return { rows, unlinked };
}

export function boardMarkup({ data, specs, signals, summaries, workTitle, showArchived, error, owner, esc }) {
  if (error) return `<p role="alert">${esc(error)}</p><button class="secondary-button" type="button" data-bench-retry>Retry</button>`;
  if (!data) return '<p role="status">Loading work items…</p>';
  const { rows, unlinked } = boardRows(data.items, specs, signals, summaries);
  const specRow = spec => `<li><button class="text-button" type="button" data-bench-spec="${esc(spec.id)}">${esc(spec.id)} · ${esc(spec.title)}</button>
    <select data-bench-stage="${esc(spec.id)}" aria-label="Stage for spec ${esc(spec.id)}"${owner ? "" : " disabled"}>
    ${STAGES.map(stage => `<option value="${stage}"${stage === spec.stage ? " selected" : ""}>${title(stage)}</option>`).join("")}</select></li>`;
  return rows.filter(({ item }) => showArchived || !data.archived?.[item.id]?.archived)
    .map(({ item, linked, counts, state, needs }) => {
      const archived = data.archived?.[item.id]?.archived;
      return `<section class="bench-board-row" data-board-item="${esc(item.id)}">
        <header><h2><button class="text-button" type="button" data-bench-work="${esc(item.id)}">${esc(workTitle(item))}</button></h2>
          ${owner ? `<button class="text-button" type="button" data-bench-archive="${esc(item.id)}" data-archive-value="${!archived}">${archived ? "Unarchive" : "Archive"}</button>` : ""}</header>
        <p>${archived ? "Archived" : state}${needs ? " · Needs you" : ""}</p>
        <dl class="bench-stage-counts">${STAGES.map(stage => `<div><dt>${title(stage)}</dt><dd>${counts[stage]}</dd></div>`).join("")}</dl>
        <ul>${linked.map(specRow).join("")}</ul>
      </section>`;
    }).join("") + `<details class="bench-board-row" data-board-unlinked><summary>Unlinked (${unlinked.length})</summary><ul>${unlinked.map(specRow).join("")}</ul></details>`;
}
