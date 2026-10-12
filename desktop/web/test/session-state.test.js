import { test } from "node:test";
import assert from "node:assert/strict";
import {
  isActiveSession,
  hasTerminalReplay,
  endedSession,
} from "../src/session-state.js";
test("replay, running and native recovery are independent", () => {
  const replay = {
    state: "recoverable",
    canResume: true,
    hasTerminal: true,
    terminal: { running: false, replayAvailable: true },
  };
  assert.equal(isActiveSession(replay), false);
  assert.equal(hasTerminalReplay(replay), true);
  assert.equal(
    isActiveSession({
      ...replay,
      terminal: { running: true, replayAvailable: true },
    }),
    true,
  );
  assert.equal(
    isActiveSession({
      state: "live",
      terminal: { running: false, replayAvailable: false },
    }),
    true,
  );
  assert.equal(
    hasTerminalReplay({ state: "recoverable", canResume: true }),
    false,
  );
  const ended = endedSession(replay);
  assert.equal(isActiveSession(ended), false);
  assert.equal(hasTerminalReplay(ended), false);
  assert.equal(ended.canResume, true);
});
test("legacy hosts and optimistic launches remain readable", () => {
  assert.equal(isActiveSession({ hostedHere: true }), true);
  assert.equal(hasTerminalReplay({ hasTerminal: true }), true);
  assert.equal(
    isActiveSession({ state: "ended", hostedHere: false, hasTerminal: true }),
    false,
  );
});
