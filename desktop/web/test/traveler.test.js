import test from "node:test";
import assert from "node:assert/strict";
import { createStamp, documentState, emptyStations, itemStations, stationsLabel, travelerStripMarkup } from "../src/traveler.js";

const escapeHtml = value => String(value).replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

test("an empty project has Why next and every other station not started", () => {
  const stations = emptyStations();
  assert.deepEqual(stations.map(s => [s.name, s.state]), [
    ["Why", "current"], ["What", "open"], ["How", "open"], ["Proof", "open"], ["Ship", "open"],
  ]);
  assert.equal(stationsLabel(stations), "Why in progress, What not started, How not started, Proof not started, Ship not started");
});

test("the strip names every station with a word, never colour alone", () => {
  const markup = travelerStripMarkup(emptyStations(), escapeHtml);
  assert.equal((markup.match(/<li /g) ?? []).length, 5);
  assert.match(markup, /aria-label="Traveler: Why in progress, What not started/);
  assert.match(markup, /data-state="current"[^>]*aria-hidden="true">Next</);
  assert.match(markup, />BRD</);
  assert.match(markup, />PRD</);
});

test("document states come from content, drafts and warnings the host reports", () => {
  assert.equal(documentState({ brd: null }, "brd").word, "Missing");
  assert.equal(documentState({ brd: null, brdDraft: {} }, "brd").state, "hold");
  assert.equal(documentState({ brd: "# Why", warnings: ["BRD has no goals"] }, "brd").word, "Warnings");
  assert.equal(documentState({ prd: "# What", warnings: ["BRD has no goals"] }, "prd").state, "done");
});

test("item stations only aggregate per-spec facts", () => {
  const item = { brd: "x", prd: "x", warnings: [] };
  const specs = [
    { id: "001", stage: "ship", criteria: [{ index: 0 }] },
    { id: "002", stage: "build", criteria: [{ index: 0 }] },
  ];
  const proofFor = spec => spec.id === "001" ? { criteria: [{ index: 0, accepted: true }] } : null;
  const states = Object.fromEntries(itemStations(item, specs, proofFor).map(s => [s.key, s.state]));
  assert.deepEqual(states, { why: "done", what: "done", how: "current", proof: "current", ship: "current" });
  // A ship stage never stands in for proof: gates warn and record, they do not block a move.
  const shipped = itemStations(item, [{ id: "003", stage: "ship", criteria: [{ index: 0 }] }], () => null);
  assert.equal(shipped.find(s => s.key === "proof").state, "current");
  assert.equal(itemStations({ brd: null, warnings: [] }, [], () => null).find(s => s.key === "how").state, "open");
});

test("a stamp presses only when it turns done after being seen", () => {
  const stamp = createStamp(escapeHtml);
  assert.doesNotMatch(stamp("why:a", "done", "Published"), /data-fresh/);
  assert.doesNotMatch(stamp("what:a", "open", "Missing"), /data-fresh/);
  assert.match(stamp("what:a", "done", "Published"), /data-fresh/);
  assert.doesNotMatch(stamp("what:a", "done", "Published"), /data-fresh/);
});
