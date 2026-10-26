import { test, expect } from "@playwright/test";
import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createServer } from "node:net";

const TOKEN = "studio-fixture-0000000000000000000000000";
let server, repo, origin, url;
test.beforeAll(async () => {
  const root = mkdtempSync(join(tmpdir(), "verb-studio-"));
  repo = join(root, "repo");
  const home = join(root, "home");
  mkdirSync(repo);
  mkdirSync(home, { mode: 0o700 });
  const env = {
    PATH: process.env.PATH,
    HOME: home,
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_CONFIG_GLOBAL: "/dev/null",
    LANG: "C",
  };
  for (const args of [
    ["init", "-q", "-b", "main"],
    ["config", "user.name", "Studio Fixture"],
    ["config", "user.email", "fixture@example.invalid"],
  ])
    execFileSync("git", ["-C", repo, ...args], { env });
  writeFileSync(
    join(repo, "README.md"),
    "# Fieldnotes\n\nSmall groups. Open air.\n\nA project for local walks.\n",
  );
  writeFileSync(
    join(repo, "index.html"),
    '<script>document.title="unsafe-preview"</script><h1>Untrusted project HTML</h1>',
  );
  writeFileSync(join(repo, ".gitignore"), ".env\n");
  writeFileSync(join(repo, ".env"), "TEST_SECRET=never-render\n");
  execFileSync(
    "git",
    ["-C", repo, "add", "README.md", "index.html", ".gitignore"],
    { env },
  );
  execFileSync("git", ["-C", repo, "commit", "-qm", "fixture"], { env });
  const port = await new Promise((ok) => {
    const listener = createServer().listen(0, "127.0.0.1", () => {
      const port = listener.address().port;
      listener.close(() => ok(port));
    });
  });
  origin = `http://127.0.0.1:${port}`;
  url = `${origin}/#${TOKEN}`;
  server = spawn(
    resolve(process.env.VERB_BIN ?? "../target/debug/verb"),
    ["web", "--port", String(port)],
    {
      cwd: repo,
      env: { ...env, VERB_TOKEN: TOKEN, VERB_STATE_DIR: join(root, "state") },
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  await new Promise((ok, fail) => {
    const timer = setTimeout(
      () => fail(new Error("Studio fixture did not start")),
      20_000,
    );
    server.stdout.on("data", (chunk) => {
      if (String(chunk).includes("Verb web UI")) {
        clearTimeout(timer);
        ok();
      }
    });
    server.on("exit", () => {
      clearTimeout(timer);
      fail(new Error("Studio fixture exited"));
    });
  });
});
test.afterAll(() => server?.kill());
async function openStudio(page) {
  await page.goto(url);
  await page.getByRole("button", { name: "Studio", exact: true }).click();
  await expect(page.locator("#studio-document h1")).toHaveText("Fieldnotes");
}

test("Studio reads the workspace without launching work or executing project HTML", async ({
  page,
}) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  const writes = [];
  page.on("request", (request) => {
    if (request.method() === "POST") writes.push(request.url());
  });
  await openStudio(page);
  expect(
    await page.locator("#studio-file option").allTextContents(),
  ).not.toContain(".env");
  await expect(page.locator("#studio-document-meta")).toContainText(
    "read-only",
  );
  await page.getByRole("button", { name: "Source", exact: true }).click();
  await expect(page.locator("#studio-document")).toContainText("# Fieldnotes");
  await page
    .getByLabel("Canvas file", { exact: true })
    .selectOption("index.html");
  await expect(page.locator("#studio-document")).toContainText(
    '<script>document.title="unsafe-preview"</script>',
  );
  await expect(
    page.locator("#studio-document script, #studio-document iframe"),
  ).toHaveCount(0);
  await expect(page).toHaveTitle("Verb — Project workbench");
  expect(errors).toEqual([]);
  expect(writes).toEqual([]);
});

test("Studio intent stays a draft until explicit Spec creation and shares the workbench identity", async ({
  page,
  request,
}) => {
  await openStudio(page);
  const direction = await page
    .getByRole("button", { name: "Define change", exact: false })
    .boundingBox();
  expect(direction.y + direction.height).toBeLessThanOrEqual(900);
  const intent =
    "Show the walk date before booking\nExplain duration and price.";
  await page.getByLabel("Describe a new change", { exact: true }).fill(intent);
  await page
    .getByRole("button", { name: "Define change", exact: false })
    .click();
  await expect(page.locator("#spec-dialog")).toBeVisible();
  await expect(page.locator("#spec-title-input")).toHaveValue(
    intent.split("\n")[0],
  );
  await expect(page.locator("#spec-problem")).toHaveValue(intent);
  await page
    .locator("#spec-dialog")
    .getByRole("button", { name: "Cancel", exact: true })
    .click();
  await page.getByRole("button", { name: "Work board", exact: true }).click();
  await page.getByRole("button", { name: "Studio", exact: true }).click();
  await expect(page.locator("#studio-intent")).toHaveValue(intent);
  const before = await request.get(`${origin}/api/specs`, {
    headers: { "X-Verb-Token": TOKEN },
  });
  expect((await before.json()).specs).toHaveLength(0);
  await page
    .getByRole("button", { name: "Define change", exact: false })
    .click();
  await page
    .locator('#spec-dialog [name="criteria"]')
    .fill("Date is shown before the booking action");
  await page.locator('#spec-dialog button[type="submit"]').click();
  await expect(page.locator("#spec-dialog")).not.toBeVisible();
  await expect(page.locator("#view-studio")).toHaveClass(/active/);
  await expect(page.locator("#studio-intent")).toHaveValue("");
  await page.getByRole("button", { name: "Studio", exact: true }).click();
  await expect(page.locator("#studio-thread-title")).toHaveText(
    intent.split("\n")[0],
  );
  await expect(page.locator("#studio-thread-problem")).toHaveText(intent);
  await expect(page.locator("#studio-criteria")).toContainText(
    "Evidence required",
  );
  const selected = await page.locator("#studio-thread").inputValue();
  await page
    .getByRole("button", { name: "Inspect work & evidence", exact: false })
    .click();
  await expect(page.locator("#spec-title")).toContainText(
    intent.split("\n")[0],
  );
  const listing = await request.get(`${origin}/api/specs`, {
    headers: { "X-Verb-Token": TOKEN },
  });
  expect((await listing.json()).specs[0].id).toBe(selected);
});

test("Studio failed work reads expose unavailability instead of stale facts", async ({
  page,
}) => {
  await page.route("**/api/specs", (route) =>
    route.fulfill({
      status: 500,
      json: { error: "Could not read project work" },
    }),
  );
  await openStudio(page);
  await expect(page.locator("#studio-thread-error")).toHaveText(
    "Could not read project work",
  );
  await expect(page.locator("#studio-spec-context")).toBeHidden();
  await expect(page.locator("#studio-inspect-work")).toBeDisabled();
});

test("phone Studio switches surfaces and preserves direction through navigation", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await openStudio(page);
  await page.getByRole("button", { name: "Current work", exact: true }).click();
  await expect(page.locator("#studio-canvas-panel")).toBeHidden();
  await page
    .getByLabel("Describe a new change", { exact: true })
    .fill("Keep my direction while I inspect files");
  await page.getByRole("button", { name: "Canvas", exact: true }).click();
  await expect(page.locator("#studio-work-panel")).toBeHidden();
  await page.getByRole("button", { name: "Files", exact: true }).click();
  await page.getByRole("button", { name: "Studio", exact: true }).click();
  await page.getByRole("button", { name: "Current work", exact: true }).click();
  await expect(page.locator("#studio-intent")).toHaveValue(
    "Keep my direction while I inspect files",
  );
  for (const width of [390, 320]) {
    await page.setViewportSize({ width, height: 844 });
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth),
    ).toBeLessThanOrEqual(width);
    for (const surface of ["Canvas", "Current work"]) {
      await page.getByRole("button", { name: surface, exact: true }).click();
      for (const control of await page
        .locator(
          "#view-studio button, #view-studio select, #studio-file-search",
        )
        .all()) {
        if (!(await control.isVisible())) continue;
        const bounds = await control.boundingBox();
        expect(bounds.height).toBeGreaterThanOrEqual(44);
        expect(bounds.x).toBeGreaterThanOrEqual(0);
        expect(bounds.x + bounds.width).toBeLessThanOrEqual(width);
      }
    }
  }
  await page.screenshot({
    path: test.info().outputPath("studio-phone.png"),
    fullPage: true,
  });
});

test("Studio file search retains the open document and shows no-match results truthfully", async ({
  page,
}) => {
  await openStudio(page);
  await page
    .getByLabel("Find a canvas file", { exact: true })
    .fill("index html");
  await expect(page.locator("#studio-file-note")).toContainText(
    "1 matching file",
  );
  await expect(page.locator("#studio-document h1")).toHaveText("Fieldnotes");
  await page
    .getByLabel("Canvas file", { exact: true })
    .selectOption("index.html");
  await expect(page.locator("#studio-document")).toContainText(
    "Untrusted project HTML",
  );
  await expect(
    page.getByRole("button", { name: "Source", exact: true }),
  ).toBeDisabled();
  await page
    .getByLabel("Find a canvas file", { exact: true })
    .fill("nothing-matches-this");
  await expect(page.locator("#studio-file-note")).toContainText(
    "0 matching files",
  );
  await expect(page.getByLabel("Canvas file", { exact: true })).toHaveValue(
    "index.html",
  );
  await expect(page.locator("#studio-document")).toContainText(
    "Untrusted project HTML",
  );
});

test("selected document text attaches as reviewable context and survives cancelling the Spec form", async ({
  page,
}) => {
  await openStudio(page);
  await page
    .locator("#studio-document")
    .getByText("Small groups. Open air.", { exact: true })
    .dblclick({ position: { x: 20, y: 10 } });
  const selection = await page.evaluate(() =>
    document.getSelection().toString().trim(),
  );
  expect(selection.length).toBeGreaterThan(0);
  const attach = page.getByRole("button", {
    name: "Use selected text",
    exact: true,
  });
  await expect(attach).toBeEnabled();
  await attach.focus();
  await page.keyboard.press("Enter");
  await expect(page.locator("#studio-context-path")).toHaveText("README.md");
  await expect(page.locator("#studio-context-text")).toHaveText(selection);
  await page
    .getByLabel("Describe a new change", { exact: true })
    .fill("Make this introduction clearer");
  await page
    .getByRole("button", { name: "Define change", exact: false })
    .click();
  await expect(page.locator("#spec-problem")).toHaveValue(
    `Make this introduction clearer\n\nContext from README.md:\n${selection}`,
  );
  await page
    .locator("#spec-dialog")
    .getByRole("button", { name: "Cancel", exact: true })
    .click();
  await expect(page.locator("#studio-context")).toBeVisible();
  await page
    .getByRole("button", { name: "Remove attached context", exact: true })
    .click();
  await expect(page.locator("#studio-context")).toBeHidden();
  await expect(
    page.getByLabel("Describe a new change", { exact: true }),
  ).toHaveValue("Make this introduction clearer");
  await page
    .locator("#studio-document")
    .getByText("Small groups. Open air.", { exact: true })
    .dblclick({ position: { x: 20, y: 10 } });
  await attach.click();
  await page
    .getByRole("button", { name: "Define change", exact: false })
    .click();
  await page
    .locator("#spec-criteria")
    .fill("The introduction explains the booking outcome");
  await page.locator("#spec-dialog-submit").click();
  await expect(page.locator("#studio-context")).toBeHidden();
  await expect(page.locator("#studio-intent")).toHaveValue("");
  await expect(page.locator("#studio-thread-problem")).toContainText(
    `Context from README.md:\n${selection}`,
  );
});
