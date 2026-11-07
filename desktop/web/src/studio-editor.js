// Local text editing is an adapter, not a replacement for the full IDE foundation.
// File drafts live in this tab only. No writes, app code, or server commands run on entry.
export function initStudioEditor({ $, api, onMode, onSaved }) {
  let capability = null,
    path = null,
    mode = "document",
    request = 0;
  const drafts = new Map();
  const editor = $("#studio-editor-text");
  const status = (text) => {
    $("#studio-editor-status").textContent = text;
  };
  const dirty = (draft) => draft && draft.text !== draft.base.text;
  const current = () => drafts.get(path);
  function setMode(next) {
    if (mode === "app" && next !== "app")
      $("#studio-app-frame").replaceChildren();
    mode = next;
    $("#studio-edit-panel").hidden = next !== "edit";
    $("#studio-app-panel").hidden = next !== "app";
    $("#studio-document").hidden = next !== "document";
    $("#studio-show-document").setAttribute(
      "aria-pressed",
      String(next === "document"),
    );
    $("#studio-edit-file").setAttribute(
      "aria-pressed",
      String(next === "edit"),
    );
    $("#studio-preview-app").setAttribute(
      "aria-pressed",
      String(next === "app"),
    );
    onMode(next);
    update();
  }
  function update() {
    const draft = current();
    $("#studio-edit-file").disabled =
      !capability?.editing.available ||
      !path ||
      /\.(png|jpe?g|gif|webp)$/i.test(path);
    $("#studio-preview-app").disabled =
      !capability?.preview.available ||
      !/\.html?$/i.test(path || "") ||
      Boolean(dirty(draft)) ||
      Boolean(draft?.saving);
    $("#studio-save-file").disabled = !dirty(draft) || Boolean(draft?.saving);
    $("#studio-reload-file").disabled = !draft || Boolean(draft?.saving);
    $("#studio-restore-draft").hidden = draft?.recovery == null;
    $("#studio-draft-state").textContent = draft?.saving
      ? "Saving…"
      : dirty(draft)
        ? "Unsaved draft · held in this tab"
        : draft
          ? `Saved on disk · ${draft.base.revision.slice(0, 8)}`
          : "";
    $("#studio-copy-draft").disabled = !draft;
  }
  function remember(doc) {
    if (!drafts.has(doc.path) && drafts.size >= 5) {
      const disposable = [...drafts].find(
        ([, d]) => !dirty(d) && !d.saving && !d.recovery,
      );
      if (!disposable)
        throw new Error(
          "Five drafts are open. Save a draft before editing another file.",
        );
      drafts.delete(disposable[0]);
    }
    const draft = { base: doc, text: doc.text, saving: false, recovery: null };
    drafts.set(doc.path, draft);
    return draft;
  }
  async function load(wanted) {
    const result = await api(
      "GET",
      `/api/studio/file?path=${encodeURIComponent(wanted)}`,
    );
    if (
      result.workspaceId !== capability?.workspaceId ||
      result.file.path !== wanted
    )
      throw new Error("Workspace changed. Reopen Studio before editing.");
    return result.file;
  }
  async function edit() {
    const wanted = path,
      generation = ++request;
    status("Opening file editor…");
    try {
      const draft = drafts.get(wanted) || remember(await load(wanted));
      if (generation !== request || path !== wanted) return;
      editor.value = draft.text;
      $("#studio-editor-conflict").hidden = true;
      setMode("edit");
      status("Explicit saves only. Saving does not commit or publish.");
      editor.focus({ preventScroll: true });
    } catch (error) {
      if (generation === request) status(error.message);
    }
  }
  async function save() {
    const wanted = path,
      draft = current();
    if (!dirty(draft) || draft.saving) return;
    const submitted = draft.text;
    draft.saving = true;
    update();
    status("Saving file…");
    try {
      const result = await api("POST", "/api/studio/file", {
        workspaceId: capability.workspaceId,
        path: wanted,
        expectedRevision: draft.base.revision,
        text: submitted,
      });
      if (
        result.workspaceId !== capability.workspaceId ||
        result.file.path !== wanted
      )
        throw new Error(
          "Save response could not be matched to this workspace. Reload before retrying.",
        );
      draft.base = result.file;
      if (draft.text === submitted) draft.recovery = null;
      if (path === wanted) {
        $("#studio-editor-conflict").hidden = true;
        status(
          dirty(draft)
            ? "Saved the submitted version. Your newer edits remain unsaved."
            : "Saved on disk. Review Git changes before committing; nothing published.",
        );
      }
      onSaved(wanted, result.file);
    } catch (error) {
      if (path === wanted) {
        status(`Save not confirmed: ${error.message}`);
        $("#studio-editor-conflict").hidden = error.status !== 409;
      }
    } finally {
      draft.saving = false;
      update();
    }
  }
  async function reload() {
    const wanted = path,
      generation = ++request,
      draft = current();
    if (!draft || draft.saving) return;
    status(
      "Reading the saved file; your current draft will remain recoverable…",
    );
    try {
      const doc = await load(wanted);
      if (generation !== request || wanted !== path) return;
      const recovery = dirty(draft) ? draft.text : draft.recovery;
      const next = remember(doc);
      next.recovery = recovery;
      editor.value = next.text;
      $("#studio-editor-conflict").hidden = true;
      status(
        recovery != null
          ? "Disk version loaded. Restore previous draft is available; inspect before saving."
          : "Disk version loaded.",
      );
      update();
    } catch (error) {
      if (generation === request) status(error.message);
    }
  }
  async function preview() {
    const wanted = path,
      generation = ++request;
    if (dirty(current())) {
      status("Save your draft first. App preview uses a saved file.");
      return;
    }
    status("Preparing isolated preview of the saved file…");
    try {
      const doc = await load(wanted);
      if (generation !== request || wanted !== path || dirty(current())) return;
      const result = await api("POST", "/api/studio/preview", {
        workspaceId: capability.workspaceId,
        path: wanted,
        expectedRevision: doc.revision,
      });
      if (generation !== request || wanted !== path || dirty(current())) return;
      if (
        result.workspaceId !== capability.workspaceId ||
        result.revision !== doc.revision ||
        !/^\/studio-preview\/[a-f0-9]{64}$/.test(result.url)
      )
        throw new Error("Preview identity could not be verified.");
      const frame = document.createElement("iframe");
      frame.title = `App preview of ${wanted}`;
      frame.setAttribute("sandbox", "allow-scripts");
      frame.referrerPolicy = "no-referrer";
      frame.setAttribute(
        "allow",
        "camera 'none'; microphone 'none'; geolocation 'none'; clipboard-read 'none'; clipboard-write 'none'; payment 'none'",
      );
      frame.src = result.url;
      $("#studio-app-frame").replaceChildren(frame);
      $("#studio-app-caption").textContent =
        `saved snapshot ${doc.revision.slice(0, 8)} · refresh restarts the app`;
      setMode("app");
      status(
        "Isolated HTML app preview. Inline scripts run; external assets and API connections are blocked.",
      );
    } catch (error) {
      if (generation === request) status(error.message);
    }
  }
  editor.addEventListener("input", () => {
    if (current()) current().text = editor.value;
    update();
  });
  editor.addEventListener("keydown", (event) => {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
      event.preventDefault();
      save();
    }
  });
  $("#studio-edit-file").addEventListener("click", edit);
  $("#studio-save-file").addEventListener("click", save);
  $("#studio-reload-file").addEventListener("click", reload);
  $("#studio-restore-draft").addEventListener("click", () => {
    const draft = current();
    if (draft?.recovery == null) return;
    const old = draft.text;
    draft.text = draft.recovery;
    draft.recovery = old;
    editor.value = draft.text;
    update();
    status(
      "Previous draft restored in the editor. Saving is a separate action.",
    );
  });
  $("#studio-copy-draft").addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(current().text);
      status("Draft copied.");
    } catch {
      editor.focus();
      editor.select();
      status("Select the editor text and copy it with your keyboard.");
    }
  });
  $("#studio-preview-app").addEventListener("click", preview);
  $("#studio-refresh-app").addEventListener("click", preview);
  $("#studio-show-document").addEventListener("click", () =>
    setMode("document"),
  );
  $("#studio-stop-app").addEventListener("click", () => {
    setMode("document");
    status("Preview stopped.");
  });
  window.addEventListener("beforeunload", (event) => {
    if ([...drafts.values()].some(dirty)) {
      event.preventDefault();
      event.returnValue = "";
    }
  });
  return {
    mode: () => mode,
    hasUnsaved: (wanted) => {
      const d = drafts.get(wanted);
      return Boolean(d && (dirty(d) || d.saving || d.recovery != null));
    },
    select(wanted) {
      ++request;
      path = wanted;
      setMode("document");
      status("");
    },
    document() {
      setMode("document");
    },
    async connect() {
      try {
        const result = await api("GET", "/api/studio/workspace");
        if (result.schemaVersion !== 1)
          throw new Error("Unsupported workspace bridge version");
        capability = result;
        $("#studio-bridge-note").textContent = result.editing.available
          ? `${result.transport === "monitored-v1" ? "Project boundary" : "Native-local"}: UTF-8 text edits up to 200 KB, explicit saves and self-contained HTML previews. Full IDE tooling is not connected yet.`
          : result.reason;
      } catch {
        $("#studio-bridge-note").textContent =
          "Workspace bridge unavailable. Project documents remain read-only.";
      }
      update();
    },
  };
}
