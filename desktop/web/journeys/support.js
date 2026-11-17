import { mkdirSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
export { test, expect } from "@playwright/test";

const here = dirname(fileURLToPath(import.meta.url));
const SUMMARY = join(here, "..", "journeys-report", "summary.json");

export function target() {
  const url = (process.env.VERB_URL || "").replace(/\/+$/, "");
  const token = process.env.VERB_TOKEN || "";
  if (!url || !token) throw new Error("Set VERB_URL and VERB_TOKEN (the token is never printed).");
  return { url, signIn: `${url}/#${token}` };
}

// Records one measurement for the run's summary. Values only: never URLs with keys, never output.
export function record(journey, name, value, unit = "ms", verdict = "info") {
  mkdirSync(dirname(SUMMARY), { recursive: true });
  const summary = existsSync(SUMMARY) ? JSON.parse(readFileSync(SUMMARY, "utf8")) : { started: new Date().toISOString(), results: [] };
  summary.results.push({ journey, name, value: Math.round(value), unit, verdict });
  writeFileSync(SUMMARY, JSON.stringify(summary, null, 2) + "\n");
}

export function verdict(ms, target, limit) {
  return ms <= target ? "pass" : ms <= limit ? "slow" : "fail";
}

// Opens Verb signed in, the way a person pastes the printed URL.
export async function open(page) {
  const { signIn } = target();
  const started = Date.now();
  await page.goto(signIn);
  return started;
}
