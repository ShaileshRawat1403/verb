// Service observations describe supervisor visibility, not application availability.
export function hostServicesHtml(services, { escapeHtml, formatDuration }) {
  if (!Array.isArray(services)) {
    return '<p class="muted">Service supervision is not observed here. Verb is running, but this page cannot establish supervisor status.</p>';
  }
  if (!services.length)
    return '<p class="muted">No supervised services were reported.</p>';
  const tone = (state) =>
    ["not supervised", "unknown"].includes(state)
      ? "idle"
      : state.startsWith("run")
        ? state.includes("stopping")
          ? "warn"
          : "ok"
        : "bad";
  const rows = services
    .map((service) => {
      const state =
        typeof service.state === "string" ? service.state : "unknown";
      return `<tr><td><code>${escapeHtml(service.name)}</code></td><td><span class="state-dot" data-state="${tone(state)}"></span>${escapeHtml(state === "unknown" ? "Supervisor status unavailable" : state)}</td><td>${formatDuration(service.uptime_secs)}</td></tr>`;
    })
    .join("");
  const unreadable = services.some(
    (service) =>
      service.state === "unknown" || typeof service.state !== "string",
  );
  return `<table class="host-table"><thead><tr><th>Service</th><th>Supervisor state</th><th>Up for</th></tr></thead><tbody>${rows}</tbody></table>${unreadable ? '<p class="muted">Verb could not read some supervisor states. Access may be restricted for this account; this does not mean those services are stopped. Check service availability in your node’s operations console.</p>' : ""}`;
}
