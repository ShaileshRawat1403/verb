// Presentation only. Structural Spec facts come from the Rust resolver.
export function specPresentation(spec, summary, signals = {}, now = Date.now()) {
  const current = Boolean(summary && typeof spec.revision === "string" && summary.revision === spec.revision);
  const age = now - summary?.observedAt;
  const observed = current && Number.isFinite(summary.observedAt) && age >= -5000 && age <= 90_000;
  const sessions = observed ? summary.execution.sessions : [];
  const attention = current ? summary.attention.filter(item => observed || item.source === "spec stage") : [];
  const waiting = sessions.filter(s => signals.waitingTerminals?.has(s.id) && signals.liveIds?.has(s.id));
  for (const session of waiting) {
    const previous = attention.findIndex(item => item.sessionId === session.id);
    if (previous >= 0) attention.splice(previous, 1);
    attention.unshift({ reason: "Agent reported that it needs your input", source: "agent observer", sessionId: session.id });
  }
  if (signals.waitingTalkSpecs?.has(spec.id)) {
    attention.unshift({ reason: "A conversation is waiting for your decision", source: "conversation permission request", sessionId: null });
  }
  return {
    current, observed, attention,
    sdlc: current ? summary.sdlc : null,
    verificationNote: current && summary.sdlc && !summary.sdlc.error ? `Verification ${summary.sdlc.verification} · review ${String(summary.sdlc.reviewState).replaceAll("_", " ")}` : "Result and freshness not established",
    execution: waiting.length ? "Agent reported: needs your input" : observed ? summary.execution.label : "Execution observations unavailable",
    verification: current ? summary.verification.label : "Updating criterion records…",
    verificationDetail: current ? summary.verification.detail : "Refresh to read this spec's current evidence records.",
    delivery: current ? summary.delivery.label : "Release not observed",
    deliveryDetail: current ? summary.delivery.detail : "A chosen stage does not prove a release.",
    checkout: current && observed && summary.delivery.checkoutChangedFiles != null
      ? `${summary.delivery.checkoutChangedFiles} changed files in this checkout; attribution to this spec is not established`
      : "Checkout changes are not attributed to this spec",
    observedAt: observed ? summary.observedAt : null,
  };
}

export function filterSpecs(specs, presentations, query, attentionOnly) {
  const words = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
  return specs.filter(spec => (!attentionOnly || presentations.get(spec.id)?.attention.length)
    && words.every(word => `${spec.id} ${spec.title} ${spec.stage} ${spec.branch}`.toLocaleLowerCase().includes(word)));
}
