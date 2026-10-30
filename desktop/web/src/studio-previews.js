import { icon, fileIconName } from "./icons.js";
// Typed previews project existing files and Git facts; they never own another work ledger.
export function initStudioPreviews({
  $,
  api,
  openFile,
  canClose,
  workspaceId,
  stopApp,
  escapeHtml,
}) {
  const root = $("#view-studio"),
    tabs = [];
  let selected = null,
    request = 0,
    zoom = null,
    share = 46;
  const message = (text) => ($("#studio-message").textContent = text);
  const isImage = (path) => /\.(png|jpe?g|gif|webp)$/i.test(path || "");
  function renderTabs() {
    $("#studio-preview-tabs").replaceChildren(
      ...tabs.map((path, index) => {
        const tab = document.createElement("button");
        tab.id = `studio-preview-tab-${index}`;
        tab.type = "button";
        tab.className = "studio-preview-tab";
        tab.setAttribute("role", "tab");
        tab.setAttribute("aria-selected", String(path === selected));
        tab.setAttribute("aria-controls", "studio-preview-content");
        tab.setAttribute("aria-keyshortcuts", "Delete");
        tab.tabIndex = path === selected ? 0 : -1;
        tab.title = path;
        tab.innerHTML = `${icon(fileIconName(path), { size: 14 })}<span>${escapeHtml(path.split("/").pop())}</span>`;
        tab.addEventListener("click", () => openFile(path));
        tab.addEventListener("keydown", (event) => {
          if (event.key === "Delete") {
            event.preventDefault();
            close();
            return;
          }
          const next =
            event.key === "ArrowRight"
              ? (index + 1) % tabs.length
              : event.key === "ArrowLeft"
                ? (index + tabs.length - 1) % tabs.length
                : event.key === "Home"
                  ? 0
                  : event.key === "End"
                    ? tabs.length - 1
                    : -1;
          if (next >= 0) {
            event.preventDefault();
            openFile(tabs[next]);
            $("#studio-preview-tabs")
              .querySelectorAll('[role="tab"]')
              [next]?.focus();
          }
        });
        return tab;
      }),
    );
    const active = tabs.indexOf(selected);
    if (active >= 0)
      $("#studio-preview-content").setAttribute(
        "aria-labelledby",
        `studio-preview-tab-${active}`,
      );
    else $("#studio-preview-content").removeAttribute("aria-labelledby");
    $("#studio-close-preview").disabled = !selected;
    $("#studio-preview-tabs").setAttribute(
      "aria-label",
      `Open previews · ${tabs.length} of 8`,
    );
  }
  function close() {
    if (!selected) return;
    if (!canClose(selected)) {
      message(
        "Save or resolve this file's draft before closing its preview. Your text is retained.",
      );
      return;
    }
    const index = tabs.indexOf(selected);
    tabs.splice(index, 1);
    selected = null;
    openFile(tabs[Math.min(index, tabs.length - 1)] || null);
  }
  function size() {
    const image = $("#studio-preview-image");
    image.style.width =
      zoom == null ? "auto" : `${(image.naturalWidth * zoom) / 100}px`;
    image.style.maxWidth = zoom == null ? "100%" : "none";
    image.style.maxHeight = zoom == null ? "100%" : "none";
    $("#studio-image-scale").textContent = zoom == null ? "Fit" : `${zoom}%`;
  }
  $("#studio-image-fit").addEventListener("click", () => {
    zoom = null;
    size();
  });
  $("#studio-image-plus").addEventListener("click", () => {
    zoom = Math.min(300, (zoom ?? 100) + 25);
    size();
  });
  $("#studio-image-minus").addEventListener("click", () => {
    zoom = Math.max(25, (zoom ?? 100) - 25);
    size();
  });
  $("#studio-close-preview").addEventListener("click", close);
  const splitter = $("#studio-pane-divider");
  function split(value) {
    share = Math.max(30, Math.min(65, value));
    $(".studio-layout").style.setProperty("--studio-work-share", `${share}%`);
    splitter.setAttribute("aria-valuenow", String(Math.round(share)));
    splitter.setAttribute(
      "aria-valuetext",
      `Work thread ${Math.round(share)} percent`,
    );
  }
  splitter.addEventListener("keydown", (event) => {
    const delta = event.shiftKey ? 10 : 2;
    const value =
      event.key === "ArrowLeft"
        ? share - delta
        : event.key === "ArrowRight"
          ? share + delta
          : event.key === "Home"
            ? 30
            : event.key === "End"
              ? 65
              : null;
    if (value != null) {
      event.preventDefault();
      split(value);
    }
  });
  splitter.addEventListener("pointerdown", (event) => {
    if (event.button !== 0) return;
    event.preventDefault();
    splitter.setPointerCapture(event.pointerId);
    splitter.onpointermove = (move) => {
      const rect = $(".studio-layout").getBoundingClientRect();
      split(((move.clientX - rect.left) / rect.width) * 100);
    };
    splitter.onpointerup = splitter.onlostpointercapture = () => {
      splitter.onpointermove = null;
    };
  });
  function paneLayout(workHidden, canvasHidden) {
    // Always retain one surface and its external restore controls. Never unmount drafts or apps.
    root.dataset.previewExpanded = String(workHidden);
    root.dataset.workExpanded = String(canvasHidden);
    for (const [id, hidden, label] of [
      ["work", workHidden, "current work"],
      ["canvas", canvasHidden, "preview"],
    ]) {
      const button = $(`#studio-toggle-${id}`);
      button.setAttribute("aria-expanded", String(!hidden));
      button.textContent = `${hidden ? "Show" : "Hide"} ${label}`;
    }
    $("#studio-expand-preview").setAttribute(
      "aria-pressed",
      String(workHidden),
    );
    $("#studio-expand-preview").textContent = workHidden
      ? "Restore split"
      : "Expand preview";
  }
  function expand(expanded) {
    paneLayout(expanded, false);
  }
  $("#studio-expand-preview").addEventListener("click", () =>
    expand(root.dataset.previewExpanded !== "true"),
  );
  $("#studio-toggle-work").addEventListener("click", () =>
    paneLayout(root.dataset.previewExpanded !== "true", false),
  );
  $("#studio-toggle-canvas").addEventListener("click", () =>
    paneLayout(false, root.dataset.workExpanded !== "true"),
  );
  $("#studio-reset-layout").addEventListener("click", () => {
    split(46);
    paneLayout(false, false);
  });
  root.addEventListener("keydown", (event) => {
    // Escape inside a dialog/editor belongs to that control. Restore only from the layout controls.
    if (
      event.key === "Escape" &&
      (event.target.closest(".studio-pane-controls") ||
        event.target === $("#studio-expand-preview"))
    ) {
      paneLayout(false, false);
      $("#studio-toggle-work").focus();
    }
  });
  function showChanges(show) {
    ++request;
    $("#studio-preview-content").hidden = show;
    $("#studio-changes-panel").hidden = !show;
    $("#studio-toggle-changes").setAttribute("aria-pressed", String(show));
    $("#studio-toggle-changes").textContent = show
      ? "Back to preview"
      : "Show changes";
    if (show) stopApp();
  }
  async function diff(path) {
    const generation = ++request;
    $("#studio-diff-path").textContent = path;
    $("#studio-diff").textContent = "Reading changes…";
    try {
      const result = await api(
        "GET",
        `/api/git/diff?path=${encodeURIComponent(path)}`,
      );
      if (generation !== request) return;
      const rows = (result.hunks || [])
        .map(
          (hunk) =>
            `<div class="studio-diff-hunk">${escapeHtml(hunk.header)}</div>` +
            hunk.lines
              .map(
                (line) =>
                  `<div class="studio-diff-line ${line.kind === "add" ? "add" : line.kind === "del" ? "del" : "context"}"><span>${line.kind === "add" ? "+" : line.kind === "del" ? "−" : " "}</span><code>${escapeHtml(line.text) || " "}</code></div>`,
              )
              .join(""),
        )
        .join("");
      $("#studio-diff").innerHTML =
        `${result.reason ? `<p>${escapeHtml(result.reason)}</p>` : ""}${rows || "<p>No textual diff available.</p>"}`;
    } catch (error) {
      if (generation === request) $("#studio-diff").textContent = error.message;
    }
  }
  $("#studio-toggle-changes").addEventListener("click", async () => {
    const show = $("#studio-changes-panel").hidden;
    showChanges(show);
    if (!show) return;
    const generation = request;
    $("#studio-changed-files").textContent = "Reading changed files…";
    $("#studio-diff").textContent = "Select a file to review its changes.";
    try {
      const summary = await api("GET", "/api/git");
      if (generation !== request) return;
      if (summary.error) throw new Error(summary.error);
      const changes = summary.changes || [];
      $("#studio-change-count").textContent =
        `${changes.length} changed file${changes.length === 1 ? "" : "s"} · checkout state`;
      $("#studio-changed-files").replaceChildren(
        ...changes.map((change) => {
          const button = document.createElement("button");
          button.type = "button";
          button.className = "studio-change-file";
          button.textContent = `${change.code}  ${change.path}`;
          button.addEventListener("click", () => diff(change.path));
          return button;
        }),
      );
      if (!changes.length)
        $("#studio-diff").textContent =
          "No uncommitted changes in this checkout.";
    } catch (error) {
      if (generation === request)
        $("#studio-changed-files").textContent =
          `Changes unavailable: ${error.message}`;
    }
  });
  return {
    open(path) {
      if (path && !tabs.includes(path)) {
        if (tabs.length >= 8) {
          message(
            "Eight previews are open. Close one before opening another file.",
          );
          return false;
        }
        tabs.push(path);
      }
      selected = path;
      showChanges(false);
      renderTabs();
      root.dataset.previewKind = isImage(path) ? "image" : "document";
      $("#studio-show-document").textContent = isImage(path)
        ? "Image"
        : "Document";
      $("#studio-image-panel").hidden = !isImage(path);
      $("#studio-preview-image").removeAttribute("src");
      $("#studio-image-download").hidden = true;
      return true;
    },
    async image(path, current) {
      const generation = ++request;
      $("#studio-image-state").textContent = "Reading image…";
      try {
        const result = await api(
          "GET",
          `/api/studio/image?path=${encodeURIComponent(path)}`,
        );
        if (generation !== request || !current()) return;
        if (workspaceId?.() && result.workspaceId !== workspaceId())
          throw new Error("Workspace changed; reopen the image.");
        const image = result.image;
        if (
          image.path !== path ||
          !/^image\/(png|jpeg|gif|webp)$/.test(image.mime) ||
          image.bytes > 2097152 ||
          image.data.length > 2796204 ||
          !/^[A-Za-z0-9+/]*={0,2}$/.test(image.data)
        )
          throw new Error("Image identity could not be verified.");
        const element = $("#studio-preview-image");
        element.alt = path;
        element.onload = () => {
          if (generation === request)
            $("#studio-image-state").textContent =
              `${element.naturalWidth} × ${element.naturalHeight} · ${(image.bytes / 1024).toFixed(1)} KB · ${image.revision.slice(0, 8)}`;
        };
        element.onerror = () => {
          if (generation === request)
            $("#studio-image-state").textContent =
              "This image could not be decoded. The original file is unchanged.";
        };
        element.src = `data:${image.mime};base64,${image.data}`;
        const link = $("#studio-image-download");
        link.href = element.src;
        link.download = path.split("/").pop();
        link.hidden = false;
        zoom = null;
        size();
      } catch (error) {
        if (generation === request)
          $("#studio-image-state").textContent =
            `Image unavailable: ${error.message}`;
      }
    },
    reveal(surface) {
      if (
        (surface === "canvas" && root.dataset.workExpanded === "true") ||
        (surface === "work" && root.dataset.previewExpanded === "true")
      )
        paneLayout(false, false);
    },
    isImage,
  };
}
