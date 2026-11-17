import test from "node:test";
import assert from "node:assert/strict";
import { parseServerTiming, percentile, readVerdict, summarize } from "../journeys/stats.js";
import { accessHeaders } from "../journeys/access.js";

test("percentiles are nearest-rank over ten samples", () => {
  const ten = [10, 1, 9, 2, 8, 3, 7, 4, 6, 5];
  assert.deepEqual(summarize(ten), { median: 5, p75: 8, p90: 9 });
  assert.equal(percentile([42], 90), 42);
  assert.ok(Number.isNaN(percentile([], 50)));
});

test("a read fails on its p75 or p90, and is slow on its median", () => {
  assert.equal(readVerdict({ median: 400, p75: 600, p90: 900 }), "pass");
  assert.equal(readVerdict({ median: 1_200, p75: 1_500, p90: 2_000 }), "slow");
  assert.equal(readVerdict({ median: 900, p75: 2_600, p90: 3_000 }), "fail");
  assert.equal(readVerdict({ median: 900, p75: 1_000, p90: 4_100 }), "fail");
  assert.equal(readVerdict({ median: 1_000, p75: 2_500, p90: 4_000 }), "pass");
});

test("Server-Timing phases are read and a proxy's own entries are ignored", () => {
  const phases = parseServerTiming('queue;dur=12, boundary;dur=980;desc="n=1", work;dur=40, total;dur=1032, cfRequestDuration;dur=1100');
  assert.deepEqual(phases, {
    queue: { ms: 12 },
    boundary: { ms: 980, entries: 1 },
    work: { ms: 40 },
    total: { ms: 1032 },
  });
  assert.equal(parseServerTiming(null), null);
  assert.equal(parseServerTiming("cfRequestDuration;dur=5"), null);
});

test("Cloudflare Access headers are sent only when both parts are set", () => {
  assert.deepEqual(accessHeaders({}), {});
  assert.deepEqual(accessHeaders({ VERB_ACCESS_CLIENT_ID: "id" }), {});
  assert.deepEqual(accessHeaders({ VERB_ACCESS_CLIENT_ID: "id", VERB_ACCESS_CLIENT_SECRET: "secret" }), {
    "CF-Access-Client-Id": "id",
    "CF-Access-Client-Secret": "secret",
  });
});
