import { test } from "node:test";
import assert from "node:assert/strict";
import { Predictor, predictable } from "../src/typeahead.js";

test("only single printable characters are drawn ahead", () => {
  assert.ok(predictable("a"));
  assert.ok(predictable(" "));
  assert.ok(!predictable("\r"));
  assert.ok(!predictable("\x7f"));
  assert.ok(!predictable("\x1b[A"));
  assert.ok(!predictable("pasted text"));
});

test("prediction turns on only when the round trip is slow", () => {
  const p = new Predictor();
  assert.ok(!p.active(0), "no measurement yet");
  p.sent(0);
  p.received(5);
  assert.ok(!p.active(10), "a fast link needs no prediction");
  const slow = new Predictor();
  slow.sent(0);
  slow.received(250);
  assert.ok(slow.active(300));
  assert.equal(slow.patience(), 1000);
});

test("the real echo confirms guesses; misses pause prediction", () => {
  const p = new Predictor();
  p.sent(0);
  p.received(250);
  p.pending = "abc";
  assert.equal(p.confirm("ab "), 2);
  assert.equal(p.pending, "c");
  assert.equal(p.confirm("x"), 0, "unrelated output does not confirm");
  p.miss(1000);
  assert.ok(p.active(1000), "one miss is forgiven");
  p.pending = "z";
  p.miss(2000);
  assert.ok(!p.active(2000), "two misses pause it");
  assert.ok(p.active(2000 + 60_001), "for a minute");
});
