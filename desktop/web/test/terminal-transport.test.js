import test from "node:test";
import assert from "node:assert/strict";
import { createTerminalTransport, reconnectDelay, RECONNECT_DELAYS, SIGNED_OUT_CODE, STABLE_MS } from "../src/terminal-transport.js";

// A stand-in browser: sockets, timers, clock and console the test drives by hand.
function harness({ ticketStatus = 200 } = {}) {
  const sockets = [];
  const timers = [];
  const logs = [];
  let clock = 0;
  let signedOut = 0;
  class FakeSocket {
    static CONNECTING = 0;
    static OPEN = 1;
    constructor(url) {
      this.url = url;
      this.readyState = FakeSocket.CONNECTING;
      sockets.push(this);
    }
    open() {
      this.readyState = FakeSocket.OPEN;
      this.onopen();
    }
    drop(code = 1006, reason = "") {
      this.readyState = 3;
      this.onclose({ code, reason });
    }
  }
  const status = { value: ticketStatus };
  const transport = createTerminalTransport({
    token: "",
    onOpen() {},
    onClose() {},
    onControl() {},
    onOutput() {},
    onSignedOut: () => signedOut++,
    WebSocketImpl: FakeSocket,
    fetchImpl: async () => ({ status: status.value, ok: status.value === 200, json: async () => ({ ticket: "t" }) }),
    setTimer: (fn, ms) => timers.push({ fn, ms }) - 1,
    clearTimer: () => {},
    now: () => clock,
    random: () => 0,
    log: { warn: (...a) => logs.push(a.join(" ")), info() {}, error() {} },
    loc: { protocol: "http:", host: "verb.test" },
    doc: null,
  });
  return {
    transport, sockets, timers, logs, status,
    get signedOut() { return signedOut; },
    advance(ms) { clock += ms; },
    async fire() { const timer = timers.at(-1); await timer.fn(); },
  };
}

test("reconnect delays back off 1, 2, 4, 8, 16, 30 s with up to 20% jitter", () => {
  assert.deepEqual(RECONNECT_DELAYS, [1, 2, 4, 8, 16, 30]);
  assert.deepEqual([0, 1, 2, 3, 4, 5, 6, 7].map(n => reconnectDelay(n, () => 0)), [1000, 2000, 4000, 8000, 16000, 30000, 30000, 30000]);
  assert.equal(reconnectDelay(0, () => 0.999), 1200);
});

test("a dropped socket reconnects on the back-off sequence and logs only the first close", async () => {
  const h = harness();
  await h.transport.connectTerminalStream();
  h.sockets[0].open();
  for (let n = 0; n < 7; n++) {
    h.sockets.at(-1).drop(1006);
    await h.fire();
  }
  assert.deepEqual(h.timers.map(t => t.ms), [1000, 2000, 4000, 8000, 16000, 30000, 30000]);
  assert.equal(h.sockets.length, 8);
  assert.deepEqual(h.logs, ["Verb terminal stream closed: 1006"]);
  // A different close is logged once more.
  h.sockets.at(-1).drop(1011, "server error");
  assert.equal(h.logs.length, 2);
});

test("a stream that stayed open for a minute starts the back-off again", async () => {
  const h = harness();
  await h.transport.connectTerminalStream();
  h.sockets[0].drop();
  await h.fire();
  h.sockets[1].drop();
  await h.fire();
  h.sockets[2].open();
  h.advance(STABLE_MS);
  h.sockets[2].drop();
  assert.deepEqual(h.timers.map(t => t.ms), [1000, 2000, 1000]);
});

test("close code 4401 stops reconnecting and asks for a reload once", async () => {
  const h = harness();
  await h.transport.connectTerminalStream();
  h.sockets[0].open();
  h.sockets[0].drop(SIGNED_OUT_CODE, "sign-in expired");
  assert.equal(h.signedOut, 1);
  assert.equal(h.timers.length, 0, "no retry is scheduled");
  await h.transport.connectTerminalStream();
  assert.equal(h.sockets.length, 1, "a later connect does not open a stream");
  assert.equal(h.transport.signedOut, true);
});

for (const status of [401, 403]) {
  test(`a ticket answer of ${status} stops reconnecting`, async () => {
    const h = harness({ ticketStatus: status });
    await h.transport.connectTerminalStream();
    assert.equal(h.signedOut, 1);
    assert.equal(h.sockets.length, 0);
    assert.equal(h.timers.length, 0);
  });
}

test("other ticket failures back off instead of signing out", async () => {
  const h = harness({ ticketStatus: 503 });
  await h.transport.connectTerminalStream();
  await h.fire();
  assert.equal(h.signedOut, 0);
  assert.deepEqual(h.timers.map(t => t.ms), [1000, 2000]);
  assert.equal(h.logs.length, 1);
});
