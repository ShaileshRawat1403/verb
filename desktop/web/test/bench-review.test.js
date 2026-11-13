import assert from "node:assert/strict";
import test from "node:test";
import { reviewVerdict } from "../src/bench-review.js";

const proof = (accepted, result, freshness = "current", observedFreshness = freshness) => ({
  record: { accepted, freshness, observedCheck: result ? { result, freshness: observedFreshness } : null },
});

test("Review verdict waits for an observed run and reports current proof gaps", () => {
  assert.equal(reviewVerdict([proof(false), proof(true)]), "");
  assert.equal(reviewVerdict([proof(false, "failed"), proof(false)]), "Not ready: 1 failed check, 1 unproven");
  assert.equal(reviewVerdict([proof(true, "passed"), proof(true)]), "Ready to ship: all checks passed, all criteria proven");
  assert.equal(reviewVerdict([proof(true, "passed", "stale")]), "Not ready: 1 stale check");
});

test("Review cannot certify stale or unknown proof even if an acceptance flag survives", () => {
  assert.equal(reviewVerdict([proof(true, "passed", "stale", "current")]), "Not ready: 1 stale proof");
  assert.equal(reviewVerdict([proof(true, "passed", "unknown", "current")]), "Not ready: proof freshness unavailable");
  assert.equal(reviewVerdict([proof(true, "passed", "current", "unknown")]), "Not ready: proof freshness unavailable");
});
