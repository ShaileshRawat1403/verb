// Runtime attachment and native recovery are separate capabilities. Older hosts remain readable.
export function isActiveSession(session) {
  if (session.terminal)
    return (
      session.terminal.running === true ||
      (session.state === "live" && !session.terminal.replayAvailable)
    );
  return session.hostedHere === true || session.state === "live";
}
export function hasTerminalReplay(session) {
  return session.terminal?.replayAvailable ?? session.hasTerminal === true;
}
export function endedSession(session) {
  return {
    ...session,
    state: "ended",
    hostedHere: false,
    hasTerminal: false,
    terminal: { running: false, replayAvailable: false },
  };
}

// A stopped process can still own output; a lightweight workspace refresh must retain that fact.
export function sessionWithRuntime(session, terminal) {
  const facts = terminal || { running: false, replayAvailable: false };
  return {
    ...session,
    state: facts.running ? "live" : session.state === "live" ? "ended" : session.state,
    hostedHere: facts.running,
    hasTerminal: facts.replayAvailable,
    terminal: facts,
  };
}
