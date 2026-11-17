// Each run starts a fresh summary, so a report never mixes two runs. Then one check that Verb
// answers at all, so a wrong address or tunnel fails once with the reason, not five times.
import { rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { accessHeaders } from "./access.js";

export default async function () {
  rmSync(join(dirname(fileURLToPath(import.meta.url)), "..", "journeys-report", "summary.json"), { force: true });
  const url = (process.env.VERB_URL || "").replace(/\/+$/, "");
  const access = accessHeaders();
  if (!url || (!process.env.VERB_TOKEN && !Object.keys(access).length)) {
    throw new Error("Set VERB_URL and VERB_TOKEN, or the Access service token (never printed).");
  }
  let reply;
  try {
    reply = await fetch(`${url}/healthz`, { headers: access, signal: AbortSignal.timeout(15_000) });
  } catch (error) {
    throw new Error(`Verb did not answer at ${url}: ${error.cause?.code || error.message}. Is the tunnel up?`);
  }
  if (reply.ok) return;
  const reason = (await reply.json().catch(() => ({}))).error || reply.statusText;
  const hint = /unexpected host/.test(reason)
    ? " Verb only accepts its own address: tunnel the same port number locally (for example -L 3005:127.0.0.1:3005) and use that in VERB_URL."
    : "";
  throw new Error(`Verb at ${url} answered ${reply.status} (${reason}).${hint}`);
}
