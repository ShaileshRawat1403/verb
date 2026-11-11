import { readWithBusyRetry } from "./read-retry.js";

export function initBenchTerminals({ $, api, ui, shell, workbench, showDialog, closeDialog, refreshState, toast, toggleSplit }) {
  const noteOffers = new Set();
  const savedNotes = new Set();
  const focused = () => ui.state?.sessions.find(session => session.id === ui.focusedTerminal);
  const owner = () => ui.accessScope === "owner";

  function render() {
    const strip = $("#bench-terminal-context");
    const session = focused();
    strip.hidden = !shell.enabled() || ui.view !== "sessions" || !session;
    if (strip.hidden) return;
    const terminal = ui.terminals.get(session.id);
    const ended = terminal?.ended || session.state === "ended";
    const delivered = session.contextDelivery;
    const assignment = session.specId ? `Spec ${session.specId}`
      : session.role === "plan" ? `Work item ${session.workItemId}` : "Scratch";
    const deliveryLabel = delivered && delivered.specId !== session.specId ? `Delivered for spec ${delivered.specId}` : "Delivered";
    $("#bench-terminal-context-summary").textContent = delivered
      ? `${assignment} · ${deliveryLabel} · ${delivered.requirements.join(", ") || "no requirement IDs"} · ${delivered.noteCount} ${delivered.noteCount === 1 ? "note" : "notes"}`
      : `${assignment} · No recorded context delivery`;
    $("#bench-view-delivery").hidden = !owner() || !delivered;
    $("#bench-resend-context").hidden = !owner() || ended || !session.specId || !terminal || session.agent === "shell";
    $("#bench-handoff").hidden = !owner() || !session.specId;
    $("#bench-offer-note").hidden = !owner() || !ended || !session.specId || savedNotes.has(session.id);
    $("[data-bench-assign]").hidden = !owner() || !terminal || ended || session.role === "plan";
    $("[data-bench-split]").hidden = ui.terminals.size < 2 || matchMedia("(max-width: 760px)").matches;
    $("[data-bench-split]").textContent = ui.splitTerminals ? "Show one terminal" : "Show side by side";
  }

  function options(select, selected) {
    select.replaceChildren(new Option("Scratch", ""));
    for (const spec of workbench.current().specs) select.add(new Option(`${spec.id} · ${spec.title}`, spec.id));
    select.value = selected ?? "";
  }

  function openLaunch(specId = "") {
    const form = $("#session-form");
    $("#bench-launch-assignment").hidden = !shell.enabled();
    options(form.elements.specId, specId);
    form.elements.role.disabled = !form.elements.specId.value;
    $("#session-launch-error").hidden = true;
    showDialog("session-dialog");
  }

  $("#session-form").elements.specId.addEventListener("change", event => {
    $("#session-form").elements.role.disabled = !event.target.value;
  });

  let review = null;
  async function contextDialog(preview) {
    const session = focused();
    if (!session) return;
    try {
      const suffix = preview ? "context-preview" : "delivered-context";
      const result = await readWithBusyRetry(api, `/api/terminals/${session.id}/${suffix}`);
      if (!result.delivery) return;
      review = preview ? { id: session.id, ...result } : null;
      $("#bench-context-title").textContent = preview ? "Review context before pasting" : "Delivered context";
      $("#bench-context-detail").textContent = preview
        ? "Verb pastes this text into the agent’s input. Submit it from the terminal when ready."
        : `${result.delivery.trigger} · ${result.delivery.at} · ${result.delivery.bytes} bytes`;
      $("#bench-context-text").textContent = result.text;
      $("#bench-context-error").hidden = true;
      $("#bench-context-dialog button[type=submit]").hidden = !preview;
      showDialog("bench-context-dialog");
    } catch (error) {
      toast(error.message, "error");
    }
  }

  function submit(dialog, action) {
    const form = $(`#${dialog} form`);
    form.addEventListener("submit", async event => {
      event.preventDefault();
      const button = form.querySelector("button[type=submit]");
      if (button.disabled) return;
      const error = form.querySelector("[role=alert]");
      error.hidden = true;
      button.disabled = true;
      try {
        await action(form);
        closeDialog(dialog);
        await refreshState(true);
        render();
      } catch (failure) {
        error.textContent = failure.message;
        error.hidden = false;
      } finally {
        button.disabled = false;
      }
    });
  }

  submit("bench-context-dialog", async () => {
    if (!review) return;
    const { id, delivery, deliveryCount } = review;
    const result = await api("POST", `/api/terminals/${id}/resend-context`, {
      hash: delivery.hash, specRevision: delivery.specRevision, deliveryCount,
    });
    review = null;
    if (result.warning) toast(`Context pasted. Could not save its record: ${result.warning}`, "error");
  });

  async function offerNote(id, automatic = false) {
    if (!shell.enabled() || !owner() || !id || savedNotes.has(id) || (automatic && noteOffers.has(id))) return;
    if (!ui.state?.sessions.some(session => session.id === id && session.specId)) return;
    const dialog = $("#bench-note-dialog");
    if (dialog.open) return;
    noteOffers.add(id);
    try {
      const preview = await readWithBusyRetry(api, `/api/terminals/${id}/session-note`);
      const form = dialog.querySelector("form");
      form.reset();
      form.dataset.id = id;
      form.elements.files.value = preview.files ?? "";
      $("#bench-note-detail").textContent = preview.files == null
        ? `Spec ${preview.specId} · Git comparison unavailable. Files you enter are reported and unverified.`
        : `Spec ${preview.specId} · Verb prefilled files from Git. Edits to that list are reported and unverified.`;
      $("#bench-note-error").hidden = true;
      showDialog("bench-note-dialog");
    } catch (error) {
      toast(error.message, "error");
    }
  }

  submit("bench-note-dialog", async form => {
    const id = form.dataset.id;
    const body = Object.fromEntries(new FormData(form));
    await api("POST", `/api/terminals/${id}/session-note`, body);
    savedNotes.add(id);
    toast("Session note saved.");
  });

  submit("bench-binding-dialog", async form => {
    const specId = form.elements.specId.value || null;
    const role = specId ? form.elements.role.value || null : null;
    await api("POST", `/api/terminals/${form.dataset.id}/binding`, { specId, role });
  });

  $("[data-bench-assign]").addEventListener("click", () => {
    const session = focused();
    if (!session) return;
    $("#bench-terminal-menu").open = false;
    const form = $("#bench-binding-dialog form");
    options(form.elements.specId, session.specId);
    form.elements.role.value = session.role ?? "";
    form.dataset.id = session.id;
    $("#bench-binding-error").hidden = true;
    showDialog("bench-binding-dialog");
  });
  $("[data-bench-split]").addEventListener("click", () => {
    $("#bench-terminal-menu").open = false;
    toggleSplit();
  });
  $("#bench-view-delivery").addEventListener("click", () => contextDialog(false));
  $("#bench-resend-context").addEventListener("click", () => contextDialog(true));
  $("#bench-handoff").addEventListener("click", () => {
    const id = focused()?.specId;
    if (!id) return;
    workbench.selectSpec(id, { navigate: false });
    workbench.openHandoff({ reviewed: true });
  });
  $("#bench-offer-note").addEventListener("click", () => offerNote(focused()?.id));
  return { render, openLaunch, offerNote };
}
