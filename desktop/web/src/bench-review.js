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

export function initBenchReview({ $, api, workbench, escapeHtml: esc, showDialog, toast, refreshChecks }) {
  const body = $("#bench-review-body");
  let selected;
  let report;
  let diffRequest = 0;
  let actionPending = false;

  function render(spec = selected) {
    selected = spec;
    const evidence = spec ? workbench.summaryFor(spec)?.sdlc : null;
    const rows = (spec?.criteria ?? []).map(criterion => {
      const record = evidence?.criteria?.find(row => row.index === criterion.index);
      const [state, label] = evidence?.criteria ? proofState(record) : ["unknown", "Proof unavailable"];
      return { criterion, record, state, label };
    }).sort((left, right) => Number(right.state === "failed") - Number(left.state === "failed"));
    const failed = rows.filter(row => row.state === "failed");
    const good = report?.lastKnownGood;
    const comparison = good?.mark.head && !good.distance.markMissing;
    const markup = spec ? `
      <h2>${esc(spec.id)} · ${esc(spec.title)}</h2>
      <section aria-labelledby="bench-checks-heading">
        <h3 id="bench-checks-heading">Checks</h3>
        ${failed.length ? `<ul class="bench-failed-checks">${failed.map(({ criterion, record }) =>
          `<li><strong>Failed check</strong> · ${esc(record.label || criterion.text)}${record.freshness === "stale" ? " · Stale" : ""}</li>`).join("")}</ul>`
          : `<p class="muted">${evidence?.criteria ? "No failed check recorded for this spec." : "Criterion checks unavailable."}</p>`}
        ${report?.clear ? '<p class="muted">Project configuration checks passed.</p>'
          : report ? checksHtml({ ...report, lastKnownGood: null }) : '<p class="muted">Reading project checks…</p>'}
        <button class="secondary-button" type="button" data-bench-review-action="run">Run a check</button>
      </section>
      <section aria-labelledby="bench-proof-heading">
        <h3 id="bench-proof-heading">Criterion proof</h3>
        <ol class="bench-proof-list">${rows.map(({ criterion, record, state, label }) => `<li value="${criterion.index + 1}">
          <strong class="bench-proof-state" data-proof-state="${state}">${label}</strong>
          <div>${esc(criterion.text)}</div>
          ${criterion.done && !record?.accepted ? '<small>Marked complete in the spec. Proof still needed.</small>' : ""}
          ${record?.recorded ? `<p>${record.source === "observed" ? "Verb-observed" : record.source === "agent" ? "Agent-reported" : "Person-reported"} · ${esc(record.label || "")}</p>` : ""}
          <button class="text-button" type="button" data-bench-review-action="report" data-criterion="${criterion.index}">Record proof</button>
          ${state === "pending" ? `<button class="text-button" type="button" data-bench-review-action="accept" data-criterion="${criterion.index}" data-evidence="${esc(record.evidenceId)}">Accept evidence</button>` : ""}
        </li>`).join("")}</ol>
        ${!rows.length ? '<p class="muted">This spec has no criteria.</p>' : ""}
      </section>
      <section aria-labelledby="bench-review-git-heading">
        <h3 id="bench-review-git-heading">Changes since last known good</h3>
        ${good ? `<p>Marked ${esc(new Date(good.mark.markedAt).toLocaleString())}. ${good.distance.filesDiffer == null ? "File count unavailable." : `${good.distance.filesDiffer} files differ.`}</p>` : '<p class="muted">No last-known-good mark available.</p>'}
        <div class="bench-review-actions">
          ${comparison ? '<button class="secondary-button" type="button" data-bench-review-action="good-diff">View diff since mark</button>' : ""}
          <button class="secondary-button" type="button" data-bench-review-action="diff">Inspect implementation diff</button>
          <button class="secondary-button" type="button" data-bench-review-action="commit">Review commit</button>
          <button class="secondary-button" type="button" data-bench-review-action="transfer">Git transfer</button>
        </div>
      </section>
      <button class="primary-button" type="button" data-bench-review-action="handoff">Review handoff</button>`
      : '<p class="muted">Select a spec to review its proof and changes.</p>';
    if (body._markup !== markup) {
      body.innerHTML = markup;
      body._markup = markup;
    }
  }

  async function openDiff(path) {
    const current = ++diffRequest;
    const dialog = $("#bench-review-diff");
    const scope = $("#bench-review-diff-scope");
    const content = dialog.querySelector("pre");
    scope.textContent = "Reading Git changes…";
    content.textContent = "";
    showDialog(dialog.id);
    try {
      const result = await readWithBusyRetry(api, path);
      if (current !== diffRequest || !dialog.open) return;
      scope.textContent = result.scope + (result.truncated ? " Diff truncated." : "");
      content.textContent = result.diff || "No changes in this comparison.";
    } catch (error) {
      if (current === diffRequest) scope.textContent = error.message;
    }
  }

  body.addEventListener("click", async event => {
    const button = event.target.closest("[data-bench-review-action]");
    if (!button || !selected || actionPending) return;
    const id = selected.id;
    const action = button.dataset.benchReviewAction;
    if (action === "good-diff") return openDiff("/api/good/diff");
    if (action === "diff") return openDiff(`/api/specs/${id}/sdlc-diff`);
    workbench.selectSpec(id, { navigate: false });
    if (action === "commit") return workbench.openCommit();
    if (action === "transfer") return workbench.openTransfer();
    if (action === "handoff") return workbench.openHandoff({ reviewed: true });
    actionPending = true;
    button.disabled = true;
    try {
      if (action === "run") await workbench.runCheck();
      else await workbench.openProof(action, Number(button.dataset.criterion), button.dataset.evidence);
    } catch (error) {
      toast(error.message, "error");
    } finally {
      actionPending = false;
      button.disabled = false;
    }
  });

  return { render, refreshChecks, setChecks(value) { report = value; render(); } };
}
