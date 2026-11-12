import { readWithBusyRetry } from "./read-retry.js";
import { checksHtml } from "./view.js";

function proofState(record) {
  if (record?.result === "failed") return ["failed", "Failed check"];
  if (record?.freshness === "stale") return ["stale", "Stale proof"];
  if (record?.accepted) return ["accepted", "Accepted proof"];
  if (!record?.recorded) return ["unproven", "Unproven"];
  if (record.result === "passed") return ["pending", "Pass awaiting acceptance"];
  return ["unproven", "Inconclusive proof"];
}

const severity = ["failed", "stale", "unproven", "unknown", "pending", "accepted"];
const countLabel = (count, noun) => count ? count + " " + noun + (count === 1 || noun === "unproven" ? "" : "s") : "";

export function reviewVerdict(rows) {
  const observed = rows.filter(row => row.record?.observedCheck);
  if (!observed.length) return "";
  const failed = observed.filter(row => row.record.observedCheck.freshness === "current" && row.record.observedCheck.result === "failed");
  const stale = observed.filter(row => row.record.observedCheck.freshness !== "current");
  const inconclusive = observed.filter(row => row.record.observedCheck.freshness === "current" && row.record.observedCheck.result !== "passed" && row.record.observedCheck.result !== "failed");
  const unproven = rows.filter(row => !row.record?.accepted && !failed.includes(row));
  const reasons = [countLabel(failed.length, "failed check"), countLabel(unproven.length, "unproven"),
    countLabel(stale.length, "stale check"), countLabel(inconclusive.length, "inconclusive check")].filter(Boolean);
  return reasons.length ? "Not ready: " + reasons.join(", ") : "Ready to ship: all checks passed, all criteria proven";
}

export function initBenchReview({ $, api, workbench, escapeHtml: esc, showDialog, toast, refreshChecks }) {
  const body = $("#bench-review-body");
  let selected;
  let report;
  let diffRequest = 0;
  let diffSpec;
  const range = $("#bench-review-diff-range");
  let actionPending = false;

  const history = $("#bench-evidence-history");
  function render(spec = selected) {
    const sameSpec = selected?.id === spec?.id;
    if (!sameSpec) history.hidden = true;
    selected = spec;
    const evidence = spec ? workbench.summaryFor(spec)?.sdlc : null;
    const rows = (spec?.criteria ?? []).map(criterion => {
      const record = evidence?.criteria?.find(row => row.index === criterion.index);
      const [state, label] = evidence?.criteria ? proofState(record) : ["unknown", "Proof unavailable"];
      return { criterion, record, state, label };
    }).sort((left, right) => severity.indexOf(left.state) - severity.indexOf(right.state));
    const verdict = reviewVerdict(rows);
    const failed = rows.filter(row => row.record?.observedCheck?.result === "failed");
    const good = report?.lastKnownGood;
    const failedForHandoff = failed.some(row => row.record.observedCheck.freshness === "current")
      || rows.some(row => row.state === "failed" && row.record.freshness === "current");
    const projectChecks = `<section aria-labelledby="bench-checks-heading">
      <h3 id="bench-checks-heading">Checks</h3>
      ${report?.clear ? '<p class="muted">Project configuration checks passed.</p>'
        : report ? checksHtml({ ...report, lastKnownGood: null }) : '<p class="muted">Reading project checks…</p>'}
      <button class="text-button" type="button" data-action="refresh-checks">Check again</button>
      <button class="text-button" type="button" data-action="mark-good">Mark as good</button>
      ${spec ? '<button class="secondary-button" type="button" data-bench-review-action="run">Run a check</button>' : ""}
    </section>`;
    const markup = spec ? `
      ${verdict ? `<p class="bench-review-verdict" role="status">${esc(verdict)}</p>` : ""}
      <h2>${esc(spec.id)} · ${esc(spec.title)}</h2>
      <div class="bench-review-actions">
        <button class="primary-button" type="button" data-bench-review-action="handoff" data-role="${failedForHandoff ? "implement" : "review"}">${failedForHandoff ? "Hand fix to an agent" : "Hand off for review"}</button>
        <button class="secondary-button" type="button" data-bench-review-action="changes">Changes</button>
        <details class="bench-document-menu" data-review-menu="git"><summary aria-label="Git actions" title="Git actions">⋯</summary>
          <div class="bench-git-menu"><button class="text-button" type="button" data-bench-review-action="commit">Review commit</button>
            <button class="text-button" type="button" data-bench-review-action="transfer">Git transfer</button></div>
        </details>
      </div>
      ${projectChecks}
      ${failed.length ? `<ul class="bench-failed-checks">${failed.map(({ criterion, record }) =>
        `<li><strong>Failed check</strong> · ${esc(record.observedCheck.label || criterion.text)}${record.observedCheck.freshness !== "current" ? " · Stale" : ""}</li>`).join("")}</ul>`
        : `<p class="muted">${evidence?.criteria ? "No failed check recorded for this spec." : "Criterion checks unavailable."}</p>`}
      <details data-review-menu="evidence"><summary>Evidence actions</summary><ul class="bench-evidence-actions">
        <li><button class="text-button" type="button" data-bench-review-action="history">Evidence history</button></li>
        <li><button class="text-button" type="button" data-bench-review-action="export">Export evidence metadata</button></li>
        <li><button class="text-button" type="button" data-bench-review-action="plan">Record implementation plan</button></li>
        <li><button class="text-button" type="button" data-bench-review-action="review">Record implementation review</button></li>
        <li><button class="text-button" type="button" data-bench-review-action="remote">Check remote</button></li>
        <li><button class="text-button" type="button" data-bench-review-action="delivery">Record delivery report</button></li>
      </ul></details>
      <section aria-labelledby="bench-proof-heading">
        <h3 id="bench-proof-heading">Criterion proof</h3>
        <ul class="bench-proof-list">${rows.map(({ criterion, record, state, label }) => `<li>
          <small class="bench-criterion-number">Criterion ${criterion.index + 1}</small>
          <strong class="bench-proof-state" data-proof-state="${state}">${label}</strong>
          <div>${esc(criterion.text)}</div>
          ${criterion.done ? `<small>${record?.accepted ? "Completion claim recorded." : "Marked complete in the spec. Proof still needed."}</small>` : ""}
          <button class="text-button" type="button" data-bench-review-action="${criterion.done ? "reopen" : "claim"}" data-claim-index="${criterion.index}">${criterion.done ? "Reopen criterion" : "Record completion claim"}</button>
          ${record?.recorded ? `<p>${record.source === "observed" ? "Verb-observed" : record.source === "agent" ? "Agent-reported" : "Person-reported"} · ${esc(record.label || "")}</p>` : ""}
          <button class="text-button" type="button" data-bench-review-action="report" data-criterion="${criterion.index}">Record proof</button>
          ${state === "pending" ? `<button class="text-button" type="button" data-bench-review-action="accept" data-criterion="${criterion.index}" data-evidence="${esc(record.evidenceId)}">Accept evidence</button>` : ""}
        </li>`).join("")}</ul>
        ${!rows.length ? '<p class="muted">This spec has no criteria.</p>' : ""}
      </section>
      <section aria-labelledby="bench-review-git-heading">
        <h3 id="bench-review-git-heading">Changes since last known good</h3>
        ${good ? `<p>Marked ${esc(new Date(good.mark.markedAt).toLocaleString())}. ${good.distance.filesDiffer == null ? "File count unavailable." : `${good.distance.filesDiffer} files differ.`}</p>` : '<p class="muted">No last-known-good mark available.</p>'}
      </section>`
      : projectChecks + '<p class="muted">Select a spec to review its proof and changes.</p>';
    if (body._markup !== markup) {
      const openMenus = sameSpec
        ? new Set([...body.querySelectorAll("[data-review-menu][open]")].map(menu => menu.dataset.reviewMenu))
        : new Set();
      body.innerHTML = markup;
      body._markup = markup;
      for (const menu of body.querySelectorAll("[data-review-menu]")) {
        menu.open = openMenus.has(menu.dataset.reviewMenu);
      }
    }
  }

  async function readDiff() {
    if (!diffSpec) return;
    const path = range.value === "good" ? "/api/good/diff" : `/api/specs/${diffSpec}/sdlc-diff`;
    const current = ++diffRequest;
    const dialog = $("#bench-review-diff");
    const scope = $("#bench-review-diff-scope");
    const content = dialog.querySelector("pre");
    scope.textContent = "Reading Git changes…";
    content.textContent = "";
    try {
      const result = await readWithBusyRetry(api, path);
      if (current !== diffRequest || !dialog.open) return;
      scope.textContent = result.scope + (result.truncated ? " Diff truncated." : "");
      content.textContent = result.diff || "No changes in this comparison.";
    } catch (error) {
      if (current === diffRequest) scope.textContent = error.message;
    }
  }

  range.addEventListener("change", readDiff);
  function openDiff(id) {
    diffSpec = id;
    range.value = report?.lastKnownGood?.mark.head && !report.lastKnownGood.distance.markMissing ? "good" : "implementation";
    showDialog("bench-review-diff");
    return readDiff();
  }

  body.addEventListener("click", async event => {
    const button = event.target.closest("[data-bench-review-action]");
    if (!button || !selected || actionPending) return;
    const id = selected.id;
    const action = button.dataset.benchReviewAction;
    if (action === "changes") return openDiff(id);
    const menu = button.closest("details");
    if (menu) menu.open = false;
    workbench.selectSpec(id, { navigate: false });
    if (action === "commit") return workbench.openCommit();
    if (action === "transfer") return workbench.openTransfer();
    if (action === "handoff") return workbench.openHandoff({ reviewed: true, role: button.dataset.role });
    actionPending = true;
    button.disabled = true;
    try {
      if (action === "run") await workbench.runCheck();
      else if (action === "history") {
        history.hidden = !history.hidden;
        if (!history.hidden) {
          await workbench.evidenceHistory();
          if (selected?.id === id) history.scrollIntoView({ block: "nearest" });
        }
      } else if (action === "export") await workbench.exportEvidence();
      else if (action === "reopen") await workbench.reopenCriterion(Number(button.dataset.claimIndex));
      else if (action === "report" || action === "accept") await workbench.openProof(action, Number(button.dataset.criterion), button.dataset.evidence);
      else await workbench.openLifecycle(action, Number(button.dataset.claimIndex));
    } catch (error) {
      toast(error.message, "error");
    } finally {
      actionPending = false;
      button.disabled = false;
    }
  });

  return { render, refreshChecks, setChecks(value) { report = value; render(); } };
}
