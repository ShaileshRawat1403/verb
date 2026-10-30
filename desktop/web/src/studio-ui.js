import { readWithBusyRetry } from "./read-retry.js";
// Studio is a second presentation of the existing project, Specs and sessions.
// It never launches work on entry or owns another task store. App execution is explicit.
import { renderMarkdown, specSessions, STAGE_GUIDE } from "./workbench.js";
import { icon } from "./icons.js";
import { initStudioPreviews } from "./studio-previews.js";
import { initStudioEditor } from "./studio-editor.js";
import { mountConversation } from "./conversation-ui.js";
import { initStudioBrowser } from "./studio-browser.js";

export function initStudio({
  $,
  api,
  ui,
  workbench,
  showView,
  selectTerminal,
  escapeHtml,
  toast,
  agentDisplayName,
  agentMark,
}) {
  const root = $("#view-studio");
  const state = {
    files: [],
    loaded: false,
    selected: null,
    request: 0,
    mode: "document",
    context: null,
    composeScope: undefined,
    composeChoice: null,
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
  const browser = initStudioBrowser({ $, api, beforeOpen() {
    bridge.document();
    if (!$("#studio-changes-panel").hidden) $("#studio-toggle-changes").click();
  } });
  for (const selector of ["#studio-show-document", "#studio-edit-file", "#studio-preview-app", "#studio-source"])
    $(selector).addEventListener("click", () => browser.hide());
  $("#studio-toggle-changes").addEventListener("click", () => browser.hide({ restore: false }));
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
      const result = await readWithBusyRetry(api, "/api/files");
      state.files = result.files ?? [];
      state.loaded = true;
      state.truncated = result.truncated;
      renderFileOptions();
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
      message(`Could not read files: ${error.message}`);
      setText("#studio-file-note", "Files unavailable. Refresh to retry.");
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

  // The agent conversation: the same panel as the Sessions tile, mounted in the work thread.
  // Choosing a session only reads it; nothing starts, and the terminal is one step away.
  const conversation = { id: null, panel: null };
  function showChangeDraft(open, { focus = false, user = false } = {}) {
    if (user) state.composeChoice = open;
    else if (state.composeChoice != null) open = state.composeChoice;
    const form = $("#studio-intent-form");
    const toggle = $("#studio-new-change-toggle");
    form.hidden = !open;
    toggle.hidden = !conversation.id && !workbench.current().spec;
    toggle.setAttribute("aria-expanded", String(open));
    toggle.textContent = open ? "Hide new change" : "Define another change";
    if (focus && open) $("#studio-intent").focus();
    if (focus && !open) toggle.focus();
  }
  $("#studio-new-change-toggle").addEventListener("click", () => {
    showChangeDraft($("#studio-intent-form").hidden, { focus: true, user: true });
  });
  function agentSessions(spec) {
    const live = (ui.state?.sessions || []).filter(
      (s) => s.state === "live" && s.hostedHere && s.agent && s.agent !== "shell",
    );
    const forSpec = new Set(
      spec ? specSessions(spec, ui.state?.sessions || []).map((s) => s.id).filter(Boolean) : [],
    );
    return live.sort((a, b) => Number(forSpec.has(b.id)) - Number(forSpec.has(a.id)) || (b.lastSeenAt ?? 0) - (a.lastSeenAt ?? 0)).map((s) => ({ ...s, forSpec: forSpec.has(s.id) }));
  }
  function renderConversationPicker(spec) {
    const sessions = agentSessions(spec);
    const picker = $("#studio-conversation-session");
    const signature = JSON.stringify([sessions.map((s) => [s.id, s.forSpec]), conversation.id]);
    if (picker.dataset.signature === signature) return;
    picker.dataset.signature = signature;
    const options = sessions.map(
      (s) => new Option(`${agentDisplayName(s.agent)} · ${s.id.slice(0, 6)}${s.forSpec ? " · this work" : ""}`, s.id),
    );
    if (conversation.id && !sessions.some((s) => s.id === conversation.id))
      options.unshift(new Option(`${conversation.id.slice(0, 6)} · no longer running`, conversation.id));
    picker.replaceChildren(
      new Option(sessions.length ? "Choose a running agent session…" : "No running agent sessions", ""),
      ...options,
    );
    picker.value = conversation.id ?? "";
    $("#studio-conversation-empty").hidden = Boolean(conversation.id);
  }
  function selectConversation(id) {
    if (conversation.id === id) return;
    conversation.panel?.dispose();
    conversation.panel = null;
    conversation.id = id || null;
    // Keep an existing work draft visible; otherwise make room for the active conversation.
    showChangeDraft((!id && !workbench.current().spec) || Boolean($("#studio-intent").value.trim() || state.context));
    const mount = $("#studio-conversation-mount");
    mount.replaceChildren();
    mount.className = "";
    $("#studio-conversation-empty").hidden = Boolean(id);
    if (!id) {
      renderConversationPicker(workbench.current().spec);
      return;
    }
    const session = (ui.state?.sessions || []).find((s) => s.id === id);
    const agent = session?.agent ?? "agent";
    conversation.panel = mountConversation(mount, id, {
      api,
      escapeHtml,
      icon,
      toast,
      agentName: agentDisplayName(agent),
      agentMark: agentMark(agent),
      workbench: () => workbench,
      openFile(path) {
        $("#studio-file").value = path;
        openFile(path);
        focusSurface("canvas", { focus: true });
      },
      openTerminal() {
        showView("sessions");
        selectTerminal(id);
        ui.terminals?.get(id)?.stream?.setMode("terminal");
      },
    });
    if (ui.view === "studio") conversation.panel.start();
    renderConversationPicker(workbench.current().spec);
  }
  $("#studio-conversation-session").addEventListener("change", (event) =>
    selectConversation(event.target.value),
  );

  function render() {
    if (ui.view !== "studio") return;
    const { spec, presentation, specs, error, loaded } = workbench.current();
    renderConversationPicker(spec);
    root.dataset.hasTask = String(Boolean(spec && !error));
    const composeScope = spec?.id || conversation.id || null;
    if (!loaded) {
      // Do not briefly invite a second task while the current one is still loading.
      $("#studio-intent-form").hidden = true;
      $("#studio-new-change-toggle").hidden = true;
      state.composeScope = undefined;
    } else if (state.composeScope !== composeScope) {
      state.composeScope = composeScope;
      showChangeDraft(!composeScope || Boolean($("#studio-intent").value.trim() || state.context || document.activeElement === $("#studio-intent")));
    }
    $("#studio-history").hidden = !spec || Boolean(error);
    setText("#studio-history-count", spec ? `(${spec.audit?.length || 0})` : "");
    setText("#studio-stage-label", spec ? `${STAGE_GUIDE[spec.stage]?.label || spec.stage} · chosen stage` : "");
    const next = {
      spec: ["Agree on the outcome and what would demonstrate success.", "Review task & criteria"],
      plan: ["Break the work into manageable steps and make its scope clear.", "Review the plan"],
      build: ["Choose a terminal or an installed agent to work on this task.", "Choose how to work"],
      verify: ["Check each acceptance criterion against current evidence.", "Review evidence"],
      review: ["Inspect the actual changes before deciding what to keep.", "Review changes"],
      ship: ["Review delivery records. A chosen stage does not confirm a release.", "Review delivery"],
    }[spec?.stage];
    setText("#studio-next-hint", next?.[0] || "Review the task and decide how to continue.");
    setText("#studio-next-action", next?.[1] || "Inspect task");
    if ($("#studio-checks").dataset.checkSpec !== (spec?.id || "")) {
      $("#studio-checks").dataset.checkSpec = spec?.id || "";
      setText("#studio-check-status", "");
    }
    $("#studio-checks").hidden = !spec || Boolean(error);
    $("#studio-run-check").disabled = !spec || Boolean(error) || !spec.criteria?.length;
    setText("#studio-check-summary", presentation?.cardVerification || "Evidence has not been established.");
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
        : !loaded ? "Loading your work…" : spec?.title || "What would you like to change?",
    );
    const rawProblem = spec?.body
      ?.split(/^## Problem[ \t]*\r?$/m)[1]
      ?.split(/^## /m)[0]
      ?.trim();
    const problem = rawProblem === "_What is wrong or missing today, and why it matters._" ? "" : rawProblem;
    setText(
      "#studio-thread-problem",
      error || problem || (spec
        ? "Review the task’s criteria, then choose how to continue."
        : "Describe an outcome. Define how to check it, then choose how to do the work."),
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
            `<button class="studio-link" type="button" data-studio-conversation="${escapeHtml(s.id)}">${icon("ask")} Read ${escapeHtml(agentDisplayName(s.agent))}'s conversation <small>${escapeHtml(s.short)}</small></button><button class="studio-link" type="button" data-studio-session="${escapeHtml(s.id)}">${icon("terminal")} Open its terminal</button>`,
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
  $("#studio-find-files").addEventListener("click", () => {
    browser.hide();
    if (!$("#studio-changes-panel").hidden) $("#studio-toggle-changes").click();
    bridge.document();
    focusSurface("canvas");
    $("#studio-file-search").focus();
    $("#studio-file-search").scrollIntoView({ block: "nearest" });
    message("Choose a file here. Your editor drafts are kept in this tab.");
  });
  $("#studio-review-changes").addEventListener("click", () => {
    focusSurface("canvas");
    if ($("#studio-changes-panel").hidden) $("#studio-toggle-changes").click();
    $("#studio-changes-panel").focus();
    $("#studio-changes-panel").scrollIntoView({ block: "nearest" });
    message("Inspect the changed files first. Commit changes opens a separate review; nothing is uploaded.");
  });
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
    showChangeDraft(true, { user: true });
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
    state.composeChoice = null;
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
  document.addEventListener("verb:evidence-status", (event) => {
    if (event.detail.specId === workbench.current().spec?.id)
      setText("#studio-check-status", event.detail.message);
  });
  $("#studio-run-check").addEventListener("click", async () => {
    const button = $("#studio-run-check");
    button.disabled = true;
    setText("#studio-check-status", "Reading current source and evidence…");
    try {
      await workbench.runCheck();
      setText("#studio-check-status", "Review the command and criterion. Nothing runs until you choose Run check.");
    } catch (error) {
      setText("#studio-check-status", error.message);
    } finally { button.disabled = !workbench.current().spec?.criteria?.length; }
  });
  $("#studio-review-evidence").addEventListener("click", () => workbench.reviewEvidence());
  $("#studio-next-action").addEventListener("click", () => {
    const spec = workbench.current().spec;
    if (!spec || workbench.current().error) return;
    if (spec.stage === "review") $("#studio-review-changes").click();
    else if (spec.stage === "build") showView("sessions");
    else if (spec.stage === "verify") workbench.reviewEvidence();
    else workbench.selectSpec(spec.id);
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
    const read = event.target.closest("[data-studio-conversation]");
    if (read) {
      selectConversation(read.dataset.studioConversation);
      $("#studio-conversation").scrollIntoView({ block: "nearest" });
      conversation.panel?.focus();
      return;
    }
    const session = event.target.closest("[data-studio-session]");
    if (session) {
      showView("sessions");
      selectTerminal(session.dataset.studioSession);
    }
  });
  return {
    render,
    acceptExternalResult({ source, text }) {
      if (state.context) { toast('A context is already attached to the Studio draft. Remove it before adding another result.'); return false; }
      state.context = { path: `MCP · ${source}`, text: `External, untrusted connector result:\n${text.slice(0, 3800)}${text.length > 3800 ? '\n[Result truncated for this draft]' : ''}` };
      showView('studio'); renderContext(); showChangeDraft(true, { user: true });
      focusSurface('work'); $('#studio-intent').focus();
      message('Connector result attached to your draft. Nothing was sent to an agent.');
      return true;
    },
    onSpecCreated(problem) {
      if (draftProblem() === problem.trim()) {
        $("#studio-intent").value = "";
        state.context = null;
        renderContext();
      }
      state.composeChoice = null;
      if (workbench.current().spec) showChangeDraft(false);
      focusSurface("work", { focus: true });
      message(
        "Work defined. Inspect its criteria and choose a terminal or agent when ready.",
      );
    },
    onShowView(view) {
      if (view !== "studio") {
        browser.hide();
        bridge.document(); // Stop app code on navigation; retain file drafts.
        conversation.panel?.stop();
        return;
      }
      conversation.panel?.start();
      render();
      if (!state.loaded) refreshFiles();
    },
  };
}
