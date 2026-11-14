import { STAGES } from "./workbench.js";

const title = value => value[0].toUpperCase() + value.slice(1);
const time = value => Date.parse(value) || 0;

export function stageAudit(action) {
  const move = /^stage \S+ → (\w+)(.*)$/.exec(action);
  if (!move) return null;
  const [, stage, tail] = move;
  const structured = tail.match(/^ \(warnings: (\[.*?\])\)(?: — (.*))?$/);
  if (structured) {
    try {
      const warnings = JSON.parse(structured[1]);
      if (warnings.every(line => typeof line === "string")) return { stage, warnings, note: structured[2] };
    } catch { /* A manually edited line may be malformed. */ }
  }
  const base = tail.match(/^ \(warning: (.*?)\)(?: — |$)/)?.[1];
  const [legacyNote, extra] = tail.split(/; SDLC readiness warnings: /, 2);
  const warnings = [base, extra].filter(Boolean).flatMap(value =>
    value.replace("Git readiness could not be established; release status is unknown",
      "Git readiness could not be established. Release status is unknown.")
      .split(/;\s*/).map(line => line.trim()).filter(Boolean));
  const note = legacyNote.slice(legacyNote.indexOf(" — ") + 3).trim();
  return { stage, warnings, note: legacyNote.includes(" — ") && note ? note : undefined };
}

export function stageSummary(stage, count) {
  return `Moved to ${title(stage)}${count ? ` with ${count} warning${count === 1 ? "" : "s"}` : ""}`;
}

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
  const gate = stage && stageAudit(stage.action);
  if (gate?.warnings.length)
    signals.push({ label: "Recorded gate warning", detail: stageSummary(gate.stage, gate.warnings.length), warnings: gate.warnings });
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

export function boardMarkup({ data, specs, signals, summaries, workTitle, showArchived, error, owner, esc, documents = () => [] }) {
  if (error) return `<p role="alert">${esc(error)}</p><button class="secondary-button" type="button" data-bench-retry>Retry</button>`;
  if (!data) return '<p role="status">Loading work items…</p>';
  const { rows, unlinked } = boardRows(data.items, specs, signals, summaries);
  return rows.filter(({ item }) => showArchived || !data.archived?.[item.id]?.archived)
    .map(({ item, counts, state, needs, latest }) => {
      const archived = data.archived?.[item.id]?.archived;
      return `<section class="bench-board-row" data-board-item="${esc(item.id)}">
        <header><h2><button class="text-button" type="button" data-bench-work="${esc(item.id)}">${esc(workTitle(item))}</button></h2>
          ${owner ? `<button class="text-button" type="button" data-bench-archive="${esc(item.id)}" data-archive-value="${!archived}">${archived ? "Unarchive" : "Archive"}</button>` : ""}</header>
        <p class="bench-board-meta">${[item.size ? title(item.size) : "Size unavailable", archived ? "Archived" : state, needs ? "Needs you" : ""].filter(Boolean).join(" · ")}</p>
        <p class="bench-board-time">${latest ? `Last change <time datetime="${new Date(latest).toISOString()}">${esc(new Date(latest).toLocaleString())}</time>` : "No spec activity recorded"}</p>
        <div class="bench-board-track">
          ${documents(item).map(doc => `<span class="bench-board-doc" data-state="${doc.state}"><span>${esc(doc.name)}</span><small class="bench-stamp" data-state="${doc.state}">${esc(doc.word)}</small></span>`).join("")}
          <dl class="bench-stage-counts">${STAGES.map(stage => `<div data-count="${counts[stage]}"><dt>${title(stage)}</dt><dd>${counts[stage]}</dd></div>`).join("")}</dl>
        </div>
      </section>`;
    }).join("") + `<details class="bench-board-row" data-board-unlinked><summary>Unlinked (${unlinked.length})</summary><p>Open an unlinked spec from the Work sidebar.</p></details>`;
}
