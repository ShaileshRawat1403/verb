// Each run starts a fresh summary, so a report never mixes two runs.
import { rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export default function () {
  rmSync(join(dirname(fileURLToPath(import.meta.url)), "..", "journeys-report", "summary.json"), { force: true });
}
