// Studio is a second presentation of the existing project, Specs and sessions.
// It never launches work on entry or owns another task store. App execution is explicit.
import { renderMarkdown, specSessions, STAGE_GUIDE } from "./workbench.js";
import { icon } from "./icons.js";
import { initStudioPreviews } from "./studio-previews.js";
import { initStudioEditor } from "./studio-editor.js";

export function initStudio({
  $,
  api,
  ui,
  workbench,
  showView,
  selectTerminal,
  escapeHtml,
}) {
  const root = $("#view-studio");
  const state = {
    files: [],
    loaded: false,
    selected: null,
    request: 0,
    mode: "document",
    context: null,
    focus: "canvas",
  };
  const setText = (selector, text) => {
    if ($(selector).textContent !== text) $(selector).textContent = text;
  };
  const paint = (selector, html) => {
    if ($(selector).innerHTML !== html) $(selector).innerHTML = html;
  };
  const message = (text) => setText("#studio-message", text);

  const bridge = initStudioEditor({
    $,
    api,
    onMode(next) {
      root.dataset.canvasMode = next;
      if (next === "document") renderDocument();
      else
        setText(
          "#studio-document-meta",
          next === "edit"
            ? "File editor · explicit save"
            : "Isolated application preview · saved snapshot",
        );
    },
    onSaved(savedPath, savedFile) {
      workbench.onShowView("studio");
      document.dispatchEvent(new CustomEvent("verb:git-changed"));
      if (savedPath === state.selected) {
        // The confirmed save is the source snapshot. Invalidate any older read in flight.
        ++state.request;
        state.preview = { ...savedFile, reason: null };
        if (bridge.mode() === "document") renderDocument();
      }
    },
  });
  const dock = initStudioPreviews({
    $,
    api,
    openFile,
    escapeHtml,
    workspaceId: () => ui.state?.project?.id,
    canClose: (path) => !bridge.hasUnsaved(path),
    stopApp: () => bridge.document(),
  });
  // Keep one set of development actions reachable from both phone surfaces.
  const tools = $(".studio-work-tools");
  const toolsHome = document.createComment("Studio development tools");
  tools.before(toolsHome);
  const narrow = matchMedia("(max-width: 820px)");
  function placeTools() {
    if (narrow.matches) root.insertBefore(tools, $(".studio-layout"));
    else toolsHome.after(tools);
  }
  narrow.addEventListener("change", placeTools);
  placeTools();
  bridge.connect();

  function focusSurface(name, { focus = false } = {}) {
    state.focus = name;
    root.dataset.focus = name;
    for (const panel of ["canvas", "work"]) {
      $(`#studio-focus-${panel}`).setAttribute(
        "aria-pressed",
        String(panel === name),
      );
    }
    if (focus && matchMedia("(max-width: 820px)").matches) {
      $(`#studio-focus-${name}`).focus({ preventScroll: true });
      $(".studio-focus").scrollIntoView({ block: "start" });
    }
  }
  function renderFileOptions() {
    const words = $("#studio-file-search")
      .value.trim()
      .toLowerCase()
      .split(/\s+/)
      .filter(Boolean);
    const matches = state.files.filter((file) =>
      words.every((word) => file.path.toLowerCase().includes(word)),
    );
    const current = state.files.find((file) => file.path === state.selected);
    const options = matches.map((file) => new Option(file.path, file.path));
    if (current && !matches.includes(current))
      options.unshift(new Option(`${current.path} · open`, current.path));
    $("#studio-file").replaceChildren(
      new Option(
        matches.length ? "Choose a project file…" : "No matching files",
        "",
      ),
      ...options,
    );
    $("#studio-file").value = state.selected || "";
    setText(
      "#studio-file-note",
      words.length
        ? `${matches.length} matching file${matches.length === 1 ? "" : "s"}${state.truncated ? " in this partial listing" : ""}. The open document stays in place.`
        : state.truncated
          ? "File listing is partial. Use Files for search and navigation."
          : "Only files exposed by this workspace are listed. Ignored secrets stay hidden.",
    );
  }
  function selectedContext() {
    const selection = document.getSelection();
    const doc = $("#studio-document");
    if (
      !selection?.rangeCount ||
      !state.preview?.text ||
      !doc.contains(selection.anchorNode) ||
      !doc.contains(selection.focusNode)
    )
      return null;
    const text = selection.toString().trim();
    return text && text.length <= 4000 ? { path: state.selected, text } : null;
  }
  function renderContext() {
    $("#studio-context").hidden = !state.context;
    setText("#studio-context-path", state.context?.path || "");
    setText("#studio-context-text", state.context?.text || "");
  }
  function draftProblem() {
    const intent = $("#studio-intent").value.trim();
    return state.context
      ? `${intent}\n\nContext from ${state.context.path}:\n${state.context.text}`
      : intent;
  }

  async function refreshFiles() {
    try {
      const result = await api("GET", "/api/files");
      state.files = result.files ?? [];
      state.loaded = true;
      state.truncated = result.truncated;
      renderFileOptions();
      if (!state.selected) {
        const path =
          state.files.find((f) => f.path === "docs/project/BRIEF.md")?.path ??
          state.files.find((f) => /^readme\.md$/i.test(f.path))?.path;
        if (path) {
          $("#studio-file").value = path;
          await openFile(path);
        }
      }
    } catch (error) {
      message(`Could not read files: ${error.message}`);
      setText("#studio-file-note", "Files unavailable. Refresh to retry.");
    }
  }

  async function openFile(path) {
    if (!dock.open(path)) return;
    const request = ++state.request;
    state.selected = path;
    renderFileOptions();
    bridge.select(path);
    state.mode = "document";
    $("#studio-source").setAttribute("aria-pressed", "false");
    setText("#studio-document-path", path || "Project canvas");
    setText("#studio-document-meta", "Reading file…");
    $("#studio-document").textContent = "";
    state.preview = null;
    $("#studio-source").disabled = true;
    $("#studio-use-selection").disabled = true;
    if (!path) {
      setText("#studio-document-meta", "No preview selected");
      $("#studio-document").textContent =
        "Open a project file to preview it beside your work.";
      return;
    }
    if (dock.isImage(path)) {
      setText("#studio-document-meta", "Image preview · original file");
      await dock.image(path, () => request === state.request);
      return;
    }
    try {
      const result = await api(
        "GET",
        `/api/files/preview?path=${encodeURIComponent(path)}`,
      );
      if (request !== state.request) return;
      state.preview = result;
      renderDocument();
    } catch (error) {
      if (request !== state.request) return;
      setText("#studio-document-meta", "File unavailable");
      $("#studio-document").textContent = error.message;
    }
  }

  function renderDocument() {
    const preview = state.preview;
    if (!preview) return;
    if (bridge.mode() === "document")
      setText("#studio-document-meta", `${preview.bytes} bytes · read-only`);
    $("#studio-source").disabled =
      preview.text == null || !/\.(md|mdx|markdown)$/i.test(state.selected);
    if (preview.text == null) {
      $("#studio-document").textContent =
        `Not shown: ${preview.reason || "preview unavailable"}.`;
      return;
    }
    // Only the existing escaped Markdown renderer may produce markup. Project HTML remains text.
    const markdown =
      state.mode !== "source" && /\.(md|mdx|markdown)$/i.test(state.selected);
    $("#studio-document").classList.toggle("studio-source", !markdown);
    if (markdown)
      $("#studio-document").innerHTML = renderMarkdown(
        preview.text,
        escapeHtml,
      );
    else $("#studio-document").textContent = preview.text;
  }

  function render() {
    if (ui.view !== "studio") return;
    const { spec, presentation, specs, error, loaded } = workbench.current();
    const picker = $("#studio-thread");
    const signature = JSON.stringify([
      specs.map((s) => [s.id, s.title]),
      spec?.id,
      error,
      loaded,
    ]);
    if (picker.dataset.signature !== signature) {
      picker.replaceChildren(
        new Option(
          error
            ? "Work unavailable"
            : loaded
              ? "Choose work…"
              : "Reading work…",
          "",
        ),
        ...specs.map((s) => new Option(`${s.id} · ${s.title}`, s.id)),
      );
      picker.value = spec?.id ?? "";
      picker.dataset.signature = signature;
    }
    setText("#studio-project-name", ui.state?.project?.name || "Your project");
    setText(
      "#studio-thread-title",
      error
        ? "Work unavailable"
        : spec?.title || "What would you like to change?",
    );
    const problem = spec?.body
      ?.split(/^## Problem[ \t]*\r?$/m)[1]
      ?.split(/^## /m)[0]
      ?.trim();
    setText(
      "#studio-thread-problem",
      error ||
        problem ||
        "Describe an outcome. Define how to check it, then choose how to do the work.",
    );
    $("#studio-thread-error").hidden = !error;
    setText("#studio-thread-error", error || "");
    $("#studio-spec-context").hidden = !spec || Boolean(error);
    $("#studio-inspect-work").disabled = !spec || Boolean(error);
    const facts = presentation
      ? [
          ["Stage", STAGE_GUIDE[spec.stage]?.label || spec.stage],
          ["Execution", presentation.execution],
          ["Verification", presentation.cardVerification],
          ["Delivery", presentation.delivery],
        ]
      : [];
    paint(
      "#studio-facts",
      facts
        .map(
          ([label, value]) =>
            `<div><dt>${label}</dt><dd>${escapeHtml(value)}</dd></div>`,
        )
        .join(""),
    );
    paint(
      "#studio-criteria",
      (spec?.criteria || [])
        .map(
          (c) =>
            `<li>${icon(c.done ? "file-check" : "square", { size: 15 })}<span>${escapeHtml(c.text)}<small>${c.done ? "Recorded complete · see evidence for proof and freshness" : "Evidence required"}</small></span></li>`,
        )
        .join(""),
    );
    const sessions = spec
      ? specSessions(spec, ui.state?.sessions || []).filter(
          (s) => s.live && s.id,
        )
      : [];
    paint(
      "#studio-live-sessions",
      sessions
        .map(
          (s) =>
            `<button class="studio-link" type="button" data-studio-session="${escapeHtml(s.id)}">${icon("terminal")} Continue ${escapeHtml(s.agent)} <small>${escapeHtml(s.short)}</small></button>`,
        )
        .join(""),
    );
    paint(
      "#studio-work-activity",
      (spec?.audit || [])
        .slice(-16)
        .map(
          (entry) =>
            `<li><div><strong>${escapeHtml(entry.actor)}</strong><time>${escapeHtml(entry.at.replace("T", " ").slice(0, 16))}</time></div><p>${escapeHtml(entry.action)}</p></li>`,
        )
        .join(""),
    );
    const git = workbench.current().git;
    setText(
      "#studio-checkout",
      git?.error
        ? `Git unavailable: ${git.error}`
        : git
          ? `${git.changes?.length ?? 0} checkout changes · attribution to this work is not established`
          : "Reading Git observations…",
    );
  }

  $("#studio-focus-canvas").addEventListener("click", () =>
    focusSurface("canvas"),
  );
  $("#studio-focus-work").addEventListener("click", () => focusSurface("work"));
  $("#studio-file-search").addEventListener("input", renderFileOptions);
  document.addEventListener("selectionchange", () => {
    $("#studio-use-selection").disabled = !selectedContext();
  });
  $("#studio-use-selection").addEventListener("pointerdown", (event) => {
    if (event.button === 0) event.preventDefault(); // Preserve the selected text until activation.
  });
  $("#studio-use-selection").addEventListener("click", () => {
    const context = selectedContext();
    if (!context) return;
    state.context = context;
    renderContext();
    focusSurface("work");
    $("#studio-intent").focus();
    message(
      "Selection attached to your draft. Review it before creating the Spec.",
    );
  });
  $("#studio-clear-context").addEventListener("click", () => {
    state.context = null;
    renderContext();
    $("#studio-intent").focus();
  });
  $("#studio-file").addEventListener("change", (event) =>
    openFile(event.target.value),
  );
  $("#studio-thread").addEventListener("change", (event) => {
    if (event.target.value)
      workbench.selectSpec(event.target.value, { navigate: false });
    render();
  });
  $("#studio-refresh").addEventListener("click", async () => {
    message("");
    const selected = state.selected;
    await refreshFiles();
    if (selected) await openFile(selected);
    workbench.onShowView("studio");
  });
  $("#studio-source").addEventListener("click", () => {
    bridge.document();
    state.mode = state.mode === "source" ? "document" : "source";
    $("#studio-source").setAttribute(
      "aria-pressed",
      String(state.mode === "source"),
    );
    renderDocument();
  });
  $("#studio-inspect-work").addEventListener("click", () => {
    const spec = workbench.current().spec;
    if (spec && !workbench.current().error) workbench.selectSpec(spec.id);
  });
  $("#studio-intent-form").addEventListener("submit", (event) => {
    event.preventDefault();
    const intent = $("#studio-intent").value.trim();
    if (!intent) {
      $("#studio-intent").focus();
      return;
    }
    // Preparing the existing form is reversible; its explicit submit is the only write.
    workbench.openNewSpec({
      title: intent.split("\n")[0].slice(0, 120),
      problem: draftProblem(),
    });
  });
  root.addEventListener("click", (event) => {
    const session = event.target.closest("[data-studio-session]");
    if (session) {
      showView("sessions");
      selectTerminal(session.dataset.studioSession);
    }
  });
  return {
    render,
    onSpecCreated(problem) {
      if (draftProblem() === problem.trim()) {
        $("#studio-intent").value = "";
        state.context = null;
        renderContext();
      }
      focusSurface("work", { focus: true });
      message(
        "Work defined. Inspect its criteria and choose a terminal or agent when ready.",
      );
    },
    onShowView(view) {
      if (view !== "studio") {
        bridge.document(); // Stop app code on navigation; retain file drafts.
        return;
      }
      render();
      if (!state.loaded) refreshFiles();
    },
  };
}
