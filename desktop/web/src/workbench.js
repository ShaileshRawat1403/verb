// Pure helpers for the spec-driven workbench: stage guidance, gate warnings, command search and Git
// labels. No DOM access here, so every rule the UI shows can be tested with `npm test`.

export const STAGES = ["spec", "plan", "build", "verify", "review", "ship"];

/** What each stage means and what to do next, in plain words. */
export const STAGE_GUIDE = {
  spec: {
    label: "Spec",
    goal: "Agree on what to build",
    next: "Describe the problem and list acceptance criteria: results anyone can check.",
  },
  plan: {
    label: "Plan",
    goal: "Decide how",
    next: "Switch to this spec's branch so the work stays separate, and note any tasks.",
  },
  build: {
    label: "Build",
    goal: "Make it",
    next: "Open a terminal, or start an AI agent on this spec. It reads the spec first.",
  },
  verify: {
    label: "Verify",
    goal: "Prove it works",
    next: "Run the tests and tick each acceptance criterion with the evidence that proves it.",
  },
  review: {
    label: "Review",
    goal: "Check the changes",
    next: "Read the changed files against the spec, then commit with a clear message.",
  },
  ship: {
    label: "Ship",
    goal: "Release it",
    next: "Merge the branch and release. The audit trail records how it got here.",
  },
};

/** The warnings a stage change will record. Mirrors desktop/src/specs.rs `set_stage`. */
export function stageWarnings(spec, target) {
  const from = STAGES.indexOf(spec.stage);
  const to = STAGES.indexOf(target);
  const warnings = [];
  if (to < 0 || from < 0 || to === from) return warnings;
  if (to > from + 1) warnings.push(`skipped ${STAGES.slice(from + 1, to).join(", ")}`);
  const open = spec.criteria.length - spec.criteriaDone;
  if (to >= 4 && open > 0) {
    warnings.push(`${open} of ${spec.criteria.length} acceptance criteria not yet proven`);
  }
  return warnings;
}

/**
 * Stages the audit trail says were skipped and never entered afterwards, so the stage bar does not
 * show a skipped stage as completed.
 */
export function skippedStages(spec) {
  const skipped = new Set();
  for (const entry of spec.audit ?? []) {
    const entered = /→ (\w+)/.exec(entry.action)?.[1];
    if (entered) skipped.delete(entered);
    const names = /skipped ([a-z, ]+)/.exec(entry.action)?.[1];
    if (names) for (const name of names.split(", ")) if (STAGES.includes(name)) skipped.add(name);
  }
  return skipped;
}

export function progressLabel(spec) {
  if (!spec.criteria.length) return "no criteria yet";
  return `${spec.criteriaDone}/${spec.criteria.length} proven`;
}

/** Specs grouped in stage order; stages with no specs are left out. */
export function groupByStage(specs) {
  return STAGES.map((stage) => ({
    stage,
    specs: specs.filter((spec) => spec.stage === stage),
  })).filter((group) => group.specs.length);
}

const CHANGE_NAMES = {
  M: "Modified",
  A: "Added",
  D: "Deleted",
  R: "Renamed",
  C: "Copied",
  "??": "New file",
  U: "Conflict",
};

/** `git status --porcelain` codes as words. */
export function changeLabel(code) {
  const c = code.trim();
  if (CHANGE_NAMES[c]) return CHANGE_NAMES[c];
  for (const letter of c) if (CHANGE_NAMES[letter]) return CHANGE_NAMES[letter];
  return c || "Changed";
}

/**
 * Subsequence match score for the command palette: every query character must appear in order.
 * Consecutive runs and word starts score higher. Returns -1 when it does not match.
 */
export function matchScore(text, query) {
  const t = text.toLowerCase();
  const q = query.toLowerCase().trim();
  if (!q) return 0;
  let score = 0;
  let pos = -1;
  let run = 0;
  for (const ch of q) {
    const next = t.indexOf(ch, pos + 1);
    if (next < 0) return -1;
    run = next === pos + 1 ? run + 1 : 0;
    const wordStart = next === 0 || /[\s\-:/·]/.test(t[next - 1]);
    score += 1 + run * 2 + (wordStart ? 3 : 0);
    pos = next;
  }
  return score - t.length * 0.01;
}

/** Commands ranked for `query`; an empty query keeps the given order. */
export function filterCommands(commands, query) {
  if (!query.trim()) return commands;
  return commands
    .map((command) => ({
      command,
      score: matchScore(`${command.title} ${command.hint ?? ""}`, query),
    }))
    .filter((entry) => entry.score >= 0)
    .sort((a, b) => b.score - a.score)
    .map((entry) => entry.command);
}

/** A suggested commit message for the selected spec. */
export function commitPrefix(spec) {
  return spec ? `spec:${spec.id} ` : "";
}

/** Nests flat paths ("a/b.txt") into folders; folders first, then files, each alphabetical. */
export function buildTree(files) {
  const root = { name: "", path: "", dirs: new Map(), files: [] };
  for (const file of files) {
    const parts = file.path.split("/");
    let node = root;
    for (let i = 0; i < parts.length - 1; i += 1) {
      const name = parts[i];
      if (!node.dirs.has(name)) {
        node.dirs.set(name, {
          name,
          path: parts.slice(0, i + 1).join("/"),
          dirs: new Map(),
          files: [],
          changed: 0,
        });
      }
      node = node.dirs.get(name);
    }
    node.files.push({ name: parts.at(-1), path: file.path, status: file.status ?? null });
  }
  const finish = (node) => {
    node.files.sort((a, b) => a.name.localeCompare(b.name));
    const dirs = [...node.dirs.values()].sort((a, b) => a.name.localeCompare(b.name));
    dirs.forEach(finish);
    node.changed =
      node.files.filter((f) => f.status).length + dirs.reduce((sum, d) => sum + d.changed, 0);
    node.dirs = dirs;
    return node;
  };
  return finish(root);
}

/** Files whose path matches every space-separated word of the query, best first, capped. */
export function filterFiles(files, query, limit = 200) {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return [];
  return files
    .map((file) => {
      const p = file.path.toLowerCase();
      if (!words.every((w) => p.includes(w))) return null;
      const name = p.split("/").at(-1);
      const score = words.reduce((s, w) => s + (name.includes(w) ? 3 : 1), 0) - p.length * 0.001;
      return { file, score };
    })
    .filter(Boolean)
    .sort((a, b) => b.score - a.score)
    .slice(0, limit)
    .map((entry) => entry.file);
}

/** "3d 4h", "2h 5m", "4m", "12s": the two largest units, for uptimes. */
export function formatDuration(secs) {
  if (secs == null) return "—";
  const units = [
    ["d", 86400],
    ["h", 3600],
    ["m", 60],
    ["s", 1],
  ];
  const parts = [];
  let rest = Math.max(0, Math.floor(secs));
  for (const [label, size] of units) {
    const n = Math.floor(rest / size);
    rest -= n * size;
    if (n || (label === "s" && !parts.length)) parts.push(`${n}${label}`);
    if (parts.length === 2) break;
  }
  return parts.join(" ");
}

/** Share of a resource in use, 0–100, rounded. */
export function usedPercent(usage) {
  if (!usage || !usage.total_kb) return null;
  return Math.round(((usage.total_kb - usage.available_kb) / usage.total_kb) * 100);
}

/** KB → "2.4 GB" / "512 MB". */
export function formatKb(kb) {
  if (kb == null) return "—";
  if (kb >= 1024 * 1024) return `${(kb / 1024 / 1024).toFixed(1)} GB`;
  return `${Math.round(kb / 1024)} MB`;
}

/** Phone-friendly thresholds: comfortable, warm (watch it), hot (throttling likely). */
export function temperatureTone(celsius) {
  if (celsius >= 45) return "hot";
  if (celsius >= 40) return "warm";
  return "ok";
}
