// The traveler: one model for the five stations a change passes through, shared by every screen that
// shows them (Plan, the work tree's mini traveler, the first-run strip). Station states come only from
// facts the host resolves; nothing here infers progress beyond those facts.
import { STAGES } from "./workbench.js";

export const STATIONS = [
  { key: "why", name: "Why", document: "BRD" },
  { key: "what", name: "What", document: "PRD" },
  { key: "how", name: "How", document: "Spec" },
  { key: "proof", name: "Proof" },
  { key: "ship", name: "Ship" },
];

// Plain words for each state, for screen readers and anywhere colour alone would carry meaning.
export const STATE_WORDS = { done: "done", current: "in progress", hold: "needs a decision", fail: "failed", open: "not started" };

// Stamps mark a station's state with a word. A station that becomes done while the screen is open
// presses its stamp in once; first renders and reduced motion stay still. Each screen keeps its own
// memory so a stamp only presses where the change was seen.
export function createStamp(escapeHtml) {
  const memory = new Map();
  return function stamp(key, state, word, attributes = "") {
    const before = memory.get(key);
    memory.set(key, state);
    const fresh = before !== undefined && before !== "done" && state === "done";
    return `<small class="bench-stamp" data-state="${state}"${fresh ? " data-fresh" : ""}${attributes}>${escapeHtml(word)}</small>`;
  };
}

// The state of a work item's BRD or PRD, as the host reports it: content, draft and warnings.
export function documentState(item, kind) {
  const content = item[kind];
  const draft = item[`${kind}Draft`];
  const warnings = (item.warnings ?? []).filter(warning => kind === "brd"
    ? /^(?:BRD|Small BRD|Duplicate BRD)\b/.test(warning)
    : /^(?:PRD|Small PRD|Requirement|Duplicate PRD)\b/.test(warning));
  const word = content == null ? draft ? "Draft" : "Missing" : warnings.length ? "Warnings" : "Published";
  return { content, draft, warnings, word, state: word === "Published" ? "done" : word === "Missing" ? "open" : "hold" };
}

// Whether every criterion of a spec has accepted proof, from the host's proof summary.
export function proofAccepted(spec, proof) {
  return Boolean(proof && !proof.error && spec.criteria.length
    && spec.criteria.every(c => (proof.criteria ?? []).find(row => row.index === c.index)?.accepted));
}

// The five states for one work item. Why and What are the document states. How, Proof and Ship only
// aggregate per-spec facts: a station is done when every linked spec has reached it and current when
// at least one has.
export function itemStations(item, specs, proofFor) {
  const known = specs.filter(Boolean);
  const after = stage => spec => STAGES.indexOf(spec.stage) >= STAGES.indexOf(stage);
  const accepted = spec => proofAccepted(spec, proofFor(spec));
  const across = (reached, started) => !known.length ? "open"
    : known.every(reached) ? "done" : known.some(started ?? reached) ? "current" : "open";
  const states = {
    why: documentState(item, "brd").state,
    what: documentState(item, "prd").state,
    how: across(after("verify"), () => true),
    proof: across(accepted, after("verify")),
    ship: across(after("ship")),
  };
  return STATIONS.map(station => ({ ...station, state: states[station.key] }));
}

// A project with no work items: nothing has passed any station, and Why is where every change starts.
export function emptyStations() {
  return STATIONS.map(station => ({ ...station, state: station.key === "why" ? "current" : "open" }));
}

export function stationsLabel(stations) {
  return stations.map(station => `${station.name} ${STATE_WORDS[station.state]}`).join(", ");
}

// Five thin segments, for the work tree.
export function miniTravelerMarkup(stations, escapeHtml) {
  return `<span class="bench-mini" role="img" aria-label="${escapeHtml(stationsLabel(stations))}">${stations.map(station => `<i data-state="${station.state}"></i>`).join("")}</span>`;
}

// The five stations as stamps in a row, for places with room to name them.
export function travelerStripMarkup(stations, escapeHtml) {
  const word = station => station.state === "current" ? "Next" : station.state === "open" ? "Open" : STATE_WORDS[station.state];
  return `<ol class="bench-strip" aria-label="${escapeHtml(`Traveler: ${stationsLabel(stations)}`)}">${stations.map(station =>
    `<li data-state="${station.state}"><span class="type-cond">${escapeHtml(station.name)}</span><span class="bench-strip-doc">${station.document ? escapeHtml(station.document) : ""}</span><small class="bench-stamp" data-state="${station.state}" aria-hidden="true">${escapeHtml(word(station))}</small></li>`).join("")}</ol>`;
}
