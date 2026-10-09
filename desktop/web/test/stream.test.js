import { test } from "node:test";
import assert from "node:assert/strict";
import { groupStream, hasStream, hasStreamView, renderProse, stepWords, workSummary } from "../src/stream-ui.js";

const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

test("only agents with a readable log get a stream", () => {
  assert.ok(hasStream("claude"));
  assert.ok(hasStream("codex"));
  assert.ok(!hasStream("shell"));
  assert.ok(!hasStream("opencode"));
  assert.ok(!hasStream("agy"), "its log is not readable");
  assert.ok(hasStreamView("agy"), "but it gets the composer");
  assert.ok(!hasStreamView("shell"));
});

test("consecutive steps fold into one block of work", () => {
  const blocks = groupStream([
    { kind: "you", text: "fix it" },
    { kind: "step", tool: "Read", at: "2026-10-09T10:00:00Z" },
    { kind: "step", tool: "Bash", failed: true, at: "2026-10-09T10:03:00Z" },
    { kind: "agent", text: "done" },
  ]);
  assert.deepEqual(blocks.map((b) => b.kind), ["you", "work", "agent"]);
  assert.equal(blocks[1].steps.length, 2);
  assert.equal(workSummary(blocks[1]), "Worked for 3 min · 2 steps");
  assert.equal(workSummary({ steps: [{}], started: undefined, ended: undefined }), "Worked · 1 step");
});

test("steps read as plain verbs", () => {
  assert.deepEqual(stepWords({ tool: "Bash", target: "npm test" }), { verb: "Ran", target: "npm test" });
  assert.deepEqual(stepWords({ tool: "Edit", target: "a.ts" }), { verb: "Edited", target: "a.ts" });
  assert.deepEqual(stepWords({ tool: "mcp_thing" }), { verb: "mcp_thing", target: "" });
});

test("prose is escaped first; only code, bold and paragraphs are interpreted", () => {
  const html = renderProse("Use `npm test` and **then**\n\nship <script>x</script>\n```\na < b\n```", esc);
  assert.ok(html.includes("<code>npm test</code>"));
  assert.ok(html.includes("<b>then</b>"));
  assert.ok(html.includes("&lt;script&gt;"));
  assert.ok(!html.includes("<script>"));
  assert.ok(html.includes("<pre>a &lt; b</pre>"));
});

test("an agent's claim about a criterion is found with its sentence", async () => {
  const { claimedCriterion, compactTokens } = await import("../src/stream-ui.js");
  assert.deepEqual(claimedCriterion("Updated the test. Criterion 2 is met: the expiry test passes now."), {
    number: 2,
    sentence: "Criterion 2 is met: the expiry test passes now.",
  });
  assert.equal(claimedCriterion("criterion #1 now passes")?.number, 1);
  assert.equal(claimedCriterion("Next I will work on criterion 3."), null, "intent is not a claim");
  assert.equal(claimedCriterion("All tests pass."), null);
  assert.equal(compactTokens(4210), "4.2k");
  assert.equal(compactTokens(722195), "722k");
  assert.equal(compactTokens(800), "800");
});
