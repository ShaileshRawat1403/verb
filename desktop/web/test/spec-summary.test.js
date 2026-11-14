import test from "node:test";
import assert from "node:assert/strict";
import { specPresentation, filterSpecs } from "../src/spec-summary.js";
import { specSessions } from "../src/workbench.js";
const spec = { id: "024", revision: "new", title: "Session recovery", branch: "spec/024", stage: "build" };
const summary = {
  revision: "new", observedAt: 1000,
  execution: { label: "1 connected session", sessions: [{ id: "session-1" }] },
  verification: { label: "2/4 criteria recorded complete", detail: "Independent results and freshness unknown." },
  delivery: { label: "Release not observed", detail: "No deployment observation.", checkoutChangedFiles: 3 },
  attention: [{ reason: "Help requested", sessionId: "session-1", source: "task ledger" }],
};
test("stale observations do not retain live requests or imply a release", () => {
  assert.equal(specPresentation({ ...spec, revision: undefined }, undefined).current, false);
  const current = specPresentation(spec, summary, {}, 1001);
  assert.equal(current.delivery, "Release not observed");
  assert.match(current.checkout, /attribution.*not established/);
  const expired = specPresentation(spec, summary, {}, 100_000);
  assert.equal(expired.execution, "Execution observations unavailable");
  assert.deepEqual(expired.attention, []);
  assert.equal(expired.checkout, "Checkout changes are not attributed to this spec");
  assert.equal(specPresentation(spec, { ...summary, revision: "old" }, {}, 1001).current, false);
  assert.equal(specPresentation(spec, { ...summary, observedAt: 1_000_000 }, {}, 1001).observed, false);
});
test("volatile waiting signals are associated, live and deduplicated", () => {
  const signals = { liveIds: new Set(["session-1"]), waitingTerminals: new Set(["session-1", "unrelated"]) };
  const view = specPresentation(spec, summary, signals, 1001);
  assert.equal(view.attention.length, 1);
  assert.equal(view.attention[0].source, "agent observer");
  assert.equal(specPresentation(spec, { ...summary, attention: [] }, { waitingTerminals: signals.waitingTerminals }, 1001).attention.length, 0);
});
test("search and attention filters compose without inventing work states", () => {
  const other = { ...spec, id: "025", title: "Colour contrast", stage: "review" };
  const views = new Map([[spec.id, { attention: [] }], [other.id, { attention: [{ reason: "Review requested" }] }]]);
  assert.deepEqual(filterSpecs([spec, other], views, "contrast review", true), [other]);
  assert.deepEqual(filterSpecs([spec, other], views, "recovery", true), []);
});
test("ambiguous session prefixes never select an arbitrary live session", () => {
  const recorded = { audit: [{ at: "now", action: "started codex on this spec (session abcdef12)" }] };
  const sessions = [{ id: "abcdef12-one", state: "live" }, { id: "abcdef12-two", state: "ended" }];
  assert.equal(specSessions(recorded, sessions)[0].live, false);
  assert.equal(specSessions(recorded, sessions)[0].id, null);
});

test("compact evidence cards preserve result and freshness boundaries", () => {
  const projection = { ...summary, sdlc: { accepted: 0, total: 3, verification: "failed", reviewState: "stale" } };
  assert.equal(specPresentation(spec, projection, {}, 1001).cardVerification, "0/3 accepted · failed");
  assert.equal(specPresentation(spec, projection, {}, 1001).cardReview, "Review stale");
  projection.sdlc.verification = "accepted"; projection.sdlc.accepted = 3;
  assert.equal(specPresentation(spec, projection, {}, 100_000).cardVerification, "3/3 accepted · freshness unconfirmed");
  assert.equal(specPresentation(spec, projection, {}, 100_000).cardReview, "Review observation unavailable");
  projection.sdlc = { error: "damaged" };
  assert.equal(specPresentation(spec, projection, {}, 1001).cardVerification, "Evidence unavailable");
});

test("stage selection composes with search and attention", () => {
  const other = { ...spec, id: "025", title: "Recovery review", stage: "review" };
  const views = new Map([[spec.id, { attention: [] }], [other.id, { attention: [{ reason: "Review requested" }] }]]);
  assert.deepEqual(filterSpecs([spec, other], views, "recovery", true, "review"), [other]);
  assert.deepEqual(filterSpecs([spec, other], views, "recovery", true, "build"), []);
  assert.deepEqual(filterSpecs([spec, other], views, "", false, "all"), [spec, other]);
});
