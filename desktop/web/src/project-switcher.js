const loopback = new Set(["localhost", "127.0.0.1", "[::1]"]);

export function projectDestination(value, currentOrigin) {
  const trimmed = value.trim();
  if (!/^https?:\/\//i.test(trimmed)) return null;
  let url;
  try {
    url = new URL(trimmed);
  } catch {
    return null;
  }
  const allowed = url.protocol === "https:" ||
    (url.protocol === "http:" && loopback.has(url.hostname));
  if (
    !allowed || url.username || url.password || url.pathname !== "/" ||
    url.search || url.origin === currentOrigin
  ) {
    return null;
  }
  return url.href;
}

export function initProjectSwitcher({ $, showDialog, closeDialog }) {
  const dialog = $("#project-switch-dialog");
  const input = $("#project-switch-url");
  const error = $("#project-switch-error");
  const reset = () => {
    input.value = "";
    input.removeAttribute("aria-invalid");
    error.hidden = true;
  };
  $("#open-project-switch").addEventListener("click", () => {
    reset();
    showDialog(dialog.id);
    input.focus();
  });
  dialog.addEventListener("close", reset);
  $("#project-switch-form").addEventListener("submit", (event) => {
    event.preventDefault();
    const destination = projectDestination(input.value, window.location.origin);
    if (!destination) {
      input.setAttribute("aria-invalid", "true");
      error.hidden = false;
      input.focus();
      return;
    }
    window.open(destination, "_blank", "noopener,noreferrer");
    reset();
    closeDialog(dialog.id);
  });
}
