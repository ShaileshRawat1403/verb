import { readWithBusyRetry } from "./read-retry.js";
import { icon, fileIconName } from "./icons.js";
// Typed previews project existing files and Git facts; they never own another work ledger.
export function initStudioPreviews({
  $,
  api,
  openFile,
  canClose,
  workspaceId,
  escapeHtml,
}) {
  const root = $("#preview-home"),
    tabs = [];
  let selected = null,
    request = 0,
    zoom = null;
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
      renderTabs();
      root.dataset.previewKind = $("#studio-canvas-panel").dataset.previewKind = isImage(path) ? "image" : "document";
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
        const result = await readWithBusyRetry(api,
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
    isImage,
  };
}
