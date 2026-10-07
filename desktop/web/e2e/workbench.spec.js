// The spec-driven workbench, driven like a person would: through the browser, against the real host.
import { test, expect } from "@playwright/test";
import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createServer } from "node:net";

const TOKEN = "e2e-token-0000000000000000000000000000";
let server;
let repo;
let url;

const git = (...args) => execFileSync("git", ["-C", repo, ...args], { encoding: "utf8" });
const freePort = () =>
  new Promise((ok) => {
    const s = createServer().listen(0, "127.0.0.1", () => {
      const { port } = s.address();
      s.close(() => ok(port));
    });
  });

test.beforeAll(async () => {
  const bin = resolve(process.env.VERB_BIN ?? "../target/debug/verb");
  repo = mkdtempSync(join(tmpdir(), "verb-e2e-"));
  execFileSync("git", ["init", "-q", "-b", "main", repo]);
  git("config", "user.name", "E2E Person");
  git("config", "user.email", "e2e@example.com");
  writeFileSync(join(repo, "README.md"), "# e2e\n");
  writeFileSync(join(repo, ".gitignore"), ".env\n");
  writeFileSync(join(repo, ".env"), "SECRET_TOKEN=do-not-show\n");
  writeFileSync(join(repo, "AGENTS.md"), "# Team rules\n\nUse tabs.\n");
  git("add", "-A");
  git("commit", "-qm", "initial");
  const port = await freePort();
  url = `http://127.0.0.1:${port}/#${TOKEN}`;
  server = spawn(bin, ["web", "--port", String(port)], {
    cwd: repo,
    env: { ...process.env, VERB_TOKEN: TOKEN },
    stdio: ["ignore", "pipe", "pipe"],
  });
  await new Promise((ok, fail) => {
    const timer = setTimeout(() => fail(new Error("verb web did not start")), 20_000);
    server.stdout.on("data", (chunk) => {
      if (String(chunk).includes("Verb web UI")) {
        clearTimeout(timer);
        ok();
      }
    });
    server.on("exit", (code) => fail(new Error(`verb web exited with ${code}`)));
  });
});

test.afterAll(() => server?.kill());

test("a spec goes from idea to a committed, audited change", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(url);

  // Specs is the home screen and explains the workflow when empty.
  await expect(page.getByRole("heading", { name: "Start with a spec" })).toBeVisible();

  // Create a spec through the guided form.
  await page.getByRole("button", { name: "＋ Write your first spec" }).click();
  await page.locator("#spec-title-input").fill("Let people sign in with email");
  await page.locator("#spec-problem").fill("Returning customers cannot sign in.");
  await page.locator("#spec-criteria").fill("A sign-in link arrives\nA bad email shows a message");
  await page.getByRole("button", { name: "Create spec" }).click();
  await expect(page.locator("#spec-title")).toHaveText("Let people sign in with email");
  await expect(page.locator(".spec-item.current")).toContainText("0/2 proven");

  // Prove a criterion: Verb asks for evidence and records it.
  await page.locator('[data-criterion="1"]').click({ force: true });
  await expect(page.getByRole("heading", { name: "How do you know it works?" })).toBeVisible();
  await page.locator("#evidence-text").fill("unit test rejects a@");
  await page.getByRole("button", { name: "Mark as proven" }).click();
  await expect(page.locator(".spec-item.current")).toContainText("1/2 proven");
  await expect(page.locator("#audit-list li").first()).toContainText("evidence: unit test rejects a@");

  // Skipping stages is allowed, warned about, and shown as skipped.
  await page.locator('#stage-bar [data-stage="build"]').click();
  await expect(page.locator("#stage-warnings")).toContainText("skipped plan");
  await page.getByRole("button", { name: "Move to Build" }).click();
  await expect(page.locator("#stage-bar li.skipped")).toHaveCount(1);

  // Commit from the dialog; the audit line lands in the same commit.
  await page.keyboard.press("Alt+KeyC");
  await expect(page.locator("#commit-message")).toHaveValue(/^spec:001 /);
  await page.locator("#commit-message").fill("spec:001 add the sign-in spec");
  await page.getByRole("button", { name: "Commit", exact: true }).click();
  await expect(page.locator("#git-status")).toHaveText("All changes saved");
  expect(git("log", "-1", "--format=%s").trim()).toBe("spec:001 add the sign-in spec");
  const file = readFileSync(join(repo, "specs/001-let-people-sign-in-with-email.md"), "utf8");
  expect(file).toContain("committed: spec:001 add the sign-in spec");
  expect(git("status", "--porcelain").trim()).toBe("");

  expect(errors).toEqual([]);
});

test("palette, theme and a terminal in the spec", async ({ page }) => {
  await page.goto(url);
  await expect(page.locator("#spec-title")).toBeVisible();

  // The command palette finds and runs actions.
  await page.locator(".palette-hint").click();
  await page.locator("#palette-input").fill("go to overview");
  await page.keyboard.press("Enter");
  await expect(page.locator("#view-overview")).toHaveClass(/active/);
  await page.keyboard.press("Alt+Digit1");
  await expect(page.locator("#view-specs")).toHaveClass(/active/);

  // The theme toggle cycles and the page follows.
  const resolved = () => page.evaluate(() => document.documentElement.dataset.resolvedTheme);
  await page.locator("#theme-toggle").click(); // system → light
  expect(await resolved()).toBe("light");
  await page.locator("#theme-toggle").click(); // light → dark
  expect(await resolved()).toBe("dark");

  // A terminal opened from the spec appears in the spec's work area.
  await page.locator('[data-action="spec-terminal"]').click();
  await expect(page.locator("#spec-work .terminal-tile")).toHaveCount(1, { timeout: 15_000 });
  await expect(page.locator("#spec-work .work-hint")).toBeHidden();
});

test("project view: files, preview, brief, and agent context sync", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(url);
  await page.keyboard.press("Alt+Digit2");
  await expect(page.locator("#view-project")).toHaveClass(/active/);

  // The tree shows what Git tracks or would track, and never ignored files.
  await expect(page.locator('#file-tree [data-file="README.md"]')).toBeVisible();
  await expect(page.locator('#file-tree [data-file=".env"]')).toHaveCount(0);
  await page.locator("#file-filter").fill("env");
  await expect(page.locator('#file-tree [data-file=".env"]')).toHaveCount(0);
  await page.locator("#file-filter").fill("");

  // Read-only preview with line numbers.
  await page.locator('#file-tree [data-file="README.md"]').click();
  await expect(page.locator("#preview-path")).toHaveText("README.md");
  await expect(page.locator(".code-lines li")).toHaveText(["# e2e"]);

  // Nudges point at the next useful step; nothing is forced.
  await expect(page.locator("#hub-nudges")).toContainText("project brief");
  await page.locator('[data-nudge="write-brief"]').click();
  await page.locator("#brief-problem").fill("Shops lose returning customers.");
  await page.locator("#brief-users").fill("Returning shoppers");
  await page.getByRole("button", { name: "Create brief" }).click();
  await expect(page.locator("#preview-path")).toHaveText("docs/project/BRIEF.md");
  await expect(page.locator("#brief-progress")).toHaveText("2/5 filled");

  // Sync writes Verb's section and keeps the user's own text.
  await page.locator('#view-project [data-action="sync-agents"]').click();
  await expect(page.locator('#agent-files li[data-state="current"]')).toHaveCount(2);
  const agents = readFileSync(join(repo, "AGENTS.md"), "utf8");
  expect(agents.startsWith("# Team rules\n\nUse tabs.\n")).toBe(true);
  expect(agents).toContain("**Problem:** Shops lose returning customers.");
  expect(readFileSync(join(repo, "docs/project/BRIEF.md"), "utf8")).toContain("synced agent context into AGENTS.md and CLAUDE.md");

  // The palette finds files.
  await page.locator(".palette-hint").click();
  await page.locator("#palette-input").fill("brief");
  await expect(page.locator("#palette-list")).toContainText("Open file docs/project/BRIEF.md");
  await page.keyboard.press("Escape");
  expect(errors).toEqual([]);
});
