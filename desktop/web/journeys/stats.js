// Pure helpers for the journeys: percentiles, read verdicts and the host's Server-Timing phases.
// Kept free of Playwright so the unit tests (npm test) cover them.
import { BUDGET } from "./budget.js";

// Nearest-rank percentile of `values` (0 < p <= 100).
export function percentile(values, p) {
  if (!values.length) return NaN;
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.max(0, Math.ceil((p / 100) * sorted.length) - 1)];
}

export function summarize(values) {
  return { median: percentile(values, 50), p75: percentile(values, 75), p90: percentile(values, 90) };
}

// fail: p75 over the read limit or p90 over the tail limit. slow: median over the target.
export function readVerdict({ median, p75, p90 }, budget = BUDGET) {
  if (p75 > budget.readLimit || p90 > budget.readTailLimit) return "fail";
  return median > budget.readTarget ? "slow" : "pass";
}

export const PHASES = ["queue", "boundary", "work", "total"];

// The host's own phases from a Server-Timing value; anything a proxy adds is ignored.
export function parseServerTiming(value) {
  if (!value) return null;
  const phases = {};
  for (const entry of value.split(",")) {
    const [name, ...params] = entry.trim().split(";").map(part => part.trim());
    if (!PHASES.includes(name)) continue;
    const phase = {};
    for (const param of params) {
      const at = param.indexOf("=");
      if (at < 0) continue;
      const key = param.slice(0, at);
      const text = param.slice(at + 1).replace(/^"|"$/g, "");
      if (key === "dur") phase.ms = Number(text);
      if (key === "desc") {
        const count = /^n=(\d+)$/.exec(text);
        if (count) phase.entries = Number(count[1]);
      }
    }
    phases[name] = phase;
  }
  return Object.keys(phases).length ? phases : null;
}
