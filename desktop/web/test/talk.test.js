import { test } from "node:test";
import assert from "node:assert/strict";
import { pendingPermission, talkTone } from "../src/talk-ui.js";

test("a talk's tab shows working, waiting on a permission, or ready", () => {
  assert.equal(talkTone({ working: true, items: [] }), "running");
  assert.equal(talkTone({ working: false, items: [{ kind: "agent" }, { kind: "permission" }] }), "waiting");
  assert.equal(talkTone({ working: false, items: [{ kind: "permission" }, { kind: "agent" }] }), "ok");
  assert.equal(talkTone(null), "ok");
  assert.equal(pendingPermission({ items: [{ kind: "permission", target: "node x" }] }).target, "node x");
  assert.equal(pendingPermission({ items: [{ kind: "agent" }] }), null);
});
