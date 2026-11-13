import assert from "node:assert/strict";
import test from "node:test";
import { documentDiff } from "../src/document-diff.js";

test("draft comparison preserves unchanged lines and excludes the managed audit", () => {
  const published = "Title\nOld problem\nScope\nOld outcome\n\n## Audit trail\n- owner published\n";
  const draft = "Title\nNew problem\nScope\nNew outcome";
  assert.equal(documentDiff(published, draft),
    "@@ Published line 2 · Draft line 2 @@\n- Old problem\n+ New problem\n  Scope\n- Old outcome\n+ New outcome");
  assert.equal(documentDiff("Same\n## Audit trail\n- published", "Same\n"),
    "No differences in document text.");
});
