import test from "node:test";
import assert from "node:assert/strict";
import { hostServicesHtml } from "../src/host-services.js";
import { escapeHtml } from "../src/view.js";
import { formatDuration } from "../src/workbench.js";
const render = (services) =>
  hostServicesHtml(services, { escapeHtml, formatDuration });
test("unknown supervisor observations explain access scope without claiming an outage", () => {
  const html = render([{ name: "verb", state: "unknown", uptime_secs: null }]);
  assert.match(html, /Supervisor status unavailable/);
  assert.match(html, /Access may be restricted for this account/);
  assert.match(html, /does not mean those services are stopped/);
  assert.match(html, /data-state="idle"/);
  assert.doesNotMatch(html, /data-state="bad"/);
});
test("absent observations do not claim that no supervisor exists", () => {
  assert.match(render(null), /cannot establish supervisor status/);
  assert.doesNotMatch(
    render(null),
    /ordinary process|No service supervisor detected/,
  );
  assert.match(render([]), /No supervised services were reported/);
});
test("observed running, stopping and down remain distinct and escaped", () => {
  const html = render([
    { name: "<script>", state: "run", uptime_secs: 60 },
    { name: "second", state: "run (stopping)" },
    { name: "third", state: "down" },
  ]);
  assert.match(html, /data-state="ok"/);
  assert.match(html, /data-state="warn"/);
  assert.match(html, /data-state="bad"/);
  assert.match(html, /&lt;script&gt;/);
  assert.doesNotMatch(html, /<script>/);
  assert.doesNotMatch(html, /Access may be restricted/);
});
