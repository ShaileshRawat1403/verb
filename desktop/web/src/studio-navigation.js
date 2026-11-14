// Navigation uses only the current textarea draft; it never reads or writes files.
export function initEditorNavigation({
  $,
  editor,
  focusEditor = () => editor.focus({ preventScroll: true }),
  selectRange,
}) {
  const toggle = $("#studio-line-toggle"),
    panel = $("#studio-line-panel"),
    input = $("#studio-line-number"),
    note = $("#studio-line-note"),
    position = $("#studio-editor-position");
  let cachedText = null,
    starts = [0];
  function sync() {
    if (cachedText !== editor.value) {
      cachedText = editor.value;
      starts = [0];
      for (let i = 0; i < cachedText.length; i++)
        if (cachedText[i] === "\n") starts.push(i + 1);
    }
    // Binary search keeps cursor movement cheap in long files.
    const offset = editor.selectionStart;
    let low = 0,
      high = starts.length;
    while (low + 1 < high) {
      const mid = (low + high) >>> 1;
      if (starts[mid] <= offset) low = mid;
      else high = mid;
    }
    const column =
      Array.from(editor.value.slice(starts[low], offset)).length + 1;
    position.textContent = `Line ${low + 1}, column ${column} · ${starts.length} ${starts.length === 1 ? "line" : "lines"}`;
    return low + 1;
  }
  function close(focus = true) {
    panel.hidden = true;
    toggle.setAttribute("aria-expanded", "false");
    input.removeAttribute("aria-invalid");
    if (focus) focusEditor();
  }
  function show() {
    input.value = String(sync());
    panel.hidden = false;
    toggle.setAttribute("aria-expanded", "true");
    input.removeAttribute("aria-invalid");
    note.textContent = `Choose a line from 1 to ${starts.length}.`;
    input.focus();
    input.select();
  }
  function go() {
    sync();
    const value = input.value.trim(),
      line = Number(value);
    if (
      !/^[1-9]\d*$/.test(value) ||
      !Number.isSafeInteger(line) ||
      line > starts.length
    ) {
      input.setAttribute("aria-invalid", "true");
      note.textContent = `Enter a whole line number from 1 to ${starts.length}.`;
      input.focus();
      return;
    }
    close();
    const offset = starts[line - 1];
    if (selectRange) selectRange(offset, offset, true);
    else {
      editor.setSelectionRange(offset, offset);
      const height = parseFloat(getComputedStyle(editor).lineHeight) || 24;
      editor.scrollTop = Math.max(
        0,
        (line - 1) * height - editor.clientHeight / 2,
      );
      editor.scrollLeft = 0;
    }
    sync();
  }
  toggle.addEventListener("click", () => (panel.hidden ? show() : close()));
  $("#studio-line-go").addEventListener("click", go);
  $("#studio-line-close").addEventListener("click", () => close());
  panel.addEventListener("keydown", (event) => {
    if (event.isComposing) return;
    if (event.key === "Escape") {
      event.preventDefault();
      close();
    } else if (event.key === "Enter" && event.target === input) {
      event.preventDefault();
      go();
    }
  });
  editor.addEventListener("keydown", (event) => {
    if (
      !event.isComposing &&
      event.ctrlKey &&
      !event.metaKey &&
      !event.altKey &&
      !event.shiftKey &&
      event.key.toLowerCase() === "g"
    ) {
      event.preventDefault();
      show();
    }
  });
  for (const event of ["input", "select", "click", "keyup"])
    editor.addEventListener(event, sync);
  return { sync, reset: () => close(false) };
}
