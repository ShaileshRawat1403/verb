import { test } from "node:test";
import assert from "node:assert/strict";
import {
  isActiveSession,
  hasTerminalReplay,
  endedSession,
  sessionWithRuntime,
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

test("lightweight runtime refresh keeps stopped output and native recovery separate", () => {
  const previous = { state: "live", canResume: false, hasTerminal: true, hostedHere: true };
  const replay = sessionWithRuntime(previous, { running: false, replayAvailable: true });
  assert.equal(isActiveSession(replay), false);
  assert.equal(hasTerminalReplay(replay), true);
  assert.equal(replay.state, "ended");
  assert.equal(hasTerminalReplay(sessionWithRuntime(previous, undefined)), false);
  const recoverable = sessionWithRuntime({ ...previous, state: "recoverable", canResume: true },
    { running: false, replayAvailable: true });
  assert.equal(recoverable.state, "recoverable");
  assert.equal(recoverable.canResume, true);
});
