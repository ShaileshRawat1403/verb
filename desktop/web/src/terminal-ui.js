// Keep stream and polling views consistent when a PTY finishes.
export function finishTerminalChrome(terminal, message) {
  terminal.ended = true;
  terminal.tile.classList.add("ended");
  const state = terminal.tile.querySelector(".terminal-state");
  state.textContent = message;
  state.dataset.tone = "";
  const close = terminal.tile.querySelector("[data-close-terminal]");
  close.textContent = "Close output";
  close.setAttribute("aria-label", "Close terminal output");
  close.title = "Close output. The session record stays in history.";
}
