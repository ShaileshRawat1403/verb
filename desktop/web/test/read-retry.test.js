import test from "node:test";
import assert from "node:assert/strict";
import { BUSY_DELAYS, createReadQueue, readWithBusyRetry } from "../src/read-retry.js";
const busy = () => Object.assign(new Error("desktop host is busy; retry shortly"), { status: 503 });
const direct = task => task();
test("only rejected busy GETs retry, preserving the successful value", async () => {
  const calls = [], delays = [];
  const result = await readWithBusyRetry(async (...args) => {
    calls.push(args); if (calls.length < 3) throw busy(); return { files: ["a"] };
  }, "/api/files", { wait: async ms => delays.push(ms), queue: direct });
  assert.deepEqual(result, { files: ["a"] });
  assert.deepEqual(calls, Array.from({ length: 3 }, () => ["GET", "/api/files"]));
  assert.equal(delays.length, 2);
});
test("persistent host congestion backs off for seconds, not a second, before giving up", async () => {
  let calls = 0;
  const delays = [];
  await assert.rejects(readWithBusyRetry(async () => { calls++; throw busy(); }, "/api/files", { wait: async ms => delays.push(ms), queue: direct }), /host is busy/);
  assert.equal(calls, BUSY_DELAYS.length + 1);
  assert.ok(delays.every((ms, i) => i === 0 || ms >= delays[i - 1] - 100), "delays grow");
  assert.ok(delays.reduce((a, b) => a + b, 0) >= 15000, "waits long enough for slow reads to finish");
});
test("authorization, network errors and other server failures are not replayed", async () => {
  for (const error of [new Error("network failure"), Object.assign(new Error("denied"), { status: 403 }), Object.assign(new Error("other failure"), { status: 503 })]) {
    let calls = 0;
    await assert.rejects(readWithBusyRetry(async () => { calls++; throw error; }, "/api/files", { wait: async () => assert.fail("unexpected retry"), queue: direct }), error);
    assert.equal(calls, 1);
  }
});
test("the page never has more reads in flight than its slots, and every read completes", async () => {
  const queue = createReadQueue(4);
  let inFlight = 0, peak = 0;
  const api = async (_method, path) => {
    inFlight++; peak = Math.max(peak, inFlight);
    await new Promise(resolve => setTimeout(resolve, 5));
    inFlight--;
    return path;
  };
  const paths = Array.from({ length: 10 }, (_, i) => `/api/read-${i}`);
  const results = await Promise.all(paths.map(path => readWithBusyRetry(api, path, { queue })));
  assert.deepEqual(results, paths);
  assert.equal(peak, 4);
});
test("a failed read frees its slot", async () => {
  const queue = createReadQueue(1);
  await assert.rejects(queue(async () => { throw new Error("boom"); }), /boom/);
  assert.equal(await queue(async () => "next"), "next");
});
