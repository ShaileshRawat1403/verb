import test from "node:test";
import assert from "node:assert/strict";
import { findMatches, replaceMatches } from "../src/studio-find.js";
test("literal search, empty queries and case matching", () => {
  assert.deepEqual(findMatches("a.*b aZZb a.*b", "a.*b"), [
    { start: 0, end: 4 },
    { start: 10, end: 14 },
  ]);
  assert.deepEqual(findMatches("hello", ""), []);
  assert.deepEqual(findMatches("hello", "absent"), []);
  assert.equal(findMatches("Verb verb VERB", "verb").length, 3);
  assert.deepEqual(findMatches("Verb verb VERB", "verb", true), [
    { start: 5, end: 9 },
  ]);
});
test("multiline and UTF-16 offsets match textarea selections", () => {
  assert.deepEqual(findMatches("🙂 one\none", "one"), [
    { start: 3, end: 6 },
    { start: 7, end: 10 },
  ]);
  assert.deepEqual(findMatches("x\ny", "x\ny"), [{ start: 0, end: 3 }]);
});
test("replacements are literal and support deletion and individual matches", () => {
  const text = "cat Cat scatter",
    matches = findMatches(text, "cat");
  assert.equal(
    replaceMatches(text, matches, "$&\\$1"),
    "$&\\$1 $&\\$1 s$&\\$1ter",
  );
  assert.equal(replaceMatches(text, matches, ""), "  ster");
  assert.equal(replaceMatches(text, [matches[1]], "dog"), "cat dog scatter");
});
test("matches neither overlap nor recursively replace inserted text", () => {
  assert.equal(
    replaceMatches("aaaaa", findMatches("aaaaa", "aa"), "aaa"),
    "aaaaaaa",
  );
  assert.equal(replaceMatches("🙂🙂", findMatches("🙂🙂", "🙂"), "x"), "xx");
});

test("replacement amplification is rejected before constructing oversized output", () => {
  const text = "a".repeat(200 * 1024);
  assert.throws(
    () => replaceMatches(text, findMatches(text, "a"), "x".repeat(1000)),
    /200 KB/,
  );
  assert.throws(
    () => replaceMatches("a", [{ start: 0, end: 1 }], "🙂".repeat(60000)),
    /200 KB/,
  );
  assert.equal(
    replaceMatches(text, findMatches(text, "a"), "b").length,
    200 * 1024,
  );
});
