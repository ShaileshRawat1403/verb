// The Host view: read-only health of the machine Verb runs on. Data from GET /api/host.

import { formatDuration, formatKb, temperatureTone, usedPercent } from "./workbench.js";

export function initHost(deps) {
  const { api, escapeHtml, $ } = deps;
  let timer = null;

  const fact = (label, value, title = "") =>
    value == null || value === ""
      ? ""
      : `<dt>${escapeHtml(label)}</dt><dd${title ? ` title="${escapeHtml(title)}"` : ""}>${escapeHtml(String(value))}</dd>`;

  function usageBar(usage, unavailable) {
    if (!usage) return `<p class="muted">${escapeHtml(unavailable)}</p>`;
    const pct = usedPercent(usage);
    const tone = pct >= 90 ? "hot" : pct >= 75 ? "warm" : "ok";
    return `<div class="usage-number">${pct}%<small> used</small></div><div class="usage-bar" data-tone="${tone}"><span style="width:${pct}%"></span></div><p class="muted">${formatKb(usage.available_kb)} free of ${formatKb(usage.total_kb)}</p>`;
  }

  const serviceTone = (state) =>
    state === "not supervised"
      ? "idle"
      : state.startsWith("run")
        ? state.includes("stopping")
          ? "warn"
          : "ok"
        : "bad";

  function render(r) {
    const v = r.verb;
    $("#host-title").textContent = r.machine.model || "This machine";
    $("#host-updated").textContent = `Updated ${new Date().toLocaleTimeString()}`;
    $("#host-notes").innerHTML = r.notes.map((n) => `<div class="host-note">ⓘ ${escapeHtml(n)}</div>`).join("");
    $("#host-verb").innerHTML =
      fact("Version", v.version) +
      fact("Commit", v.commit ?? "not from a recorded deployment") +
      fact("Running for", formatDuration(v.uptime_secs), v.started ? `since ${v.started}` : "") +
      fact("Memory used", v.rss_kb != null ? formatKb(v.rss_kb) : null) +
      fact("Binary", v.binary_sha256 ? `${v.binary_sha256.slice(0, 12)}…` : null, v.binary_sha256 ?? "");
    $("#host-machine").innerHTML =
      fact("Model", r.machine.model) +
      fact("System", `${r.machine.os} · ${r.machine.arch}`) +
      fact("Container", r.machine.container);
    $("#host-memory").innerHTML = usageBar(r.memory, "Not readable on this machine.");
    $("#host-storage").innerHTML = usageBar(r.storage, "Not readable on this machine.");
    $("#host-temps").innerHTML = r.temperatures.length
      ? r.temperatures
          .map(
            (t) =>
              `<div class="temp" data-tone="${temperatureTone(t.celsius)}"><span>${escapeHtml(t.name)}</span><b>${t.celsius.toFixed(1)}°C</b></div>`,
          )
          .join("")
      : '<p class="muted">No temperature sensors are readable here.</p>';
    const b = r.battery;
    const batteryTemp = r.temperatures.find((t) => t.name === "Battery");
    $("#host-battery").innerHTML =
      b.level != null
        ? `<div class="usage-number">${b.level}%<small>${b.charging ? " charging" : ""}</small></div><div class="usage-bar" data-tone="${b.level < 20 ? "hot" : "ok"}"><span style="width:${b.level}%"></span></div>`
        : `${batteryTemp ? `<div class="usage-number">${batteryTemp.celsius.toFixed(1)}°C<small> battery temperature</small></div>` : ""}<p class="muted">Charge level unavailable. ${escapeHtml(b.reason ?? "")}</p>`;
    $("#host-services").innerHTML = r.services
      ? `<table class="host-table"><thead><tr><th>Service</th><th>State</th><th>Up for</th></tr></thead><tbody>${r.services
          .map(
            (s) =>
              `<tr><td><code>${escapeHtml(s.name)}</code></td><td><span class="state-dot" data-state="${serviceTone(s.state)}"></span>${escapeHtml(s.state)}</td><td>${formatDuration(s.uptime_secs)}</td></tr>`,
          )
          .join("")}</tbody></table>`
      : '<p class="muted">No service supervisor detected. Verb is running as an ordinary process here.</p>';
    $("#host-events").innerHTML = r.events.length
      ? r.events
          .map(
            (e) =>
              `<li data-kind="${e.kind}"><time>${escapeHtml(e.at)} UTC</time><span>${escapeHtml(e.detail)}</span></li>`,
          )
          .join("")
      : '<li class="muted">No restart history available on this machine.</li>';
    const crashedRecently = r.events.some(
      (e) => e.kind === "crash" && Date.now() - Date.parse(`${e.at.replace(" ", "T")}Z`) < 86400000,
    );
    const hot = r.temperatures.some((t) => temperatureTone(t.celsius) === "hot");
    $("#nav-host-alert").hidden = !(crashedRecently || hot);
    $("#nav-host-alert").title = crashedRecently ? "Verb crashed in the last 24 hours" : "Running hot";
  }

  async function refresh() {
    try {
      render(await api("GET", "/api/host"));
    } catch (error) {
      $("#host-updated").textContent = `Could not read host: ${error.message}`;
    }
  }

  refresh();
  return {
    onShowView(view) {
      clearInterval(timer);
      if (view !== "host") return;
      refresh();
      timer = setInterval(() => !document.hidden && refresh(), 10000);
    },
  };
}
