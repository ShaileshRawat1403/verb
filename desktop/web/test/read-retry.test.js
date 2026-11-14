import test from "node:test";
import assert from "node:assert/strict";
import { readWithBusyRetry } from "../src/read-retry.js";
const busy = () => Object.assign(new Error("desktop host is busy; retry shortly"), { status: 503 });
test("only rejected busy GETs retry, preserving the successful value", async () => {
  const calls = [], delays = [];
  const result = await readWithBusyRetry(async (...args) => {
    calls.push(args); if (calls.length < 3) throw busy(); return { files: ["a"] };
  }, "/api/files", { wait: async ms => delays.push(ms) });
  assert.deepEqual(result, { files: ["a"] });
  assert.deepEqual(calls, Array.from({ length: 3 }, () => ["GET", "/api/files"]));
  assert.equal(delays.length, 2);
});
test("persistent host congestion stops after three reads", async () => {
  let calls = 0;
  await assert.rejects(readWithBusyRetry(async () => { calls++; throw busy(); }, "/api/files", { wait: async () => {} }), /host is busy/);
  assert.equal(calls, 3);
});
test("authorization, network errors and other server failures are not replayed", async () => {
  for (const error of [new Error("network failure"), Object.assign(new Error("denied"), { status: 403 }), Object.assign(new Error("other failure"), { status: 503 })]) {
    let calls = 0;
    await assert.rejects(readWithBusyRetry(async () => { calls++; throw error; }, "/api/files", { wait: async () => assert.fail("unexpected retry") }), error);
    assert.equal(calls, 1);
  }
});
