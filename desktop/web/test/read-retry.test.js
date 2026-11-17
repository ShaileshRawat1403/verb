import test from "node:test";
import assert from "node:assert/strict";
import { BUSY_DELAYS, FIRST_PAINT, READ_SLOTS, createReadQueue, forgetRemembered, readRank, readRemembered, readWithBusyRetry, remember, remembered } from "../src/read-retry.js";
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
const store = () => { const m = new Map(); return { getItem: k => m.get(k) ?? null, setItem: (k, v) => m.set(k, String(v)), removeItem: k => m.delete(k), key: i => [...m.keys()][i], get length() { return m.size; } }; };
test("a reload shows the last answer first, then the fresh one replaces it", async () => {
  const storage = store();
  const seen = [];
  const first = await readRemembered(async () => ({ n: 1 }), "/api/specs", v => seen.push(["prior", v]), { storage, queue: direct });
  assert.deepEqual(first, { n: 1 });
  assert.deepEqual(seen, [], "nothing remembered on the first load");
  const second = await readRemembered(async () => ({ n: 2 }), "/api/specs", v => seen.push(["prior", v]), { storage, queue: direct });
  assert.deepEqual(seen, [["prior", { n: 1 }]]);
  assert.deepEqual(second, { n: 2 });
  assert.deepEqual(remembered("/api/specs", storage), { n: 2 });
});
test("a failed read keeps the last good answer; a sign-out forgets every answer", async () => {
  const storage = store();
  remember("/api/git", { branch: "main" }, storage);
  storage.setItem("verb-web-token", "kept");
  await assert.rejects(readRemembered(async () => { throw new Error("down"); }, "/api/git", () => {}, { storage, queue: direct }), /down/);
  assert.deepEqual(remembered("/api/git", storage), { branch: "main" });
  forgetRemembered(storage);
  assert.equal(remembered("/api/git", storage), undefined);
  assert.equal(storage.getItem("verb-web-token"), "kept", "only remembered reads are forgotten");
});
test("an oversized or unreadable answer is not remembered", () => {
  const storage = store();
  remember("/api/files", { big: "x".repeat(600 * 1024) }, storage);
  assert.equal(remembered("/api/files", storage), undefined);
  storage.setItem("verb.read.v1:/api/hub", "{not json");
  assert.equal(remembered("/api/hub", storage), undefined);
});
test("on a one-read boundary, a first load starts one read at a time in first-paint order", async () => {
  const queue = createReadQueue(4);
  queue.setSlots(1);
  const started = [];
  let inFlight = 0, peak = 0;
  const api = async (_method, path) => {
    started.push(path);
    inFlight++; peak = Math.max(peak, inFlight);
    await new Promise(resolve => setTimeout(resolve, 2));
    inFlight--;
    return path;
  };
  // Asked in the order the page's modules happen to start, not the order the page needs.
  const asked = ["/api/files", "/api/talk", "/api/git", "/api/specs", "/api/work-items", "/api/hub", "/api/state"];
  await Promise.all(asked.map(path => readWithBusyRetry(api, path, { queue })));
  assert.deepEqual(started, [...FIRST_PAINT, "/api/talk"]);
  assert.equal(peak, 1);
});
test("a read asked later still waits behind lower-ranked reads, and slots never exceed the page's own", async () => {
  const queue = createReadQueue(4);
  queue.setSlots(1);
  const started = [];
  let release;
  const gate = new Promise(resolve => { release = resolve; });
  const api = async (_method, path) => { started.push(path); if (path === "/api/state") await gate; return path; };
  const first = readWithBusyRetry(api, "/api/state", { queue });
  await new Promise(resolve => setTimeout(resolve, 5));
  const later = ["/api/files", "/api/hub"].map(path => readWithBusyRetry(api, path, { queue }));
  await new Promise(resolve => setTimeout(resolve, 5));
  assert.deepEqual(started, ["/api/state"], "one at a time");
  release();
  await Promise.all([first, ...later]);
  assert.deepEqual(started, ["/api/state", "/api/hub", "/api/files"]);
  queue.setSlots(64);
  assert.equal(queue.slots(), READ_SLOTS);
  queue.setSlots(undefined);
  assert.equal(queue.slots(), READ_SLOTS);
  assert.equal(readRank("/api/files/preview?path=a"), FIRST_PAINT.length);
  assert.equal(readRank("/api/git?x=1"), FIRST_PAINT.indexOf("/api/git"));
});
