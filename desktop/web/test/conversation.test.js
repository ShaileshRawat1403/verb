import { test } from "node:test";
import assert from "node:assert/strict";
import {
  agentTime,
  composerBlock,
  deliveryLine,
  isStale,
  newMessageId,
  previewableTarget,
  readinessChip,
  reconcile,
  streamDevices,
} from "../src/conversation-model.js";

const view = (over = {}) => ({
  available: true,
  identity: { session: "s1", conversation: "c1", attempt: 1, attemptStartedAt: 1000 },
  stream: { enabled: true, generation: 3 },
  readiness: { state: "ready", reason: "Waiting for you.", basedOn: ["The agent reports its status as idle."] },
  canSend: true,
  sendContract: "terminal",
  deliveries: [],
  ...over,
});

test("a time the agent did not give is unavailable, never now", () => {
  assert.equal(agentTime(undefined), null);
  assert.equal(agentTime("yesterday"), null);
  assert.equal(agentTime("2026-10-27 10:00:00"), null);
  assert.ok(agentTime("2026-10-27T10:00:00.120Z"));
  const line = deliveryLine({ state: "received", receivedAt: "garbage" }, "Claude Code");
  assert.match(line.label, /time unavailable/);
});

test("the host is the authority for every message it lists", () => {
  const outbox = new Map([["m1", { id: "m1", text: "hi", state: "posting", attempt: "1:1000", createdAt: 1 }]]);
  reconcile(outbox, view({ deliveries: [{ id: "m1", state: "received", turnId: "u-1", receivedAt: "2026-10-27T10:00:00Z" }] }));
  assert.equal(outbox.get("m1").state, "received");
  assert.equal(outbox.get("m1").turnId, "u-1");
});

test("a missing receipt cannot prove a lost request was never typed", () => {
  const outbox = new Map([["m1", { id: "m1", text: "hi", state: "lost", attempt: "1:1000", createdAt: 1 }]]);
  reconcile(outbox, view());
  assert.equal(outbox.get("m1").state, "unknown");
  assert.match(outbox.get("m1").reason, /may still arrive/);
});

test("after a restart an unconfirmed message is unknown, and is never resent", () => {
  const outbox = new Map([["m1", { id: "m1", text: "hi", state: "sent", seenByHost: true, attempt: "1:1000", createdAt: 1 }]]);
  const shown = reconcile(outbox, view({ identity: { session: "s1", conversation: "c1", attempt: 2, attemptStartedAt: 5000 } }));
  assert.equal(outbox.get("m1").state, "unknown");
  assert.match(outbox.get("m1").reason, /restarted/);
  const line = deliveryLine(shown[0], "Claude Code");
  assert.deepEqual(line.actions, ["check", "resend", "discard", "terminal"]);
  assert.match(line.label, /may or may not have reached Claude Code/);
});

test("a message the host forgot within the same attempt is unknown, not failed", () => {
  const outbox = new Map([["m1", { id: "m1", text: "hi", state: "sent", seenByHost: true, attempt: "1:1000", createdAt: 1 }]]);
  reconcile(outbox, view());
  assert.equal(outbox.get("m1").state, "unknown");
});

test("an ended process leaves unconfirmed messages unknown", () => {
  const outbox = new Map([["m1", { id: "m1", text: "hi", state: "sent", seenByHost: true, attempt: "1:1000", createdAt: 1 }]]);
  reconcile(outbox, { available: false, reason: "ended" });
  assert.equal(outbox.get("m1").state, "unknown");
});

test("other devices' messages are shown only while they need attention", () => {
  const now = 10 * 60 * 1000 + 5;
  const shown = reconcile(new Map(), view({ deliveries: [
    { id: "p1", state: "sent", origin: "phone", createdAt: 1 },
    { id: "p2", state: "received", origin: "phone", createdAt: 2 },
    { id: "p3", state: "failed", origin: "phone", createdAt: 3 },
  ] }), now);
  assert.deepEqual(shown.map((m) => m.id), ["p1"]);
});

test("a response from before the stream changed is dropped", () => {
  assert.equal(isStale({ stream: { generation: 2 } }, 3), true);
  assert.equal(isStale({ stream: { generation: 3 } }, 3), false);
  assert.equal(isStale({ stream: { generation: 4 } }, 3), false);
  assert.equal(isStale({ stream: { generation: 1 } }, null), false);
});

test("stream-off lists each device and whether it confirmed clearing", () => {
  const rows = streamDevices(
    { generation: 5, clients: [
      { id: "me", label: "Chrome on macOS", ackedGeneration: 5, ackedAt: 1 },
      { id: "laptop", label: "Safari on macOS", ackedGeneration: 4, ackedAt: 1 },
    ] },
    { paired: true, connected: false, ackedGeneration: 3 },
    "me",
  );
  assert.deepEqual(rows.map((r) => [r.name, r.state]), [
    ["This browser", "cleared"],
    ["Safari on macOS", "not_confirmed"],
    ["Paired phone", "not_confirmed"],
  ]);
});

test("the composer fails closed and says why", () => {
  assert.equal(composerBlock(view()), null);
  assert.match(composerBlock(view(), { online: false }), /offline/);
  assert.match(composerBlock(view({ canSend: false, readiness: { state: "unknown", reason: "Verb can't tell whether the agent is ready." } })), /can't tell/);
  assert.match(composerBlock(view({ stream: { enabled: false, generation: 4 } })), /stream on/);
  assert.match(composerBlock(view({ deliveries: [{ id: "x", state: "unknown" }] })), /earlier message/);
  assert.match(composerBlock(view({ sendContract: "unsupported", sendNote: "Use the terminal." })), /terminal/);
  assert.ok(composerBlock(null));
});

test("screen-text signs read as signs, not facts", () => {
  const chip = readinessChip({ state: "approval", basedOn: ["Screen text looks like a permission prompt (a sign, not a fact)."] });
  assert.equal(chip.label, "Looks like a permission prompt");
  assert.match(chip.basedOn, /a sign, not a fact/);
});

test("only relative project files open in the preview pane", () => {
  assert.equal(previewableTarget({ tool: "Edit", target: "src/app.js" }), "src/app.js");
  assert.equal(previewableTarget({ tool: "Bash", target: "npm test" }), null);
  assert.equal(previewableTarget({ tool: "Read", target: "/etc/passwd" }), null);
  assert.equal(previewableTarget({ tool: "Edit", target: "../outside" }), null);
  assert.equal(previewableTarget({ tool: "Edit", target: "a.js, b.js" }), null);
});

test("message ids are random and shaped for the host", () => {
  const a = newMessageId();
  assert.match(a, /^m-[0-9a-f]{24}$/);
  assert.notEqual(a, newMessageId());
});


test("privacy clears every panel before accepting an off generation and fences late reads", async () => {
  const { createConversationPrivacy } = await import("../src/conversation-privacy.js");
  const privacy = createConversationPrivacy();
  const cleared = [];
  privacy.subscribe((s) => { if (!s.enabled) cleared.push("studio"); });
  privacy.subscribe((s) => { if (!s.enabled) cleared.push("hidden sessions"); });
  assert.equal(privacy.apply({ enabled: true, generation: 1 }), true);
  privacy.hide();
  assert.deepEqual(cleared, ["studio", "hidden sessions"]);
  assert.equal(privacy.apply({ enabled: true, generation: 1 }), false);
  assert.equal(privacy.apply({ enabled: false, generation: 2 }, { explicit: true }), true);
  assert.equal(privacy.apply({ enabled: true, generation: 1 }), false);
  assert.equal(privacy.apply({ enabled: true, generation: 3 }), true);
  assert.equal(privacy.blocked, false);
});
