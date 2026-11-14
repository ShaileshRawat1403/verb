import { test } from "node:test";
import assert from "node:assert/strict";
import { blockMeta, failureNote, formatDuration, parseShellMark, unescapeCommand } from "../src/blocks.js";

test("shell marks parse; anything else is ignored", () => {
  assert.deepEqual(parseShellMark("A"), { kind: "A" });
  assert.deepEqual(parseShellMark("C"), { kind: "C" });
  assert.deepEqual(parseShellMark("D;2"), { kind: "D", exit: 2 });
  assert.deepEqual(parseShellMark("D"), { kind: "D", exit: null });
  assert.deepEqual(parseShellMark("E;npm test"), { kind: "E", command: "npm test" });
  assert.equal(parseShellMark("P;Cwd=/x"), null);
  assert.equal(parseShellMark("AB"), null);
});

test("the integration scripts' escaping is undone", () => {
  assert.equal(unescapeCommand("echo a\\x3b echo b"), "echo a; echo b");
  assert.equal(unescapeCommand("printf 'x\\x0ay'"), "printf 'x\ny'");
  assert.equal(unescapeCommand("a\\\\b"), "a\\b");
});

test("durations and block words read naturally", () => {
  assert.equal(formatDuration(420), "0.4 s");
  assert.equal(formatDuration(12_300), "12 s");
  assert.equal(formatDuration(125_000), "2 min 5 s");
  assert.equal(formatDuration(120_000), "2 min");
  assert.equal(blockMeta(0, 4200), "4.2 s");
  assert.equal(blockMeta(1, 4200), "exit 1 · 4.2 s");
  assert.equal(blockMeta(null, 100), "0.1 s");
  assert.equal(blockMeta(0, 40), "", "instant commands carry no time");
  assert.equal(blockMeta(2, 40), "exit 2");
});

test("a hand-off note is facts only and keeps the last 30 lines", () => {
  const output = Array.from({ length: 40 }, (_, i) => `line ${i}`).join("\n");
  const note = failureNote("npm test", 1, output);
  assert.match(note, /^`npm test` failed with exit 1\./);
  assert.ok(note.includes("line 39"));
  assert.ok(!note.includes("line 9\n"));
  assert.ok(!note.endsWith("\n"), "never ends in a newline that would send it");
});
