// The Project view: file tree, read-only preview, and the context hub (brief, agent context sync,
// project documents, nudges). Rules that can be tested live in workbench.js.

import { buildTree, changeLabel, filterFiles, renderMarkdown } from "./workbench.js";
import { fileIconName, icon } from "./icons.js";

export function initProject(deps) {
  const { api, toast, showDialog, closeDialog, escapeHtml, $ } = deps;
  const state = {
    files: [],
    truncated: false,
    open: new Set([""]),
    selected: null,
    hub: null,
    loadedAt: 0,
  };

  // ------------------------------------------------------------------------------ data

  async function refreshFiles() {
    try {
      const result = await api("GET", "/api/files");
      state.files = result.files;
      state.truncated = result.truncated;
      state.loadedAt = Date.now();
      renderTree();
    } catch (error) {
      $("#file-tree").innerHTML = `<p class="muted">${escapeHtml(error.message)}</p>`;
    }
  }
  async function refreshHub() {
    try {
      state.hub = await api("GET", "/api/hub");
      renderHub();
    } catch (error) {
      toast(error.message, "error");
    }
  }

  // ---------------------------------------------------------------------------- render

  const statusBadge = (status) =>
    status
      ? `<span class="file-status" data-kind="${escapeHtml(status)}" title="${escapeHtml(changeLabel(status))}">${escapeHtml(status === "??" ? "U" : status[0])}</span>`
      : "";

  function fileRow(file, depth, showPath = false) {
    const current = file.path === state.selected ? " current" : "";
    return `<button type="button" role="treeitem" class="tree-row file${current}" style="--depth:${depth}" data-file="${escapeHtml(file.path)}" title="${escapeHtml(file.path)}"><span class="tree-icon">${icon(fileIconName(file.name ?? file.path), { size: 14 })}</span><span class="tree-name">${escapeHtml(showPath ? file.path : file.name)}</span>${statusBadge(file.status)}</button>`;
  }

  function renderDir(dir, depth) {
    let html = "";
    for (const child of dir.dirs) {
      const open = state.open.has(child.path);
      html += `<button type="button" role="treeitem" aria-expanded="${open}" class="tree-row dir" style="--depth:${depth}" data-dir="${escapeHtml(child.path)}"><span class="tree-caret">${icon(open ? "chevron-down" : "chevron-right", { size: 12 })}</span><span class="tree-icon">${icon(open ? "folder-open" : "folder", { size: 14 })}</span><span class="tree-name">${escapeHtml(child.name)}</span>${child.changed ? `<span class="dir-changed" title="${child.changed} changed">${child.changed}</span>` : ""}</button>`;
      if (open) html += `<div role="group">${renderDir(child, depth + 1)}</div>`;
    }
    for (const file of dir.files) html += fileRow(file, depth);
    return html;
  }

  function renderTree() {
    const query = $("#file-filter").value;
    $("#file-count").textContent = state.truncated ? `${state.files.length}+` : state.files.length;
    if (!state.files.length) {
      $("#file-tree").innerHTML = '<p class="muted">No files yet.</p>';
      return;
    }
    $("#file-tree").innerHTML = query.trim()
      ? filterFiles(state.files, query)
          .map((f) => fileRow(f, 0, true))
          .join("") || '<p class="muted">No matching file.</p>'
      : renderDir(buildTree(state.files), 0);
  }

  async function openFile(path) {
    state.selected = path;
    renderTree();
    $("#file-preview").hidden = false;
    $("#preview-path").textContent = path;
    $("#preview-meta").textContent = "Loading…";
    $("#preview-body").innerHTML = "";
    if (deps.ui.view !== "project") deps.showView("project");
    try {
      const preview = await api("GET", `/api/files/preview?path=${encodeURIComponent(path)}`);
      $("#preview-meta").textContent = `${formatBytes(preview.bytes)} · read-only`;
      if (preview.text == null) {
        $("#preview-body").innerHTML = `<p class="muted preview-reason">Not shown: ${escapeHtml(preview.reason)}.</p>`;
        return;
      }
      state.previewText = preview.text;
      state.previewMarkdown = /\.(md|mdx|markdown)$/i.test(path);
      state.previewRaw = false;
      paintPreview();
    } catch (error) {
      $("#preview-meta").textContent = "";
      $("#preview-body").innerHTML = `<p class="muted preview-reason">${escapeHtml(error.message)}</p>`;
    }
  }
  const formatBytes = (n) => (n < 1024 ? `${n} B` : `${(n / 1024).toFixed(n < 10240 ? 1 : 0)} KB`);

  function renderHub() {
    const hub = state.hub;
    if (!hub) return;
    $("#hub-title").textContent = deps.ui.state?.project?.name || "Project";
    const nudges = hub.nudges;
    $("#nav-nudge-count").hidden = !nudges.length;
    $("#nav-nudge-count").textContent = nudges.length;
    $("#hub-nudges").innerHTML = nudges
      .map(
        (n) =>
          `<div class="nudge"><span class="nudge-icon">${icon("info", { size: 15 })}</span><span>${escapeHtml(n.text)}</span><button class="text-button" type="button" data-nudge="${escapeHtml(n.action)}">${nudgeLabel(n.action)} →</button></div>`,
      )
      .join("");
    const sections = hub.brief.sections;
    const done = sections.filter(([, ok]) => ok).length;
    $("#brief-progress").textContent = hub.brief.exists ? `${done}/${sections.length} filled` : "not started";
    $("#brief-sections").innerHTML = sections
      .map(
        ([name, ok]) =>
          `<li class="${ok ? "ok" : ""}"><span class="fill-dot" aria-hidden="true"></span>${escapeHtml(name)}</li>`,
      )
      .join("");
    $("#brief-actions").innerHTML = hub.brief.exists
      ? `<button class="secondary-button" type="button" data-action="open-brief">Open brief</button>`
      : `<button class="primary-button" type="button" data-action="write-brief">Write the brief</button>`;
    const label = { missing: "not created yet", "no-section": "no Verb section yet", outdated: "out of date", current: "up to date" };
    $("#agent-files").innerHTML = hub.agent_files
      .map(
        (f) =>
          `<li data-state="${f.state}"><code>${escapeHtml(f.name)}</code><span>${label[f.state]}</span></li>`,
      )
      .join("");
    // The next useful step leads: writing the brief first, then sharing it with the agents.
    const syncPending = hub.brief.exists && hub.agent_files.some((f) => f.state !== "current");
    const sync = document.querySelector('[data-action="sync-agents"]');
    sync.classList.toggle("primary-button", syncPending);
    sync.classList.toggle("secondary-button", !syncPending);
    $("#hub-documents").innerHTML = hub.documents.length
      ? hub.documents
          .map((d) => `<li><button class="text-button" type="button" data-file="${escapeHtml(d)}">${icon("file-text", { size: 14 })} ${escapeHtml(d.split("/").pop())}</button></li>`)
          .join("")
      : '<li class="muted">None yet. The brief will be the first.</li>';
  }
  const nudgeLabel = (action) =>
    ({ "write-brief": "Write it", "open-brief": "Open brief", "sync-agents": "Sync now", "open-spec": "Open specs" })[action] ?? "Go";

  // ---------------------------------------------------------------------------- actions

  function writeBrief() {
    $("#brief-dialog-form").reset();
    showDialog("brief-dialog");
    $("#brief-problem").focus();
  }
  /** Markdown reads formatted by default, with a Raw toggle; everything else shows its lines. */
  function paintPreview() {
    const formatted = state.previewMarkdown && !state.previewRaw;
    $("#preview-mode").hidden = !state.previewMarkdown;
    $("#preview-mode").textContent = formatted ? "Raw" : "Formatted";
    if (formatted) {
      $("#preview-body").innerHTML = `<div class="md-preview">${renderMarkdown(state.previewText, escapeHtml)}</div>`;
      return;
    }
    const lines = state.previewText.split("\n");
    if (lines.at(-1) === "") lines.pop();
    $("#preview-body").innerHTML = `<ol class="code-lines">${lines
      .map((line) => `<li><code>${escapeHtml(line) || " "}</code></li>`)
      .join("")}</ol>`;
  }
  document.addEventListener("click", (event) => {
    if (!event.target.closest('[data-action="preview-mode"]')) return;
    state.previewRaw = !state.previewRaw;
    paintPreview();
  });

  async function syncAgents() {
    try {
      const result = await api("POST", "/api/hub/sync");
      toast(`${result.message}${result.changed.length ? ". Recorded in the brief's audit trail." : ""}`);
      await Promise.all([refreshHub(), refreshFiles()]);
      document.dispatchEvent(new CustomEvent("verb:git-changed"));
    } catch (error) {
      toast(error.message, "error");
    }
  }
  const openBrief = () => openFile(state.hub?.brief.path ?? "docs/project/BRIEF.md");

  $("#brief-dialog-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const form = Object.fromEntries(new FormData(event.target));
    try {
      const result = await api("POST", "/api/hub/brief", form);
      closeDialog("brief-dialog");
      toast(`${result.message}. Sync agent context to share it with every agent.`);
      await Promise.all([refreshHub(), refreshFiles()]);
      document.dispatchEvent(new CustomEvent("verb:git-changed"));
      openFile(result.path);
    } catch (error) {
      toast(error.message, "error");
    }
  });

  $("#file-filter").addEventListener("input", renderTree);
  document.addEventListener("click", (event) => {
    const dir = event.target.closest("[data-dir]");
    if (dir) {
      const path = dir.dataset.dir;
      if (state.open.has(path)) state.open.delete(path);
      else state.open.add(path);
      return renderTree();
    }
    const file = event.target.closest("[data-file]");
    if (file) return openFile(file.dataset.file);
    const nudge = event.target.closest("[data-nudge]")?.dataset.nudge;
    const action = nudge ?? event.target.closest("[data-action]")?.dataset.action;
    const handlers = {
      "write-brief": writeBrief,
      "open-brief": openBrief,
      "sync-agents": syncAgents,
      "open-spec": () => deps.showView("specs"),
      "close-preview": () => {
        state.selected = null;
        $("#file-preview").hidden = true;
        renderTree();
      },
      "copy-file-path": async () => {
        try {
          await navigator.clipboard.writeText(state.selected);
          toast(`Copied ${state.selected}`);
        } catch {
          toast(state.selected);
        }
      },
    };
    if (handlers[action]) handlers[action]();
  });

  refreshHub();

  return {
    openFile,
    onShowView(view) {
      if (view !== "project") return;
      refreshHub();
      if (Date.now() - state.loadedAt > 3000) refreshFiles();
    },
    /** Commands for the palette: hub actions always; files when the query looks for one. */
    commands(query) {
      const list = [
        { title: "Write project brief", run: writeBrief },
        { title: "Open project brief", run: openBrief },
        { title: "Sync agent context (AGENTS.md, CLAUDE.md)", run: syncAgents },
      ];
      if (query.trim().length >= 2) {
        if (!state.files.length) refreshFiles();
        for (const file of filterFiles(state.files, query, 8)) {
          list.push({ title: `Open file ${file.path}`, hint: "file", run: () => openFile(file.path) });
        }
      }
      return list;
    },
  };
}
