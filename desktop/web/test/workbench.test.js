import { test } from "node:test";
import assert from "node:assert/strict";
import {
  STAGES,
  STAGE_GUIDE,
  changeLabel,
  commitPrefix,
  filterCommands,
  groupByStage,
  matchScore,
  progressLabel,
  skippedStages,
  stageWarnings,
} from "../src/workbench.js";

const spec = (stage, done = 0, total = 3) => ({
  id: "012",
  stage,
  criteriaDone: done,
  criteria: Array.from({ length: total }, (_, i) => ({ index: i, done: i < done })),
});

test("every stage has guidance", () => {
  for (const stage of STAGES) assert.ok(STAGE_GUIDE[stage].next.length > 10, stage);
});

test("stage warnings match the server's rules", () => {
  assert.deepEqual(stageWarnings(spec("spec"), "plan"), []);
  assert.deepEqual(stageWarnings(spec("spec"), "build"), ["skipped plan"]);
  assert.deepEqual(stageWarnings(spec("verify", 1), "review"), [
    "2 of 3 acceptance criteria not yet proven",
  ]);
  assert.deepEqual(stageWarnings(spec("verify", 3), "review"), []);
  assert.deepEqual(stageWarnings(spec("build"), "build"), []);
  assert.deepEqual(stageWarnings(spec("review"), "plan"), [], "moving back never warns");
});

test("progress and grouping", () => {
  assert.equal(progressLabel(spec("build", 1)), "1/3 proven");
  assert.equal(progressLabel(spec("build", 0, 0)), "no criteria yet");
  const groups = groupByStage([spec("build"), spec("spec"), spec("build")]);
  assert.deepEqual(groups.map((g) => [g.stage, g.specs.length]), [["spec", 1], ["build", 2]]);
});

test("git codes read as words", () => {
  assert.equal(changeLabel("M"), "Modified");
  assert.equal(changeLabel("??"), "New file");
  assert.equal(changeLabel("AM"), "Added");
  assert.equal(changeLabel(" D"), "Deleted");
});

test("palette search matches in order and prefers word starts", () => {
  assert.equal(matchScore("New terminal", "xyz"), -1);
  assert.ok(matchScore("New terminal", "nt") > matchScore("Commit changes", "nt"));
  const commands = [
    { title: "Commit changes" },
    { title: "New spec" },
    { title: "New terminal" },
  ];
  assert.equal(filterCommands(commands, "ns")[0].title, "New spec");
  assert.equal(filterCommands(commands, "").length, 3);
});

test("commit prefix", () => {
  assert.equal(commitPrefix({ id: "007" }), "spec:007 ");
  assert.equal(commitPrefix(null), "");
});

test("skipped stages come from the audit trail until they are entered", () => {
  const audit = (actions) => ({ audit: actions.map((action) => ({ action })) });
  assert.deepEqual([...skippedStages(audit(["created spec", "stage spec → build (warning: skipped plan)"]))], ["plan"]);
  assert.deepEqual(
    [...skippedStages(audit(["stage spec → verify (warning: skipped plan, build)", "stage verify → plan"]))],
    ["build"],
  );
  assert.equal(skippedStages(audit(["stage spec → plan"])).size, 0);
});

import { buildTree, filterFiles } from "../src/workbench.js";

test("files nest into folders, folders first, with change counts", () => {
  const tree = buildTree([
    { path: "src/b.js", status: "M" },
    { path: "README.md" },
    { path: "src/a.js" },
    { path: "src/lib/x.js", status: "??" },
  ]);
  assert.deepEqual(tree.dirs.map((d) => d.name), ["src"]);
  assert.deepEqual(tree.files.map((f) => f.name), ["README.md"]);
  const src = tree.dirs[0];
  assert.deepEqual(src.files.map((f) => f.name), ["a.js", "b.js"]);
  assert.equal(src.dirs[0].path, "src/lib");
  assert.equal(src.changed, 2);
  assert.equal(tree.changed, 2);
});

test("file filter needs every word and prefers file-name hits", () => {
  const files = [{ path: "src/auth/login.js" }, { path: "docs/login-notes/readme.md" }, { path: "src/app.js" }];
  assert.deepEqual(filterFiles(files, "login").map((f) => f.path)[0], "src/auth/login.js");
  assert.deepEqual(filterFiles(files, "src login").map((f) => f.path), ["src/auth/login.js"]);
  assert.deepEqual(filterFiles(files, ""), []);
});

import { formatDuration, formatKb, temperatureTone, usedPercent } from "../src/workbench.js";

test("host formatting", () => {
  assert.equal(formatDuration(43270), "12h 1m");
  assert.equal(formatDuration(86400 * 3 + 3600 * 4 + 5), "3d 4h");
  assert.equal(formatDuration(12), "12s");
  assert.equal(formatDuration(0), "0s");
  assert.equal(formatDuration(null), "—");
  assert.equal(usedPercent({ total_kb: 7439804, available_kb: 2462520 }), 67);
  assert.equal(usedPercent(null), null);
  assert.equal(formatKb(7439804), "7.1 GB");
  assert.equal(formatKb(524288), "512 MB");
  assert.deepEqual([38, 40, 44.9, 45].map(temperatureTone), ["ok", "warm", "warm", "hot"]);
});

import { lastAgent, specSessions } from "../src/workbench.js";

test("spec sessions come from the audit trail, joined with live sessions", () => {
  const spec = {
    audit: [
      { at: "2026-10-08T01:00:00Z", action: "created spec \"x\"" },
      { at: "2026-10-08T02:00:00Z", action: "started claude on this spec (session aaaa1111)" },
      { at: "2026-10-08T03:00:00Z", action: "handed off from claude to codex — flaky tests" },
      { at: "2026-10-08T03:00:01Z", action: "started codex on this spec (session bbbb2222)" },
    ],
  };
  const live = [{ id: "bbbb2222-full-id", state: "live" }];
  const sessions = specSessions(spec, live);
  assert.deepEqual(
    sessions.map((s) => [s.agent, s.short, s.live, s.id]),
    [
      ["codex", "bbbb2222", true, "bbbb2222-full-id"],
      ["claude", "aaaa1111", false, null],
    ],
  );
  assert.equal(lastAgent(spec), "codex");
  assert.equal(lastAgent({ audit: [] }), null);
});

import { meterLabel } from "../src/workbench.js";

test("context meter labels: percentages only when the window is known", () => {
  const codex = meterLabel({ tokens: 185167, window: 760000, percent: 24, rate_limits: [{ label: "5-hour limit", used_percent: 18 }] });
  assert.equal(codex.text, "24% of 760K context · 5-hour limit 18%");
  assert.equal(codex.tone, "ok");
  assert.equal(meterLabel({ tokens: 700000, window: 760000, percent: 92, rate_limits: [] }).tone, "warm");
  assert.equal(meterLabel({ tokens: 750000, window: 760000, percent: 98, rate_limits: [] }).tone, "hot");
  const claude = meterLabel({ tokens: 732864, percent: null, rate_limits: [], note: "Claude Code does not record its context window size." });
  assert.equal(claude.text, "733K tokens in context");
  assert.equal(claude.percent, null);
  assert.equal(meterLabel({ note: "No reply recorded yet." }).text, "No reply recorded yet.");
});

import { clientSignals } from "../src/workbench.js";

test("client-side observer signals: context pressure and wrong branch", () => {
  const meters = { a: { percent: 84, tokens: 640000, window: 760000 }, b: { percent: 20 }, c: { percent: null, tokens: 9 } };
  const sessions = [{ id: "a", agent: "Codex" }];
  const s = clientSignals({ meters, sessions });
  assert.deepEqual(s.map((x) => [x.kind, x.key, x.title]), [["context", "context:a:80", "Codex context is 84% full"]]);
  assert.equal(clientSignals({ meters, sessions, muted: ["context"] }).length, 0);

  const spec = { id: "002", branch: "spec/002-x", stage: "build" };
  const git = { branch: "main", changes: [{ path: "a" }] };
  assert.equal(clientSignals({ git, spec })[0].kind, "branch");
  assert.equal(clientSignals({ git: { ...git, changes: [] }, spec }).length, 0, "nothing to protect");
  assert.equal(clientSignals({ git, spec: { ...spec, stage: "spec" } }).length, 0, "not building yet");
  assert.equal(clientSignals({ git: { ...git, branch: "spec/002-x" }, spec }).length, 0, "already there");
});

import { renderMarkdown, suggestCommitMessage } from "../src/workbench.js";

test("commit messages are suggested from what changed", () => {
  const spec = { id: "001", file: "specs/001-greet.md" };
  assert.equal(
    suggestCommitMessage(spec, [
      { path: "docs/project/BRIEF.md", code: "??" },
      { path: "specs/001-greet.md", code: "??" },
    ]),
    "spec:001 add the project brief and add the spec",
  );
  assert.equal(suggestCommitMessage(spec, [{ path: "src/greet.js", code: "M" }]), "spec:001 update greet.js");
  assert.equal(
    suggestCommitMessage(null, [
      { path: "a.js", code: "M" },
      { path: "b.js", code: "M" },
      { path: "c.js", code: "D" },
    ]),
    "update 3 files (a.js, b.js, …)",
  );
  assert.equal(suggestCommitMessage(spec, []), "spec:001 ");
});

test("markdown previews are escaped before anything is interpreted", () => {
  const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
  const html = renderMarkdown("# Title\n\nSome **bold** and `code`.\n\n- [ ] todo\n- one <script>x</script>\n\n```\n<b>raw</b>\n```\n[ok](https://example.com) [bad](javascript:alert(1))", esc);
  assert.ok(html.includes("<h1>Title</h1>"));
  assert.ok(html.includes("<b>bold</b>") && html.includes("<code>code</code>"));
  assert.ok(html.includes('<ul><li><span class="md-box" aria-hidden="true"></span>todo</li>'));
  assert.ok(!html.includes("<script>"), "markup in the file stays text");
  assert.ok(html.includes("<pre><code>&lt;b&gt;raw&lt;/b&gt;</code></pre>"));
  assert.ok(html.includes('<a href="https://example.com"'));
  assert.ok(!html.includes("javascript:alert(1)\""), "only http(s) links become links");
});
