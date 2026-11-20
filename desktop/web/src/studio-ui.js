import { readWithBusyRetry } from "./read-retry.js";
// The Preview tab's canvas: project files as documents, images, an editor, the app preview and the
// browser. It reads the workspace and never launches work on its own; app execution is explicit.
import { renderMarkdown } from "./workbench.js";
import { initStudioPreviews } from "./studio-previews.js";
import { initStudioEditor } from "./studio-editor.js";
import { initStudioBrowser } from "./studio-browser.js";

export function initStudio({ $, api, readProjectFiles, ui, escapeHtml }) {
  const root = $("#preview-home");
  const canvas = $("#studio-canvas-panel");
  const canvasHome = document.createComment("Preview canvas");
  canvas.before(canvasHome);
  let previewMounted = false;
  let filesRequest;

  const state = {
    files: [],
    loaded: false,
    filesUnavailable: false,
    selected: null,
    request: 0,
    mode: "document",
  };
  const setText = (selector, text) => {
    if ($(selector).textContent !== text) $(selector).textContent = text;
  };
  const message = (text) => setText("#studio-message", text);

  const bridge = initStudioEditor({
    $,
    api,
    inBench: () => previewMounted,
    onCapability: renderPreviewLimit,
    onMode(next) {
      root.dataset.canvasMode = canvas.dataset.canvasMode = next;
      if (previewMounted && next !== "edit") {
        $("#studio-source").setAttribute("aria-pressed", String(next === "document" && state.mode === "source"));
        $("#studio-show-document").setAttribute("aria-pressed", String(state.mode !== "source" || next === "app"));
      }
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
  });
  const browser = initStudioBrowser({ $, api, beforeOpen() {
    bridge.document();
  } });
  for (const selector of ["#studio-show-document", "#studio-edit-file", "#studio-preview-app", "#studio-source"])
    $(selector).addEventListener("click", () => browser.hide());
  bridge.connect();

  function renderFileOptions() {
    const words = $("#studio-file-search")
      .value.trim()
      .toLowerCase()
      .split(/\s+/)
      .filter(Boolean);
    const matches = state.files.filter((file) =>
      words.every((word) => file.path.toLowerCase().includes(word)),
    );
    const results = $("#bench-file-results");
    results.hidden = !previewMounted || !words.length;
    if (!results.hidden) {
      results.innerHTML = matches.slice(0, 100).map(file =>
        `<li><button type="button" class="text-button" data-preview-file="${escapeHtml(file.path)}">${escapeHtml(file.path)}</button></li>`
      ).join("");
      setText("#studio-file-note", matches.length > 100 ? `Showing 100 of ${matches.length} matches. Narrow your search.` :
        matches.length ? "" : "No matching files.");
    } else if (previewMounted) {
      setText("#studio-file-note", state.loaded ? "" : "Reading files…");
    }
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
    if (state.filesUnavailable) {
      setText("#studio-file-note", "Files unavailable. Run Refresh project (⌘K) to retry.");
      return;
    }
    if (previewMounted) return;
    setText(
      "#studio-file-note",
      words.length
        ? `${matches.length} matching file${matches.length === 1 ? "" : "s"}${state.truncated ? " in this partial listing" : ""}. The open document stays in place.`
        : state.truncated
          ? "File listing is partial. Use Files for search and navigation."
          : "Only files exposed by this workspace are listed. Ignored secrets stay hidden.",
    );
  }
  function loadFiles() {
    filesRequest ??= refreshFiles().finally(() => { filesRequest = null; });
    return filesRequest;
  }

  function renderPreviewLimit() {
    const limit = $("#bench-file-limit");
    limit.hidden = !previewMounted || !state.selected || state.preview?.text == null || !bridge.readOnly();
    if (!limit.hidden) limit.textContent = "Read-only in this workspace.";
  }

  function setPreviewLabels(bench) {
    $("#studio-show-document").textContent = bench ? "Preview" : dock.isImage(state.selected) ? "Image" : "Document";
    $("#studio-edit-file").textContent = bench ? "Edit" : "Edit file";
    $("label[for=studio-file-search]").textContent = bench ? "Find a file" : "Find a canvas file";
    $("#studio-document-path").textContent = state.selected || (bench ? "Project files" : "Project canvas");
    if (state.preview) renderDocument();
    renderPreviewLimit();
  }

  function mountPreview(mount, visible) {
    if (mount && canvas.parentElement !== mount) {
      previewMounted = true;
      browser.hide();
      mount.append(canvas);
      setPreviewLabels(true);
      renderFileOptions();
      if (!state.loaded) loadFiles();
    } else if (!mount && previewMounted) {
      bridge.suspendPreview();
      previewMounted = false;
      canvasHome.after(canvas);
      setPreviewLabels(false);
      renderFileOptions();
    }
    if (!visible && previewMounted) bridge.suspendPreview();
    watchPreview(Boolean(previewMounted && visible));
  }

  // An agent edits files while they are open here. While the preview is on screen, the open file
  // is reread every few seconds and redrawn only when its content changed, keeping the scroll.
  // Edit mode is never touched, so unsaved edits cannot be replaced.
  const PREVIEW_WATCH_MS = 3000;
  let previewWatch = null;
  function watchPreview(on) {
    if (on === Boolean(previewWatch)) return;
    clearInterval(previewWatch);
    previewWatch = on ? setInterval(rereadOpenFile, PREVIEW_WATCH_MS) : null;
    if (on) rereadOpenFile();
  }
  async function rereadOpenFile() {
    const path = state.selected;
    if (!path || document.hidden || dock.isImage(path) || bridge.mode() !== "document" || !state.preview) return;
    const request = state.request;
    let result;
    try {
      result = await readWithBusyRetry(api, `/api/files/preview?path=${encodeURIComponent(path)}`);
    } catch {
      return; // The next tick tries again; a passing read error is not worth a message.
    }
    if (request !== state.request || path !== state.selected || bridge.mode() !== "document") return;
    if (result.text === state.preview?.text && result.bytes === state.preview?.bytes) return;
    const view = $("#studio-document");
    const scrollers = [view, view.parentElement].filter(Boolean).map((el) => [el, el.scrollTop]);
    state.preview = result;
    renderDocument();
    for (const [el, top] of scrollers) el.scrollTop = top;
  }
  document.addEventListener("visibilitychange", () => {
    if (!document.hidden && previewWatch) rereadOpenFile();
  });

  async function refreshFiles() {
    try {
      const result = await readProjectFiles();
      state.files = result.files ?? [];
      state.loaded = true;
      state.filesUnavailable = false;
      state.truncated = result.truncated;
      renderFileOptions();
      document.dispatchEvent(new Event("verb:preview-files"));
      // Background file discovery must not replace the browser the person just opened.
      if (!state.selected && $("#studio-browser").hidden) {
        const path =
          state.files.find((f) => f.path === "docs/project/BRIEF.md")?.path ??
          state.files.find((f) => /^readme\.md$/i.test(f.path))?.path;
        if (path) {
          $("#studio-file").value = path;
          await openFile(path);
        }
      }
    } catch (error) {
      state.filesUnavailable = true;
      message(`Could not read files: ${error.message}`);
      renderFileOptions();
    }
  }

  async function openFile(path) {
    if (!dock.open(path)) return;
    browser.hide();
    const request = ++state.request;
    state.selected = path;
    renderFileOptions();
    bridge.select(path);
    state.mode = "document";
    $("#studio-source").setAttribute("aria-pressed", "false");
    setText("#studio-document-path", path || (previewMounted ? "Project files" : "Project canvas"));
    if (previewMounted) $("#studio-show-document").textContent = "Preview";
    setText("#studio-document-meta", "Reading file…");
    $("#studio-document").textContent = "";
    state.preview = null;
    renderPreviewLimit();
    $("#studio-source").disabled = true;
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
      const result = await readWithBusyRetry(api,
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
    renderPreviewLimit();
    if (bridge.mode() === "document")
      setText("#studio-document-meta", `${preview.bytes} bytes · read-only`);
    $("#studio-source").disabled =
      preview.text == null || (!previewMounted && !/\.(md|mdx|markdown)$/i.test(state.selected));
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

  $("#bench-file-results").addEventListener("click", async event => {
    const button = event.target.closest("[data-preview-file]");
    if (!button) return;
    $("#studio-file-search").value = "";
    const path = button.dataset.previewFile;
    await openFile(path);
    if (previewMounted && !$("#bench-preview").hidden && !$("#bench-right-panel").hidden && state.selected === path && /\.html?$/i.test(path)) {
      bridge.showPreview();
    }
  });
  $("#studio-show-document").addEventListener("pointerdown", () => {
    if (previewMounted) state.mode = "document";
  });
  $("#studio-show-document").addEventListener("click", () => {
    if (!previewMounted) return;
    state.mode = "document";
    $("#studio-source").setAttribute("aria-pressed", "false");
    $("#studio-show-document").setAttribute("aria-pressed", "true");
    if (bridge.mode() === "document") renderDocument();
  });
  $("#studio-file-search").addEventListener("input", renderFileOptions);
  $("#studio-file").addEventListener("change", (event) =>
    openFile(event.target.value),
  );
  $("#studio-source").addEventListener("click", () => {
    bridge.document();
    state.mode = previewMounted || state.mode !== "source" ? "source" : "document";
    if (previewMounted) $("#studio-show-document").setAttribute("aria-pressed", "false");
    $("#studio-source").setAttribute(
      "aria-pressed",
      String(state.mode === "source"),
    );
    renderDocument();
  });
  return {
    mountPreview,
    openPreviewFile: openFile,
    hasPreviewFiles: () => state.files.length > 0,
    refreshFiles: loadFiles,
    onShowView(view) {
      browser.hide();
      if (view !== "sessions" || !previewMounted) bridge.document();
    },
  };
}
