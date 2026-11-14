import { test } from "node:test";
import assert from "node:assert/strict";
import { clampSize } from "../src/resize.js";

test("panel sizes stay within their bounds and are whole pixels", () => {
  assert.equal(clampSize(150, 200, 400), 200);
  assert.equal(clampSize(900, 200, 400), 400);
  assert.equal(clampSize(287.6, 200, 400), 288);
});
