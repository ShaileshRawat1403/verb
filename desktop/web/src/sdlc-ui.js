import { readWithBusyRetry } from "./read-retry.js";

export function initSdlc({ api, $, escapeHtml: esc, toast, showDialog, closeDialog, current, replaceSpec, changed, openCommit }) {
  let spec = null, flow = null, pending = null, running = false, generation = 0, pendingSpecId = null;
  function status(message, id = pendingSpecId) {
    $("#sdlc-dialog-status").textContent = message;
    document.dispatchEvent(new CustomEvent("verb:evidence-status", { detail: { specId: id, message } }));
  }
  const words = value => String(value || "unknown").replaceAll("_", " ");
  const sourceText = record => record?.source === "observed" ? "Verb-observed" : record?.source === "agent" ? "Agent-reported" : "Person-reported";
  function render() {
    $("#sdlc-history").innerHTML = flow?.error ? `<li>${esc(flow.error)}</li>`
      : !flow ? "<li>Reading evidence…</li>"
      : (flow.records || []).slice(0, 20).map(r => `<li><strong>${esc(words(r.kind))} · ${esc(sourceText(r))}${r.result ? ` · ${esc(words(r.result))}` : ""}</strong><p>${esc(r.label)}</p>${r.kind === "delivery" ? `<p>Target: ${esc(r.target)} · Reference: ${esc(r.reference || "")}</p>` : ""}<small>${esc(r.actor)} · ${esc(new Date(r.at).toLocaleString())}${r.binding ? ` · ${esc(r.binding.commit.slice(0, 10))}` : ""}${r.exitCode != null ? ` · exit ${r.exitCode}` : ""}</small></li>`).join("") || '<li class="muted">No lifecycle records yet.</li>';
  }
  async function refresh(next = current()) {
    if (!next || running) return;
    const request = ++generation;
    spec = next;
    flow = null;
    render();
    try {
      const value = await readWithBusyRetry(api, `/api/specs/${next.id}/sdlc`);
      if (request !== generation) return;
      flow = value;
    } catch (error) {
      if (request !== generation) return;
      flow = { error: error.message };
    }
    render();
  }
  const text = (label, name, placeholder = "") => `<label for="sdlc-${name}">${label}</label><textarea id="sdlc-${name}" name="${name}" required maxlength="8192" rows="4" placeholder="${esc(placeholder)}"></textarea>`;
  function open(action, evidenceId, index, prefill) {
    if (running || !spec || (action !== "claim" && (!flow || flow.error))) return;
    pendingSpecId = spec.id;
    pending = { action, expectedRevision: action === "claim" ? spec.revision : flow.specRevision, expectedVersion: flow?.version, expectedContext: flow?.context, ...(evidenceId ? { evidenceId } : {}) };
    const criterion = `<label for="sdlc-criterion">Criterion this evidence supports</label><select id="sdlc-criterion" name="criterion">${(flow?.criteria ?? []).map(c => `<option value="${c.index}">${esc(c.text)}</option>`).join("")}</select>`;
    const result = '<label for="sdlc-result">Reported result</label><select id="sdlc-result" name="result"><option value="passed">Passed</option><option value="failed">Failed</option><option value="inconclusive">Inconclusive</option></select>';
    const target = '<label for="sdlc-target">Device, environment or release target</label><input id="sdlc-target" name="target" maxlength="200" placeholder="e.g. phone server / staging" />';
    let fields = "", title = "", submit = "Save record";
    if (action === "claim") {
      pending.index = index;
      title = "Record a completion claim";
      submit = "Record claim";
      fields = `<p>${esc(spec.criteria.find(row => row.index === index).text)}</p><p>This records a claim. Evidence is accepted separately.</p><label for="sdlc-text">Supporting details (optional)</label><textarea id="sdlc-text" name="text" maxlength="8192" rows="4"></textarea>`;
    }
    if (action === "plan") { title = "Plan this work"; fields = text("Approach, risks and verification strategy", "text") + target + '<label for="sdlc-goal">What counts as delivery?</label><select id="sdlc-goal" name="goal" required><option value="">Choose a completion target</option><option value="code">Reviewed code, observed remotely and integrated locally</option><option value="artifact">Reviewed code, observed remotely and release artifact reported</option><option value="service">Reviewed code, observed remotely, deployment and health reported</option></select>'; }
    if (action === "report") { title = "Record proof"; fields = criterion + '<label for="sdlc-source">Who observed this?</label><select id="sdlc-source" name="source"><option value="manual">A person</option><option value="agent">An agent reported it</option></select>' + result + target.replace("Device, environment or release target", "Where it was checked (optional)") + text("What was checked and what supports the result?", "text", "Test/device/check reference. Do not paste terminal output, credentials or transcripts."); }
    if (action === "run") { title = "Run a check for this criterion"; submit = "Run check"; fields = '<p class="warning-box">This runs project-defined test code with the existing execution permissions, for up to three minutes. Output is not stored. A pass still needs your acceptance. Save code changes to Git first.</p>' + criterion + '<label for="sdlc-check">Check</label><select id="sdlc-check" name="check"><option value="rust">Rust tests · offline</option><option value="node">Node tests · offline</option><option value="python">Python unit tests</option></select>'; }
    if (action === "run") fields += `<div class="sdlc-check-preview"><p id="sdlc-check-source">Source: ${esc(flow.source?.branch || "unknown")} · ${esc(flow.source?.commit?.slice(0, 10) || "unavailable")} · ${flow.source?.clean ? "code committed" : "commit code changes before running"}</p><p id="sdlc-command-label">Command to run in the project root</p><pre id="sdlc-check-command" aria-labelledby="sdlc-command-label" tabindex="0"></pre><p id="sdlc-check-description"></p></div>`;
    if (action === "accept") { title = "Accept evidence for this criterion"; fields = '<p>Accept only if this evidence is sufficient for the promised behaviour. Source and freshness are checked again before saving.</p>' + text("Why is this evidence sufficient?", "text"); submit = "Accept evidence"; }
    if (action === "review") { title = "Record an implementation review"; fields = `<label for="sdlc-decision">Decision</label><select id="sdlc-decision" name="decision"><option value="changes_requested">Changes requested</option><option value="approved"${flow.accepted !== flow.total || !flow.total ? " disabled" : ""}>Approve this implementation and evidence</option></select>` + text("What did you review and what risks remain?", "text"); }
    if (action === "delivery") { title = "Record a manual delivery report"; fields = '<p>This is your report, not an independently observed deployment. It does not run a deployment or health probe.</p><label for="sdlc-milestone">Milestone</label><select id="sdlc-milestone" name="milestone"><option value="artifact">Release artifact</option><option value="deployment">Deployment</option><option value="health">Health after deployment</option></select>' + target + result + text("Release / deployment / health-check reference", "text"); }
    if (action === "remote") { title = "Check the linked remote"; fields = '<p>This contacts the linked Git remote, fetches its branch information and records whether the current implementation is present there. It does not upload work or change your files.</p>'; submit = "Check remote"; }
    const commitRequired = action === "report" && flow.source?.clean === false;
    if (commitRequired) {
      fields = '<div class="warning-box"><p>Commit code changes before recording proof.</p><button type="button" class="secondary-button" id="sdlc-open-commit">Commit changes</button></div>' + fields;
    }
    $("#sdlc-dialog-title").textContent = title;
    $("#sdlc-fields").innerHTML = fields;
    $("#sdlc-submit").textContent = submit;
    $("#sdlc-submit").disabled = commitRequired;
    $("#sdlc-open-commit")?.addEventListener("click", async () => {
      const draft = new FormData($("#sdlc-form"));
      const id = pendingSpecId;
      closeDialog("sdlc-dialog");
      $("#commit-dialog").addEventListener("close", async () => {
        try {
          await changed();
          if (current()?.id !== id) return;
          await refresh(current());
          if (!flow || flow.error || current()?.id !== id) throw new Error("Could not refresh proof after commit review.");
          open("report");
          for (const [key, value] of draft) $("#sdlc-form").elements[key].value = value;
        } catch (error) {
          toast(error.message, "error");
        }
      }, { once: true });
      await openCommit();
    });
    $("#sdlc-dialog-error").hidden = true;
    $("#sdlc-dialog-status").textContent = "";
    if (action === "plan" && flow.plan) { $("#sdlc-text").value = flow.plan.label; $("#sdlc-goal").value = flow.goal || ""; }
    if (action === "delivery" || action === "plan") $("#sdlc-target").value = flow.plan?.target || "";
    if (action === "run") {
      const descriptions = {
        rust: ["cargo test --offline", "Runs the project's Rust tests. Cargo will not download dependencies; test code can still use the network."],
        node: ["npm --offline test", "Runs the project's npm test script, including its pretest and posttest scripts. npm will not download dependencies; script code can still use the network."],
        python: ["python3 -c 'import sys, unittest; suite = unittest.defaultTestLoader.discover(\".\"); sys.exit(125 if suite.countTestCases() == 0 else (0 if unittest.TextTestRunner().run(suite).wasSuccessful() else 1))'", "Discovers Python unittest tests from the project root. No tests or a missing runtime is inconclusive, not a pass."],
      };
      const explain = () => {
        const [command, description] = descriptions[$("#sdlc-check").value];
        $("#sdlc-check-command").textContent = command;
        $("#sdlc-check-description").textContent = description;
      };
      $("#sdlc-check").addEventListener("change", explain);
      explain();
    }
    if (action === "claim") $("#sdlc-text").value = prefill ?? "";
    showDialog("sdlc-dialog");
    $("#sdlc-fields").querySelector("textarea,select,input")?.focus();
  }
  $("#sdlc-form").addEventListener("submit", async event => {
    event.preventDefault(); if (!pending || running || $("#sdlc-submit").disabled) return;
    const id = pendingSpecId, request = { ...pending, ...Object.fromEntries(new FormData(event.target)) };
    if (request.criterion != null) request.criterion = Number(request.criterion);
    running = true; $("#sdlc-submit").disabled = true;
    status(request.action === "run" ? "Check running. You can use your terminals while it runs. Output will not be stored." : "Recording…", id);
    try {
      if (request.action === "claim") {
        const chosen = current();
        if (chosen?.id !== id || chosen.revision !== request.expectedRevision) throw new Error("The spec changed. Review the criterion again.");
        replaceSpec(await api("POST", `/api/specs/${id}/criteria/${request.index}`, { done: true, evidence: request.text }));
        closeDialog("sdlc-dialog");
        toast("Completion claim recorded.");
        return;
      }
      const value = await api("POST", `/api/specs/${id}/sdlc`, request);
      if (current()?.id === id) { flow = value; render(); }
      status(request.action === "run" ? "Check result recorded. Review evidence before accepting it." : "Record saved.", id);
      document.dispatchEvent(new CustomEvent("verb:evidence-changed", { detail: { specId: id } }));
      await changed();
      closeDialog("sdlc-dialog");
    } catch (error) {
      $("#sdlc-dialog-error").hidden = false; $("#sdlc-dialog-error").textContent = error.message;
      status(request.action === "run" ? `Check outcome not confirmed: ${error.message}. Review evidence before trying again; this request will not be resent automatically.` : error.message, id); toast(error.message, "error");
    } finally { running = false; $("#sdlc-submit").disabled = false; render(); }
  });
  return {
    history: refresh,
    async exportEvidence() {
      const chosen = current();
      if (!chosen) return;
      const data = await api("GET", `/api/specs/${chosen.id}/sdlc-export`);
      const url = URL.createObjectURL(new Blob([JSON.stringify(data, null, 2)], { type: "application/json" }));
      const link = document.createElement("a");
      link.href = url;
      link.download = `spec-${chosen.id}-evidence.json`;
      link.click();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
    },
    async openAction(action, index, prefill) {
      if (running) throw new Error("An evidence action is already running.");
      const chosen = current();
      if (!chosen) return;
      if (action === "claim") {
        const criterion = chosen.criteria.find(row => row.index === index);
        if (!criterion || criterion.done) throw new Error("Review the current criterion before recording a claim.");
        spec = chosen;
        open(action, null, index, prefill);
        return;
      }
      await refresh(chosen);
      if (current()?.id !== chosen.id || spec?.id !== chosen.id || !flow || flow.error)
        throw new Error("Evidence is unavailable or the selected spec changed.");
      open(action, null, index, prefill);
    },
    async openCheck() {
      if (running) throw new Error("A check or evidence action is already running. Wait for its result before starting another.");
      const chosen = current();
      if (!chosen) throw new Error("Choose work with acceptance criteria before running a check.");
      await refresh(chosen);
      if (current()?.id !== chosen.id || spec?.id !== chosen.id || !flow || flow.error)
        throw new Error("Check context changed or could not be read. Refresh and try again.");
      if (!flow.criteria?.length) throw new Error("Add an acceptance criterion before running a check.");
      open("run");
    },
    async openProof(action, index, evidenceId) {
      if (running) throw new Error("A check or evidence action is already running.");
      const chosen = current();
      if (!chosen) throw new Error("Choose a spec first.");
      await refresh(chosen);
      if (current()?.id !== chosen.id || spec?.id !== chosen.id || !flow || flow.error)
        throw new Error("Evidence context changed or could not be read. Refresh and try again.");
      const criterion = flow.criteria.find(row => row.index === index);
      if (!criterion || (action === "accept" && criterion.evidence?.id !== evidenceId))
        throw new Error("The criterion or its evidence changed. Review the current record.");
      open(action, evidenceId);
      if (action === "report") $("#sdlc-criterion").value = String(index);
    },
  };
}
