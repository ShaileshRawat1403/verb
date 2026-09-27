// Tests for the browser workbench's pure view helpers. Run: npm test (node --test, no dependencies).
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  checksHtml,
  escapeHtml,
  sessionAction,
  stateName,
} from "../src/view.js";

const base = {
  schemaVersion: 1,
  clear: true,
  repositoryStatus: "read",
  repository: [],
  runtimes: [],
  lastKnownGood: null,
};

test("escapeHtml neutralises every character that could open markup", () => {
  assert.equal(
    escapeHtml(`<img src=x onerror="alert('1')">&`),
    "&lt;img src=x onerror=&quot;alert(&#39;1&#39;)&quot;&gt;&amp;",
  );
  assert.equal(escapeHtml(null), "");
});

test("a clear report says so and lists satisfied runtimes", () => {
  const html = checksHtml({
    ...base,
    runtimes: [
      {
        runtime: "node",
        source: ".nvmrc",
        wants: "20",
        found: "20.11.1",
        probe: "found",
        verdict: "satisfied",
      },
    ],
  });
  assert.match(html, /Nothing observed calls for care\./);
  assert.match(html, /node 20\.11\.1/);
  assert.match(html, /data-tone="clear"/);
});

test("repository warnings show the fact and the safe next step", () => {
  const html = checksHtml({
    ...base,
    clear: false,
    repository: [
      {
        code: "operation-in-progress",
        level: "caution",
        fact: "A merge is in progress.",
        safeNext: "git merge --abort",
      },
    ],
  });
  assert.match(html, /A merge is in progress\./);
  assert.match(html, /safe next: git merge --abort/);
  assert.doesNotMatch(html, /Nothing observed/);
});

test("text from the repository is escaped, never rendered as markup", () => {
  const html = checksHtml({
    ...base,
    clear: false,
    runtimes: [
      {
        runtime: "node",
        source: ".nvmrc",
        wants: "<script>alert(1)</script>",
        found: "18.0.0",
        probe: "found",
        verdict: "mismatch",
      },
    ],
  });
  assert.doesNotMatch(html, /<script>/);
  assert.match(html, /&lt;script&gt;/);
  assert.match(html, /Verb&#39;s environment runs a different version/);
});

test("unknown and unavailable are never shown as fine or as absent", () => {
  const unavailable = checksHtml({
    ...base,
    repository: null,
    repositoryStatus: "unavailable",
  });
  assert.match(unavailable, /Git could not read this checkout here\./);
  const notRepo = checksHtml({
    ...base,
    repository: null,
    repositoryStatus: "notRepository",
  });
  assert.match(notRepo, /Not a Git repository\./);

  const unknownRuntime = checksHtml({
    ...base,
    runtimes: [
      {
        runtime: "node",
        source: ".nvmrc",
        wants: "lts/*",
        found: "20.0.0",
        probe: "found",
        verdict: "unknown",
      },
    ],
  });
  assert.match(unknownRuntime, /Verb cannot compare this requirement/);
});

test("an unknown file distance from last-known-good is said, not hidden", () => {
  const good = (distance) =>
    checksHtml({
      ...base,
      lastKnownGood: {
        mark: { head: "a".repeat(40), markedAt: "", uncommitted: 0 },
        distance: {
          identical: false,
          headMoved: false,
          commitsSince: 0,
          commitsDropped: 0,
          filesDiffer: null,
          markMissing: false,
          ...distance,
        },
      },
    });
  assert.match(good({}), /file changes unknown/);
  assert.match(
    good({ filesDiffer: 1, commitsSince: 2 }),
    /2 commits, 1 file differs/,
  );
  assert.match(good({ identical: true, filesDiffer: 0 }), /exactly as marked/);
  assert.match(good({ markMissing: true }), /the marked commit is gone/);
});

test("session labels name the action a row will take", () => {
  assert.equal(sessionAction({ hasTerminal: true }), "Open terminal");
  assert.equal(sessionAction({ canResume: true }), "Resume session");
  assert.equal(stateName("interrupted"), "Recovery unconfirmed");
  assert.equal(stateName("something-new"), "something-new");
});
