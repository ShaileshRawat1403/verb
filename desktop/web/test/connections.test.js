import { test } from "node:test";
import assert from "node:assert/strict";
import {
  connectionResultText,
  validateConnectionNumbers,
} from "../src/connections-ui.js";
test("connector tool and resource text is readable and remains literal external content", () => {
  assert.equal(
    connectionResultText({
      content: [{ type: "text", text: "<script>external text</script>" }],
    }),
    "<script>external text</script>",
  );
  assert.equal(
    connectionResultText({
      contents: [{ uri: "memo://reference", text: "Reference text" }],
    }),
    "Reference text",
  );
});
test("structured and truncated connector data retain their meaning", () => {
  assert.match(
    connectionResultText({
      content: [{ type: "text", text: "Recorded" }],
      structuredContent: { revision: "abc" },
    }),
    /Recorded[\s\S]*"revision": "abc"/,
  );
  assert.equal(
    connectionResultText({ truncated: true, text: "Partial" }),
    "[Truncated service response]\nPartial",
  );
  assert.match(connectionResultText({ task: { taskId: "123" } }), /taskId/);
});

test("unsafe input integers are refused before a review request can silently round them", () => {
  assert.throws(
    () => validateConnectionNumbers(JSON.parse('{"id":9007199254740993}')),
    /precisely/,
  );
  assert.doesNotThrow(() =>
    validateConnectionNumbers({
      id: "9007199254740993",
      count: 42,
      nested: [1.5],
    }),
  );
});
