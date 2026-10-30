import { CODE_EDITOR_ENGINE } from "./studio-code-notices.js";
import { EditorState, Prec, Compartment } from "@codemirror/state";
import {
  EditorView,
  keymap,
  lineNumbers,
  highlightActiveLine,
  highlightActiveLineGutter,
  drawSelection,
  highlightSpecialChars,
} from "@codemirror/view";
import {
  history,
  historyKeymap,
  defaultKeymap,
  undo,
  redo,
  undoDepth,
  redoDepth,
  isolateHistory,
} from "@codemirror/commands";
import {
  bracketMatching,
  indentOnInput,
  foldGutter,
  foldKeymap,
  syntaxHighlighting,
  HighlightStyle,
} from "@codemirror/language";
import {
  autocompletion,
  completionKeymap,
  closeBrackets,
  closeBracketsKeymap,
} from "@codemirror/autocomplete";
import { linter, lintGutter } from "@codemirror/lint";
import { javascript } from "@codemirror/lang-javascript";
import { html } from "@codemirror/lang-html";
import { css } from "@codemirror/lang-css";
import { json, jsonParseLinter } from "@codemirror/lang-json";
import { python } from "@codemirror/lang-python";
import { tags } from "@lezer/highlight";

const highlights = HighlightStyle.define([
  {
    tag: [tags.keyword, tags.modifier, tags.operatorKeyword],
    color: "var(--code-keyword)",
  },
  { tag: [tags.string, tags.regexp], color: "var(--code-string)" },
  { tag: [tags.number, tags.bool, tags.null], color: "var(--code-number)" },
  { tag: [tags.comment], color: "var(--text-muted)", fontStyle: "italic" },
  {
    tag: [tags.tagName, tags.typeName, tags.className],
    color: "var(--code-tag)",
  },
  {
    tag: [tags.attributeName, tags.propertyName],
    color: "var(--code-property)",
  },
]);
export function codeLanguage(path = "") {
  if (/\.(tsx?|jsx?|mjs|cjs)$/i.test(path))
    return /\.tsx?$/i.test(path) ? "TypeScript" : "JavaScript";
  if (/\.html?$/i.test(path)) return "HTML";
  if (/\.css$/i.test(path)) return "CSS";
  if (/\.json$/i.test(path)) return "JSON";
  if (/\.py$/i.test(path)) return "Python";
  return "Plain text";
}
function language(path) {
  const name = codeLanguage(path);
  if (name === "JavaScript" || name === "TypeScript")
    return javascript({
      typescript: name === "TypeScript",
      jsx: /\.[jt]sx$/i.test(path),
    });
  if (name === "HTML") return html();
  if (name === "CSS") return css();
  if (name === "Python") return python();
  if (name === "JSON") return [json(), linter(jsonParseLinter()), lintGutter()];
  return [];
}
// The existing textarea/draft map remains the save boundary. No project code runs.
// CodeMirror states retain per-file cursor/history; they are evicted with clean drafts.
export function initStudioCode({ $, textarea }) {
  const parent = $("#studio-code-editor"),
    codeButton = $("#studio-code-mode"),
    plainButton = $("#studio-plain-mode"),
    tools = $("#studio-code-history"),
    note = $("#studio-code-note"),
    undoButton = $("#studio-code-undo"),
    redoButton = $("#studio-code-redo");
  parent.dataset.engine = CODE_EDITOR_ENGINE;
  const states = new Map();
  const wrapping = new Compartment();
  let wrapLines = matchMedia("(max-width: 640px)").matches;
  const wrapButton = $("#studio-code-wrap");
  wrapButton.setAttribute("aria-pressed", String(wrapLines));
  let key = null,
    syncing = false,
    plain = false,
    view;
  function mirror() {
    const selection = view.state.selection.main;
    textarea.setSelectionRange(selection.from, selection.to);
  }
  function historyStatus() {
    undoButton.disabled = !view || undoDepth(view.state) === 0;
    redoButton.disabled = !view || redoDepth(view.state) === 0;
  }
  function create(path, text) {
    return EditorState.create({
      doc: text,
      extensions: [
        wrapping.of(wrapLines ? EditorView.lineWrapping : []),
        lineNumbers(),
        highlightActiveLine(),
        highlightActiveLineGutter(),
        drawSelection(),
        highlightSpecialChars(),
        history(),
        indentOnInput(),
        bracketMatching(),
        foldGutter(),
        closeBrackets(),
        syntaxHighlighting(highlights),
        language(path),
        autocompletion(),
        // Tab retains normal focus navigation. Completion has Ctrl+Space and arrow keys.
        keymap.of([
          ...closeBracketsKeymap,
          ...defaultKeymap,
          ...historyKeymap,
          ...foldKeymap,
          ...completionKeymap,
        ]),
        EditorView.contentAttributes.of({
          "aria-label": "Code editor",
          "aria-describedby": "studio-code-note studio-draft-state",
          spellcheck: "false",
          autocapitalize: "off",
          autocorrect: "off",
        }),
        Prec.highest(
          EditorView.domEventHandlers({
            keydown(event) {
              if (event.isComposing) return false;
              const forwarded = new KeyboardEvent("keydown", {
                key: event.key,
                code: event.code,
                ctrlKey: event.ctrlKey,
                metaKey: event.metaKey,
                altKey: event.altKey,
                shiftKey: event.shiftKey,
                cancelable: true,
              });
              if (!textarea.dispatchEvent(forwarded)) {
                event.preventDefault();
                event.stopPropagation();
                return true;
              }
              return false;
            },
          }),
        ),
        EditorView.updateListener.of((update) => {
          if (syncing) return;
          mirror();
          historyStatus();
          if (update.docChanged) {
            textarea.value = update.state.doc.toString();
            // Assigning textarea.value resets its selection; mirror it again afterward.
            mirror();
            textarea.dispatchEvent(new Event("input", { bubbles: true }));
          } else if (update.selectionSet)
            textarea.dispatchEvent(new Event("select"));
        }),
        EditorView.theme({
          "&": {
            height: "100%",
            color: "var(--text)",
            backgroundColor: "var(--surface)",
          },
          ".cm-scroller": {
            overflow: "auto",
            fontFamily: "var(--mono)",
            lineHeight: "1.8",
          },
          ".cm-content": { padding: "16px 0", caretColor: "var(--text)" },
          ".cm-gutters": {
            backgroundColor: "var(--bg-2)",
            color: "var(--text-muted)",
            borderRight: "1px solid var(--line)",
          },
          ".cm-activeLine, .cm-activeLineGutter": {
            backgroundColor: "var(--accent-bg)",
          },
          ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--text)" },
          "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection":
            { backgroundColor: "var(--code-selection)" },
          ".cm-tooltip": {
            backgroundColor: "var(--surface)",
            color: "var(--text)",
            border: "1px solid var(--line-strong)",
          },
        }),
      ],
    });
  }
  $("#studio-code-files").addEventListener("click", () =>
    $("#studio-find-files").click(),
  );
  try {
    view = new EditorView({ parent, state: create("", "") });
  } catch {
    parent.hidden = true;
    tools.hidden = true;
    wrapButton.hidden = true;
    codeButton.disabled = true;
    codeButton.setAttribute("aria-pressed", "false");
    plainButton.setAttribute("aria-pressed", "true");
    note.textContent =
      "Code editor unavailable in this browser. Plain text editing and explicit saves remain available.";
    return {
      open() {},
      forget() {},
      reset() {},
      focus: () => textarea.focus(),
      selectRange(start, end, focus) {
        if (focus) textarea.focus();
        textarea.setSelectionRange(start, end);
        textarea.scrollTop = Math.max(
          0,
          textarea.value.slice(0, start).split("\n").length * 24 -
            textarea.clientHeight / 2,
        );
      },
    };
  }
  function choose(next) {
    plain = next;
    parent.hidden = plain;
    textarea.hidden = !plain;
    tools.hidden = plain;
    wrapButton.hidden = plain;
    codeButton.setAttribute("aria-pressed", String(!plain));
    plainButton.setAttribute("aria-pressed", String(plain));
    note.textContent = plain
      ? "Plain text mode · your draft is shared with Code mode. Saving is explicit."
      : "Syntax and local suggestions · no project language server. Tab moves focus; Ctrl+Space opens suggestions.";
    if (plain) textarea.focus({ preventScroll: true });
    else {
      view.requestMeasure();
      view.focus();
    }
  }
  wrapButton.addEventListener("click", () => {
    wrapLines = !wrapLines;
    wrapButton.setAttribute("aria-pressed", String(wrapLines));
    view.dispatch({
      effects: wrapping.reconfigure(wrapLines ? EditorView.lineWrapping : []),
    });
    view.focus();
  });
  codeButton.addEventListener("click", () => choose(false));
  plainButton.addEventListener("click", () => choose(true));
  undoButton.addEventListener("click", () => {
    view.focus();
    undo(view);
  });
  redoButton.addEventListener("click", () => {
    view.focus();
    redo(view);
  });
  choose(false);
  historyStatus();
  return {
    open(path, text) {
      syncing = true;
      try {
        if (key !== path) {
          if (key != null) states.set(key, view.state);
          key = path;
          view.setState(states.get(path) || create(path, text));
          view.dispatch({
            effects: wrapping.reconfigure(
              wrapLines ? EditorView.lineWrapping : [],
            ),
          });
        }
        if (view.state.doc.toString() !== text)
          view.dispatch({
            changes: { from: 0, to: view.state.doc.length, insert: text },
            annotations: isolateHistory.of("full"),
            selection: {
              anchor: Math.min(textarea.selectionStart, text.length),
              head: Math.min(textarea.selectionEnd, text.length),
            },
          });
        if (!plain) mirror();
        $("#studio-code-language").textContent = codeLanguage(path);
        historyStatus();
        view.requestMeasure();
      } finally {
        syncing = false;
      }
    },
    forget(path) {
      states.delete(path);
      if (path === key) {
        key = null;
        syncing = true;
        view.setState(create("", ""));
        syncing = false;
      }
    },
    reset() {
      if (key != null) {
        states.delete(key);
        syncing = true;
        view.setState(create(key, textarea.value));
        syncing = false;
        historyStatus();
      }
    },
    focus() {
      if (plain) textarea.focus({ preventScroll: true });
      else view.focus();
    },
    selectRange(start, end, focus = false) {
      if (plain) {
        if (focus) textarea.focus({ preventScroll: true });
        textarea.setSelectionRange(start, end);
        const height = parseFloat(getComputedStyle(textarea).lineHeight) || 24;
        textarea.scrollTop = Math.max(
          0,
          (textarea.value.slice(0, start).split("\n").length - 1) * height -
            textarea.clientHeight / 2,
        );
        textarea.scrollLeft = 0;
      } else {
        if (focus) view.focus();
        view.dispatch({
          selection: { anchor: start, head: end },
          effects: EditorView.scrollIntoView(start, { y: "center" }),
        });
        mirror();
      }
    },
  };
}
