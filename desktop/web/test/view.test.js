// Tests for the browser workbench's pure view helpers. Run: npm test (node --test, no dependencies).
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  agentDisplayName,
  agentMark,
  checksHtml,
  escapeHtml,
  filterSessions,
  filterTasks,
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

test("session and task search find the next place to work", () => {
  const sessions = [
    { id: "abc123", agent: "Codex", state: "live", isolated: true },
    { id: "def456", agent: "OpenCode", state: "recoverable", isolated: false },
  ];
  assert.deepEqual(filterSessions(sessions, " READY TO RESUME "), [
    sessions[1],
  ]);
  assert.deepEqual(filterSessions(sessions, "abc123"), [sessions[0]]);
  assert.deepEqual(filterSessions(sessions, "isolated"), [sessions[0]]);
  const tasks = [
    {
      title: "Review handoff",
      brief: "Check the phone bridge",
      status: "needs review",
      owner: "abc123",
      needsHelp: false,
    },
    {
      title: "Fix colors",
      brief: "",
      status: "active",
      owner: null,
      needsHelp: true,
    },
  ];
  assert.deepEqual(
    filterTasks(tasks, "codex", () => "Codex abc123"),
    [tasks[0]],
  );
  assert.deepEqual(
    filterTasks(tasks, "help requested", () => ""),
    [tasks[1]],
  );
});

test("agentMark and agentDisplayName recognize Terminal, Claude, Codex, AGY, Gemini, and others", () => {
  assert.equal(agentMark("claude"), "CC");
  assert.equal(agentMark("codex"), "CX");
  assert.equal(agentMark("agy"), "AG");
  assert.equal(agentMark("antigravity"), "AG");
  assert.equal(agentMark("gemini"), "GM");
  assert.equal(agentMark("opencode"), "OC");
  assert.equal(agentMark("shell"), "&gt;_");
  assert.equal(agentMark("custom"), "&gt;_");

  assert.equal(agentDisplayName("shell"), "Terminal");
  assert.equal(agentDisplayName("claude"), "Claude Code");
  assert.equal(agentDisplayName("codex"), "Codex");
  assert.equal(agentDisplayName("agy"), "AGY");
  assert.equal(agentDisplayName("antigravity"), "AGY");
  assert.equal(agentDisplayName("gemini"), "Gemini");
  assert.equal(agentDisplayName("custom-tool"), "custom-tool");
});

test("pasted input is bounded by UTF-8 bytes without splitting Unicode characters", async () => {
  const { takeInputChunk } = await import("../src/view.js");
  const input = "🙂".repeat(3000) + "\npwd\n";
  const first = takeInputChunk(input);
  assert.equal(new TextEncoder().encode(first.chunk).length, 8192);
  assert.equal(first.chunk + first.rest, input);
  assert.equal(first.chunk.endsWith("🙂"), true);
  assert.equal(takeInputChunk("pwd\n").rest, "");
});

test("built terminal CSS contains xterm input and viewport styling", async () => {
  const { readFile } = await import("node:fs/promises");
  const css = await readFile(
    new URL("../dist/app.css", import.meta.url),
    "utf8",
  );
  assert.match(css, /\.xterm-helper-textarea/);
  assert.match(css, /\.xterm-viewport/);
});
