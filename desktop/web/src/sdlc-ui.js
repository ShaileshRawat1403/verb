import { renderMarkdown } from "./workbench.js";

export function initSdlc({ api, $, escapeHtml: esc, toast, showDialog, closeDialog, current, changed, openGit }) {
  let spec = null, flow = null, pending = null, running = false, generation = 0;
  let panel = "brief", lastHint = null;
  const names = { brief: "Spec", plan: "Plan", build: "Build", verify: "Verify", review: "Review", ship: "Ship" };
  const words = value => String(value || "unknown").replaceAll("_", " ");
  const button = (action, label) => `<button class="secondary-button" type="button" data-sdlc-action="${action}"${running || !flow || flow.error ? " disabled" : ""}>${label}</button>`;
  const sourceText = record => record?.source === "observed" ? "Verb-observed" : record?.source === "agent" ? "Agent-reported" : "Person-reported";
  function render() {
    $("#sdlc-tabs").innerHTML = Object.entries(names).map(([id, label]) => `<button type="button" class="secondary-button" data-sdlc-panel="${id}" aria-controls="sdlc-content" aria-pressed="${panel === id}">${label}</button>`).join("");
    $("#sdlc-content").setAttribute("aria-label", `${names[panel]} workspace`);
    $("#sdlc-context").textContent = flow?.error || (flow?.source ? `Source ${flow.source.commit.slice(0, 10)} · ${flow.source.branch} · ${flow.source.clean ? "code committed" : "code changes need saving"}. Records belong to this Verb host.` : "Source observations unavailable. Plans can still be recorded; evidence requires committed code.");
    if (!spec || !flow) { $("#sdlc-content").innerHTML = '<p class="muted">Reading lifecycle records…</p>'; return; }
    if (flow.error) { $("#sdlc-content").innerHTML = '<p class="warning-box">Records could not be read. Refresh before adding evidence; existing records have not been replaced.</p>'; return; }
    const criteria = flow.criteria || [];
    let html = "";
    if (panel === "brief") html = `<div class="sdlc-prose">${renderMarkdown(spec.body.split("\n## Audit trail")[0], esc)}</div><p class="muted">Legacy checkmarks are claims. Verify links evidence and records explicit acceptance.</p>`;
    if (panel === "plan") html = `<h3>Approach and completion target</h3>${flow.plan ? `<div class="sdlc-prose">${renderMarkdown(flow.plan.label, esc)}</div><p><b>Completion target:</b> ${esc(words(flow.goal))}${flow.plan.target ? ` · ${esc(flow.plan.target)}` : ""}</p>` : '<p>No plan recorded yet. Describe the approach, risks, verification strategy and what delivery means.</p>'}${button("plan", flow.plan ? "Update plan…" : "Write plan…")}`;
    if (panel === "build") html = `<h3>Work in the right checkout</h3><p>Spec branch: <code>${esc(spec.branch || "not defined")}</code>. This checkout: <code>${esc(flow.source?.branch || "unknown")}</code>.</p><p>Sessions and file changes are observations, not ownership guarantees. Keep one writer per checkout; use separate worktrees for independent writers.</p><div class="sdlc-actions"><button class="secondary-button" type="button" data-action="spec-terminal">Open terminal</button><button class="secondary-button" type="button" data-action="switch-branch">Switch to spec branch</button>${button("git", "Review Git upload / download…")}</div>`;
    if (panel === "verify") html = `<h3>Evidence for each promise</h3><p>${flow.accepted} of ${flow.total} criteria have accepted current evidence. A passing check awaits your acceptance.</p><div class="sdlc-actions">${button("run", "Run a check…")}${button("report", "Add a report…")}</div><ol class="sdlc-criteria">${criteria.map(row => `<li><strong>${esc(row.text)}</strong><span class="sdlc-result" data-result="${esc(row.result || "unproven")}">${row.accepted ? "Accepted" : "Not accepted"} · ${esc(words(row.result || "unproven"))} · ${esc(row.freshness)}</span>${row.evidence ? `<p>${esc(sourceText(row.evidence))}: ${esc(row.evidence.label)}</p><small>${esc(row.evidence.actor)} · ${esc(new Date(row.evidence.at).toLocaleString())} · ${esc(row.evidence.target || "environment not reported")}</small>${row.result === "passed" && row.freshness === "current" && !row.accepted ? `<button class="secondary-button" type="button" data-sdlc-accept="${esc(row.evidence.id)}"${running ? " disabled" : ""}>Accept this evidence…</button>` : ""}` : '<p class="muted">No structured evidence yet. A checkbox alone does not establish verification.</p>'}</li>`).join("")}</ol>`;
    if (panel === "review") html = `<h3>Review the implementation against the promise</h3><p><b>Review:</b> ${esc(words(flow.reviewState))}. <b>Evidence:</b> ${flow.accepted}/${flow.total} accepted.</p>${flow.review ? `<p>${esc(flow.review.note)}<br><small>${esc(flow.review.actor)} · ${esc(new Date(flow.review.at).toLocaleString())}</small></p>` : ""}<p>Approval requires accepted current evidence for every criterion. A new implementation, plan or accepted evidence set requires a new review.</p><div class="sdlc-actions">${button("diff", "Inspect implementation changes")}${button("review", "Record review…")}</div><pre id="sdlc-diff" class="sdlc-diff" hidden></pre>`;
    if (panel === "ship") html = `<h3>Delivery is more than a stage</h3><dl class="sdlc-delivery"><div><dt>Completion target</dt><dd>${esc(words(flow.goal || "not set"))}${flow.plan?.target ? ` · ${esc(flow.plan.target)}` : ""}</dd></div><div><dt>Implementation review</dt><dd>${esc(words(flow.reviewState))}</dd></div><div><dt>Remote Git</dt><dd>${flow.uploaded ? "Implementation present in the last explicit remote check" : "Upload not established for this implementation"}</dd></div><div><dt>Local integration</dt><dd>${flow.source?.integrated === true ? "Current commit is included in local main/master" : flow.source?.integrated === false ? "Current commit is not in local main/master" : "Unknown"}</dd></div><div><dt>Delivery</dt><dd>${esc(flow.deliveryLabel)}</dd></div></dl><p class="muted">Artifact, deployment and health entries are explicitly labelled manual reports. Recording them does not deploy, restart or probe a service.</p><div class="sdlc-actions">${button("remote", "Check remote and record observation…")}${button("git", "Upload / download…")}${button("delivery", "Record delivery report…")}</div>`;
    $("#sdlc-content").innerHTML = html;
    $("#sdlc-history").innerHTML = (flow.records || []).slice(0, 20).map(r => `<li><strong>${esc(words(r.kind))} · ${esc(sourceText(r))}${r.result ? ` · ${esc(words(r.result))}` : ""}</strong><p>${esc(r.label)}</p>${r.kind === "delivery" ? `<p>Target: ${esc(r.target)} · Reference: ${esc(r.reference || "")}</p>` : ""}<small>${esc(r.actor)} · ${esc(new Date(r.at).toLocaleString())}${r.binding ? ` · ${esc(r.binding.commit.slice(0, 10))}` : ""}${r.exitCode != null ? ` · exit ${r.exitCode}` : ""}</small></li>`).join("") || '<li class="muted">No lifecycle records yet.</li>';
  }
  async function refresh(next = current(), force = false, hint = null) {
    const hintChanged = hint && hint !== lastHint;
    if (!next || running || (!force && !hintChanged && next.id === spec?.id && next.revision === spec?.revision)) return;
    if (hint) lastHint = hint;
    const request = ++generation;
    const different = next.id !== spec?.id;
    spec = next; flow = null;
    if (different) panel = next.stage === "spec" ? "brief" : next.stage;
    render();
    try {
      const value = await api("GET", `/api/specs/${next.id}/sdlc`);
      if (request !== generation) return;
      flow = value; render();
    } catch (error) { if (request === generation) { flow = { error: error.message }; render(); } }
  }
  const text = (label, name, placeholder = "") => `<label for="sdlc-${name}">${label}</label><textarea id="sdlc-${name}" name="${name}" required maxlength="8192" rows="4" placeholder="${esc(placeholder)}"></textarea>`;
  function open(action, evidenceId) {
    if (!flow || flow.error || running || !spec) return;
    pending = { action, expectedRevision: spec.revision, expectedVersion: flow.version, expectedContext: flow.context, ...(evidenceId ? { evidenceId } : {}) };
    const criterion = `<label for="sdlc-criterion">Criterion this evidence supports</label><select id="sdlc-criterion" name="criterion">${flow.criteria.map(c => `<option value="${c.index}">${esc(c.text)}</option>`).join("")}</select>`;
    const result = '<label for="sdlc-result">Reported result</label><select id="sdlc-result" name="result"><option value="passed">Passed</option><option value="failed">Failed</option><option value="inconclusive">Inconclusive</option></select>';
    const target = '<label for="sdlc-target">Device, environment or release target</label><input id="sdlc-target" name="target" maxlength="200" placeholder="e.g. phone server / staging" />';
    let fields = "", title = "", submit = "Save record";
    if (action === "plan") { title = "Plan this work"; fields = text("Approach, risks and verification strategy", "text") + target + '<label for="sdlc-goal">What counts as delivery?</label><select id="sdlc-goal" name="goal" required><option value="">Choose a completion target</option><option value="code">Reviewed code, observed remotely and integrated locally</option><option value="artifact">Reviewed code, observed remotely and release artifact reported</option><option value="service">Reviewed code, observed remotely, deployment and health reported</option></select>'; }
    if (action === "report") { title = "Link an evidence report"; fields = criterion + '<label for="sdlc-source">Who observed this?</label><select id="sdlc-source" name="source"><option value="manual">A person</option><option value="agent">An agent reported it</option></select>' + result + target + text("What was checked and what supports the result?", "text", "Test/device/check reference. Do not paste terminal output, credentials or transcripts."); }
    if (action === "run") { title = "Run a check for this criterion"; submit = "Run check"; fields = '<p class="warning-box">This runs project-defined test code with the existing execution permissions, for up to three minutes. Output is not stored. A pass still needs your acceptance. Save code changes to Git first.</p>' + criterion + '<label for="sdlc-check">Check</label><select id="sdlc-check" name="check"><option value="rust">Rust tests · offline</option><option value="node">Node tests · offline</option><option value="python">Python unit tests</option></select>'; }
    if (action === "accept") { title = "Accept evidence for this criterion"; fields = '<p>Accept only if this evidence is sufficient for the promised behaviour. Source and freshness are checked again before saving.</p>' + text("Why is this evidence sufficient?", "text"); submit = "Accept evidence"; }
    if (action === "review") { title = "Record an implementation review"; fields = `<label for="sdlc-decision">Decision</label><select id="sdlc-decision" name="decision"><option value="changes_requested">Changes requested</option><option value="approved"${flow.accepted !== flow.total || !flow.total ? " disabled" : ""}>Approve this implementation and evidence</option></select>` + text("What did you review and what risks remain?", "text"); }
    if (action === "delivery") { title = "Record a manual delivery report"; fields = '<p>This is your report, not an independently observed deployment. It does not run a deployment or health probe.</p><label for="sdlc-milestone">Milestone</label><select id="sdlc-milestone" name="milestone"><option value="artifact">Release artifact</option><option value="deployment">Deployment</option><option value="health">Health after deployment</option></select>' + target + result + text("Release / deployment / health-check reference", "text"); }
    if (action === "remote") { title = "Check the linked remote"; fields = '<p>This contacts the linked Git remote, fetches its branch information and records whether the current implementation is present there. It does not upload work or change your files.</p>'; submit = "Check remote"; }
    $("#sdlc-dialog-title").textContent = title;
    $("#sdlc-fields").innerHTML = fields;
    $("#sdlc-submit").textContent = submit;
    $("#sdlc-dialog-error").hidden = true;
    if (action === "plan" && flow.plan) { $("#sdlc-text").value = flow.plan.label; $("#sdlc-goal").value = flow.goal || ""; }
    if (action === "delivery" || action === "plan") $("#sdlc-target").value = flow.plan?.target || "";
    showDialog("sdlc-dialog");
    $("#sdlc-fields").querySelector("textarea,select,input")?.focus();
  }
  $("#sdlc-form").addEventListener("submit", async event => {
    event.preventDefault(); if (!pending || running) return;
    const id = spec.id, request = { ...pending, ...Object.fromEntries(new FormData(event.target)) };
    if (request.criterion != null) request.criterion = Number(request.criterion);
    running = true; $("#sdlc-submit").disabled = true;
    $("#sdlc-status").textContent = request.action === "run" ? "Check running. You can use your terminals while it runs. Output will not be stored." : "Recording…";
    try {
      const value = await api("POST", `/api/specs/${id}/sdlc`, request);
      closeDialog("sdlc-dialog");
      if (current()?.id === id) { flow = value; render(); }
      $("#sdlc-status").textContent = "Record saved.";
      await changed();
    } catch (error) {
      $("#sdlc-dialog-error").hidden = false; $("#sdlc-dialog-error").textContent = error.message;
      $("#sdlc-status").textContent = error.message; toast(error.message, "error");
    } finally { running = false; $("#sdlc-submit").disabled = false; render(); }
  });
  $("#sdlc-workflow").addEventListener("click", async event => {
    const selected = event.target.closest("[data-sdlc-panel]");
    if (selected) { panel = selected.dataset.sdlcPanel; render(); $("#sdlc-content").focus({ preventScroll: true }); return; }
    const accept = event.target.closest("[data-sdlc-accept]");
    if (accept) return open("accept", accept.dataset.sdlcAccept);
    const action = event.target.closest("[data-sdlc-action]")?.dataset.sdlcAction;
    if (!action) return;
    if (action === "refresh") return refresh(current(), true);
    if (action === "git") return openGit();
    if (action === "diff") {
      const pre = $("#sdlc-diff"); pre.hidden = false; pre.textContent = "Reading implementation changes…";
      try { const result = await api("GET", `/api/specs/${spec.id}/sdlc-diff`); pre.textContent = `${result.scope}\n${result.truncated ? "Diff truncated; use a terminal to review the remaining changes.\n" : ""}${result.diff || "No changes in this comparison."}`; }
      catch (error) { pre.textContent = error.message; }
      return;
    }
    if (action === "export") {
      try {
        const data = await api("GET", `/api/specs/${spec.id}/sdlc-export`);
        const url = URL.createObjectURL(new Blob([JSON.stringify(data, null, 2)], { type: "application/json" }));
        const link = document.createElement("a"); link.href = url; link.download = `spec-${spec.id}-evidence.json`; link.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
      } catch (error) { toast(error.message, "error"); }
      return;
    }
    open(action);
  });
  return { sync: refresh };
}
