// Literal, non-overlapping matches use textarea UTF-16 offsets. No workspace reads.
export function findMatches(text, query, matchCase = false) {
  if (!query) return [];
  const escaped = query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const matches = [];
  for (const match of text.matchAll(
    new RegExp(escaped, matchCase ? "gu" : "giu"),
  ))
    matches.push({ start: match.index, end: match.index + match[0].length });
  return matches;
}

// Replacement is literal too: $&, $1 and backslashes are never substitutions.
export function replaceMatches(text, matches, replacement) {
  // Bound amplification before constructing output (e.g. 200,000 short matches).
  const limit = 200 * 1024;
  const length = matches.reduce(
    (size, match) => size + replacement.length - (match.end - match.start),
    text.length,
  );
  if (length > limit)
    throw new Error(
      "Replacement would exceed the 200 KB file limit. No changes made.",
    );
  let cursor = 0;
  const pieces = [];
  for (const match of matches) {
    pieces.push(text.slice(cursor, match.start), replacement);
    cursor = match.end;
  }
  pieces.push(text.slice(cursor));
  const result = pieces.join("");
  if (new TextEncoder().encode(result).length > limit)
    throw new Error(
      "Replacement would exceed the 200 KB file limit. No changes made.",
    );
  return result;
}

export function initEditorFind({ $, editor, getKey, onChange }) {
  const panel = $("#studio-find-panel"),
    toggle = $("#studio-find-toggle"),
    query = $("#studio-find-query"),
    replacement = $("#studio-find-replacement"),
    caseOption = $("#studio-find-case"),
    counter = $("#studio-find-count");
  let key = getKey(),
    matches = [],
    undo = null;
  let cached = null;
  const all = () => {
    const text = editor.value,
      term = query.value,
      matchCase = caseOption.checked;
    if (
      !cached ||
      cached.text !== text ||
      cached.term !== term ||
      cached.matchCase !== matchCase
    )
      cached = {
        text,
        term,
        matchCase,
        matches: findMatches(text, term, matchCase),
      };
    return cached.matches;
  };
  const selectedIndex = () =>
    matches.findIndex(
      (m) => m.start === editor.selectionStart && m.end === editor.selectionEnd,
    );
  function sync() {
    if (key !== getKey()) {
      key = getKey();
      undo = null;
    }
    if (undo && editor.value !== undo.after) undo = null;
    matches = all();
    const selected = selectedIndex();
    counter.textContent = !query.value
      ? "Enter text to find"
      : !matches.length
        ? "No matches"
        : selected < 0
          ? `${matches.length} matches`
          : `${selected + 1} of ${matches.length}`;
    for (const id of ["prev", "next", "replace", "replace-all"])
      $(`#studio-find-${id}`).disabled = matches.length === 0;
    $("#studio-find-undo").disabled = !undo;
  }
  function select(index, focus = false) {
    if (!matches.length) return;
    const match = matches[(index + matches.length) % matches.length];
    if (focus) editor.focus({ preventScroll: true });
    editor.setSelectionRange(match.start, match.end);
    // Scroll only inside the textarea; leave the surrounding Studio layout stable.
    const line = editor.value.slice(0, match.start).split("\n").length - 1;
    const height = parseFloat(getComputedStyle(editor).lineHeight) || 24;
    editor.scrollTop = Math.max(0, line * height - editor.clientHeight / 2);
    sync();
  }
  function navigate(direction, focus = false) {
    sync();
    const current = selectedIndex();
    if (current >= 0) return select(current + direction, focus);
    let index =
      direction > 0
        ? matches.findIndex((m) => m.start >= editor.selectionEnd)
        : matches.findLastIndex((m) => m.end <= editor.selectionStart);
    if (index < 0) index = direction > 0 ? 0 : matches.length - 1;
    select(index, focus);
  }
  function show() {
    panel.hidden = false;
    toggle.setAttribute("aria-expanded", "true");
    const selection = editor.value.slice(
      editor.selectionStart,
      editor.selectionEnd,
    );
    if (selection && selection.length <= 1000 && !selection.includes("\n"))
      query.value = selection;
    sync();
    if (matches.length && selectedIndex() < 0) navigate(1);
    query.focus();
    query.select();
  }
  function close() {
    panel.hidden = true;
    toggle.setAttribute("aria-expanded", "false");
    editor.focus({ preventScroll: true });
  }
  function replace(every) {
    sync();
    if (!matches.length) return;
    const chosen =
      selectedIndex() >= 0
        ? selectedIndex()
        : Math.max(
            0,
            matches.findIndex((m) => m.start >= editor.selectionStart),
          );
    const targets = every ? matches : [matches[chosen]];
    const before = editor.value;
    let after;
    try {
      after = replaceMatches(before, targets, replacement.value);
    } catch (error) {
      counter.textContent = error.message;
      return;
    }
    if (before === after) return;
    undo = {
      before,
      after,
      start: editor.selectionStart,
      end: editor.selectionEnd,
    };
    editor.value = after;
    const end = targets[0].start + replacement.value.length;
    editor.setSelectionRange(end, end);
    onChange();
    sync();
    counter.textContent = `Replaced ${targets.length} ${targets.length === 1 ? "match" : "matches"} · draft only`;
  }
  toggle.addEventListener("click", () => (panel.hidden ? show() : close()));
  $("#studio-find-close").addEventListener("click", close);
  $("#studio-find-prev").addEventListener("click", () => navigate(-1, true));
  $("#studio-find-next").addEventListener("click", () => navigate(1, true));
  $("#studio-find-replace").addEventListener("click", () => replace(false));
  $("#studio-find-replace-all").addEventListener("click", () => replace(true));
  $("#studio-find-undo").addEventListener("click", () => {
    sync();
    if (!undo) return;
    const previous = undo;
    undo = null;
    editor.value = previous.before;
    editor.setSelectionRange(previous.start, previous.end);
    onChange();
    sync();
    counter.textContent = "Replacement undone · draft only";
  });
  for (const field of [query, caseOption])
    field.addEventListener("input", () => {
      sync();
      if (matches.length) select(0);
    });
  panel.addEventListener("keydown", (event) => {
    if (event.isComposing) return;
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      close();
    } else if (event.key === "Enter" && event.target === query) {
      event.preventDefault();
      navigate(event.shiftKey ? -1 : 1);
    }
  });
  editor.addEventListener("keydown", (event) => {
    if (event.isComposing) return;
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "f") {
      event.preventDefault();
      event.stopPropagation();
      show();
    } else if (event.key === "Escape" && !panel.hidden) {
      event.preventDefault();
      event.stopPropagation();
      close();
    }
  });
  editor.addEventListener("select", () => {
    if (!panel.hidden) sync();
  });
  return {
    sync,
    resetUndo() {
      undo = null;
      sync();
    },
  };
}
