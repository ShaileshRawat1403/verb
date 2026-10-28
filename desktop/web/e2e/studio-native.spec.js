import { test, expect } from "@playwright/test";
import { spawn, execFileSync } from "node:child_process";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  readFileSync,
  symlinkSync,
  linkSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createServer } from "node:net";

const TOKEN = "native-studio-fixture-000000000000000000000";
// Denied requests deliberately leave bodies unread (vendor/tiny_http/VERB_PATCH.md).
// Close those fixture connections rather than reusing an unread request stream.
const headers = { "X-Verb-Token": TOKEN, Connection: "close" };
const APP = `<!doctype html><meta charset="utf-8"><style>body{font:20px system-ui;padding:28px;background:#f6f1e7;color:#292d2c}button{padding:12px;font:inherit}</style><h1>Fieldnotes</h1><p>Choose a slower Sunday.</p><button id="count">Book a walk · 0</button><p id="parent-access"></p><p id="storage-access"></p><p id="api-access"></p><script>
let count=0;document.querySelector('#count').onclick=()=>document.querySelector('#count').textContent='Book a walk · '+(++count);
try{parent.document.body;document.querySelector('#parent-access').textContent='parent exposed'}catch{document.querySelector('#parent-access').textContent='parent blocked'}
try{localStorage.setItem('probe','x');document.querySelector('#storage-access').textContent='storage exposed'}catch{document.querySelector('#storage-access').textContent='storage blocked'}
fetch('/api/state').then(()=>document.querySelector('#api-access').textContent='API exposed').catch(()=>document.querySelector('#api-access').textContent='API blocked');
</script>`;
let repo, home, stateDir, origin, remoteOrigin, url, server, remote, nativeEnv;
const bin = resolve(process.env.VERB_BIN ?? "../target/debug/verb");
async function launch(env) {
  const port = await new Promise((ok) => {
    const s = createServer().listen(0, "127.0.0.1", () => {
      const p = s.address().port;
      s.close(() => ok(p));
    });
  });
  const child = spawn(bin, ["web", "--port", String(port)], {
    cwd: repo,
    env,
    stdio: ["ignore", "pipe", "pipe"],
  });
  await new Promise((ok, fail) => {
    const t = setTimeout(
      () => fail(new Error("native fixture did not start")),
      20_000,
    );
    child.stdout.on("data", (b) => {
      if (String(b).includes("Verb web UI")) {
        clearTimeout(t);
        ok();
      }
    });
    child.on("exit", () => {
      clearTimeout(t);
      fail(new Error("fixture exited"));
    });
  });
  return { child, origin: `http://127.0.0.1:${port}` };
}
test.beforeAll(async () => {
  const root = mkdtempSync(join(tmpdir(), "verb-studio-native-browser-"));
  repo = join(root, "project");
  home = join(root, "home");
  stateDir = join(root, "state");
  mkdirSync(repo);
  mkdirSync(home, { mode: 0o700 });
  nativeEnv = {
    PATH: process.env.PATH,
    HOME: home,
    LANG: "C",
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_CONFIG_GLOBAL: "/dev/null",
    VERB_TOKEN: TOKEN,
    VERB_STATE_DIR: stateDir,
  };
  for (const args of [
    ["init", "-q", "-b", "main"],
    ["config", "user.name", "Fixture"],
    ["config", "user.email", "fixture@example.invalid"],
  ])
    execFileSync("git", ["-C", repo, ...args], { env: nativeEnv });
  writeFileSync(
    join(repo, "README.md"),
    "# Native Studio\n\nAn isolated project.\n",
  );
  writeFileSync(join(repo, "app.html"), APP);
  writeFileSync(join(repo, "sample.png"), Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=", "base64"));
  writeFileSync(join(repo, "disguised.png"), '<svg onload="alert(1)"></svg>');
  writeFileSync(join(repo, "conflict.txt"), "original");
  writeFileSync(join(repo, "typing.txt"), "original");
  writeFileSync(join(repo, ".gitignore"), ".env\n");
  writeFileSync(join(repo, ".env"), "PRIVATE=fixture-only");
  writeFileSync(join(repo, "hard-target.txt"), "target");
  execFileSync(
    "git",
    [
      "-C",
      repo,
      "add",
      "README.md",
      "app.html",
      "conflict.txt",
      "typing.txt",
      ".gitignore",
      "hard-target.txt",
    ],
    { env: nativeEnv },
  );
  execFileSync("git", ["-C", repo, "commit", "-qm", "fixture"], {
    env: nativeEnv,
  });
  symlinkSync("app.html", join(repo, "linked.html"));
  linkSync(join(repo, "hard-target.txt"), join(repo, "hard.txt"));
  const native = await launch(nativeEnv);
  server = native.child;
  origin = native.origin;
  url = `${origin}/#${TOKEN}`;
  const gated = await launch({
    ...nativeEnv,
    VERB_DEPLOYMENT: "remote",
    VERB_STATE_DIR: join(root, "remote-state"),
  });
  remote = gated.child;
  remoteOrigin = gated.origin;
});
test.afterAll(() => {
  server?.kill();
  remote?.kill();
});
async function openFile(page, path) {
  await page.goto(url);
  await page.getByRole("button", { name: "Studio", exact: true }).click();
  await page.getByLabel("Canvas file", { exact: true }).selectOption(path);
  await expect(
    page.getByRole("button", { name: "Edit file", exact: true }),
  ).toBeEnabled();
  await page.getByRole("button", { name: "Edit file", exact: true }).click();
  await expect(
    page.getByLabel("File contents · UTF-8 text editor", { exact: true }),
  ).toBeVisible();
}
async function doc(request, path) {
  const r = await request.get(
    `${origin}/api/studio/file?path=${encodeURIComponent(path)}`,
    { headers },
  );
  expect(r.ok()).toBeTruthy();
  return r.json();
}

test("edit, explicit save, app interaction and Git review share one actual file", async ({
  page,
}) => {
  await openFile(page, "app.html");
  const editor = page.getByLabel("File contents · UTF-8 text editor", {
    exact: true,
  });
  await editor.fill(
    APP.replace("Choose a slower Sunday.", "Sunday 1 Nov · 2 hours · ₹850"),
  );
  await expect(
    page.getByRole("button", { name: "App preview", exact: true }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "Save file", exact: true }).click();
  await expect(page.locator("#studio-draft-state")).toContainText(
    "Saved on disk",
  );
  expect(readFileSync(join(repo, "app.html"), "utf8")).toContain(
    "Sunday 1 Nov · 2 hours · ₹850",
  );
  await page.getByRole("button", { name: "Document", exact: true }).click();
  await expect(page.locator("#studio-document")).toContainText(
    "Sunday 1 Nov · 2 hours · ₹850",
  );
  await page.getByRole("button", { name: "App preview", exact: true }).click();
  const frame = page.frameLocator("#studio-app-frame iframe");
  await expect(
    frame.getByRole("heading", { name: "Fieldnotes" }),
  ).toBeVisible();
  await frame.getByRole("button", { name: "Book a walk · 0" }).click();
  await expect(
    frame.getByRole("button", { name: "Book a walk · 1" }),
  ).toBeVisible();
  await expect(frame.locator("#parent-access")).toHaveText("parent blocked");
  await expect(frame.locator("#storage-access")).toHaveText("storage blocked");
  await expect(frame.locator("#api-access")).toHaveText("API blocked");
  const previewFrame = page.locator("#studio-app-frame iframe");
  await page.getByRole("button", { name: "Phone", exact: true }).click();
  await expect(page.getByRole("button", { name: "Phone", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator("#studio-viewport-label")).toHaveText("Phone · 390 px");
  expect(await previewFrame.evaluate((frame) => frame.getBoundingClientRect().width)).toBe(390);
  await expect(frame.locator("body")).toBeVisible();
  await page.getByRole("button", { name: "Tablet", exact: true }).click();
  expect(await previewFrame.evaluate((frame) => frame.getBoundingClientRect().width)).toBe(768);
  await page.getByRole("button", { name: "Fit", exact: true }).click();
  await expect(frame.getByRole("button", { name: "Book a walk · 1" })).toBeVisible();
  expect(
    await page.locator("#studio-app-frame iframe").getAttribute("sandbox"),
  ).toBe("allow-scripts");
  await expect(page.locator("#studio-app-caption")).toContainText(
    "saved snapshot",
  );
  await page
    .getByRole("button", { name: "Review Git changes", exact: true })
    .click();
  await expect(page.getByRole("region", { name: "Git changes preview", exact: true })).toBeVisible();
  await expect(page.locator("#commit-dialog")).toBeHidden();
  await expect(page.locator("#studio-changed-files")).toContainText("app.html");
  await page.getByRole("region", { name: "Git changes preview", exact: true }).getByRole("button", { name: "Commit changes…", exact: true }).click();
  await expect(page.locator("#commit-dialog")).toBeVisible();
  await expect(page.locator("#commit-dialog")).toContainText("app.html");
  await page
    .locator("#commit-dialog")
    .getByRole("button", { name: "Cancel", exact: true })
    .click();
  await page.getByRole("button", { name: "Sessions", exact: true }).click();
  await expect(page.locator("#studio-app-frame iframe")).toHaveCount(0);
});

test("outside edits cause a conflict, retain the draft, and support recoverable reload", async ({
  page,
}) => {
  await openFile(page, "conflict.txt");
  const editor = page.getByLabel("File contents · UTF-8 text editor", {
    exact: true,
  });
  await editor.fill("my unsaved draft");
  writeFileSync(join(repo, "conflict.txt"), "changed by another editor");
  await page.getByRole("button", { name: "Save file", exact: true }).click();
  await expect(page.locator("#studio-editor-conflict")).toBeVisible();
  await expect(editor).toHaveValue("my unsaved draft");
  expect(readFileSync(join(repo, "conflict.txt"), "utf8")).toBe(
    "changed by another editor",
  );
  await page
    .getByRole("button", { name: "Load disk version", exact: true })
    .click();
  await expect(editor).toHaveValue("changed by another editor");
  await page
    .getByRole("button", { name: "Restore previous draft", exact: true })
    .click();
  await expect(editor).toHaveValue("my unsaved draft");
  await page.getByRole("button", { name: "Save file", exact: true }).click();
  await expect(page.locator("#studio-draft-state")).toContainText(
    "Saved on disk",
  );
  expect(readFileSync(join(repo, "conflict.txt"), "utf8")).toBe(
    "my unsaved draft",
  );
});

test("typing during a save remains a newer unsaved draft", async ({ page }) => {
  await openFile(page, "typing.txt");
  let release, entered;
  const gate = new Promise((ok) => (release = ok)),
    started = new Promise((ok) => (entered = ok));
  await page.route("**/api/studio/file", async (route) => {
    if (route.request().method() === "POST") {
      entered();
      await gate;
    }
    await route.continue();
  });
  const editor = page.getByLabel("File contents · UTF-8 text editor", {
    exact: true,
  });
  await editor.fill("submitted version");
  await page.getByRole("button", { name: "Save file", exact: true }).click();
  await started;
  await editor.fill("newer edits");
  release();
  await expect(page.locator("#studio-editor-status")).toContainText(
    "newer edits remain unsaved",
  );
  expect(readFileSync(join(repo, "typing.txt"), "utf8")).toBe(
    "submitted version",
  );
  await expect(editor).toHaveValue("newer edits");
  await expect(
    page.getByRole("button", { name: "Save file", exact: true }),
  ).toBeEnabled();
});

test("workspace, revision, path, size and authentication checks protect actual writes", async ({
  request,
}) => {
  const read = await doc(request, "typing.txt");
  const payload = {
    workspaceId: read.workspaceId,
    path: "typing.txt",
    expectedRevision: read.file.revision,
    text: "not allowed",
  };
  expect(
    (
      await request.post(`${origin}/api/studio/file`, {
        headers: { Connection: "close" },
        data: payload,
      })
    ).status(),
  ).toBe(403);
  expect(
    (
      await request.post(`${origin}/api/studio/file`, {
        headers: { ...headers, Origin: "https://foreign.invalid" },
        data: payload,
      })
    ).status(),
  ).toBe(403);
  expect(
    (
      await request.post(`${origin}/api/studio/file`, {
        headers,
        data: { ...payload, workspaceId: "another-project" },
      })
    ).status(),
  ).toBe(409);
  expect(
    (
      await request.post(`${origin}/api/studio/file`, {
        headers,
        data: { ...payload, expectedRevision: "stale" },
      })
    ).status(),
  ).toBe(409);
  for (const path of ["../outside", ".env", "linked.html", "hard.txt"])
    expect(
      (
        await request.get(
          `${origin}/api/studio/file?path=${encodeURIComponent(path)}`,
          { headers },
        )
      ).status(),
    ).toBe(400);
  const large = "é".repeat(40_000);
  const saved = await request.post(`${origin}/api/studio/file`, {
    headers,
    data: { ...payload, text: large },
  });
  expect(saved.ok()).toBeTruthy();
  expect(readFileSync(join(repo, "typing.txt"), "utf8")).toBe(large);
  const tooLarge = await request.post(`${origin}/api/studio/file`, {
    headers,
    data: {
      ...payload,
      expectedRevision: (await saved.json()).file.revision,
      text: "x".repeat(200 * 1024 + 1),
    },
  });
  expect(tooLarge.status()).toBe(400);
  expect(readFileSync(join(repo, "typing.txt"), "utf8")).toBe(large);
});

test("preview tickets are scoped, single-use, iframe-only and carry HTTP sandboxing", async ({
  request,
}) => {
  const read = await doc(request, "app.html");
  const data = {
    workspaceId: read.workspaceId,
    path: "app.html",
    expectedRevision: read.file.revision,
  };
  expect(
    (
      await request.post(`${origin}/api/studio/preview`, {
        headers: { Connection: "close" },
        data,
      })
    ).status(),
  ).toBe(403);
  const minted = await request.post(`${origin}/api/studio/preview`, {
    headers,
    data,
  });
  expect(minted.ok()).toBeTruthy();
  const info = await minted.json();
  expect(info.url).not.toContain(TOKEN);
  expect((await request.get(`${origin}${info.url}`)).status()).toBe(403);
  const content = await request.get(`${origin}${info.url}`, {
    headers: { "Sec-Fetch-Dest": "iframe" },
  });
  expect(content.ok()).toBeTruthy();
  expect(content.headers()["content-security-policy"]).toContain(
    "sandbox allow-scripts",
  );
  expect(content.headers()["content-security-policy"]).toContain(
    "connect-src 'none'",
  );
  expect(content.headers()["content-security-policy"]).not.toContain(
    "allow-same-origin",
  );
  expect(content.headers()["set-cookie"]).toBeUndefined();
  expect(
    (
      await request.get(`${origin}${info.url}`, {
        headers: { "Sec-Fetch-Dest": "iframe" },
      })
    ).status(),
  ).toBe(410);
});

test("remote hosts remain read-only and cannot mint or serve Studio app previews", async ({
  request,
}) => {
  expect((await request.get(`${remoteOrigin}/api/browser`, { headers })).status()).toBe(403);
  expect((await request.post(`${remoteOrigin}/api/browser`, { headers, data: { op: "start", client: "remote-fixture" } })).status()).toBe(403);
  const cap = await request.get(`${remoteOrigin}/api/studio/workspace`, {
    headers,
  });
  expect(cap.ok()).toBeTruthy();
  const capabilities = await cap.json();
  expect(capabilities.editing.available).toBe(false);
  expect(capabilities.preview.available).toBe(false);
  expect(
    (
      await request.get(`${remoteOrigin}/api/studio/file?path=app.html`, {
        headers,
      })
    ).status(),
  ).toBe(403);
  expect(
    (
      await request.post(`${remoteOrigin}/api/studio/file`, {
        headers,
        data: {},
      })
    ).status(),
  ).toBe(403);
  expect(
    (
      await request.post(`${remoteOrigin}/api/studio/preview`, {
        headers,
        data: {},
      })
    ).status(),
  ).toBe(403);
  expect(
    (
      await request.get(`${remoteOrigin}/studio-preview/${"a".repeat(64)}`, {
        headers: { "Sec-Fetch-Dest": "iframe" },
      })
    ).status(),
  ).toBe(403);
  const shell = await request.get(remoteOrigin);
  expect(shell.headers()["content-security-policy"]).not.toContain(
    "frame-src 'self'",
  );
});

test("read-scoped clients cannot edit files or authorize script previews", async ({
  request,
}) => {
  const output = execFileSync(
    bin,
    [
      "token",
      "create",
      "--name",
      "studio-reader",
      "--scope",
      "read",
      "--expires",
      "1h",
    ],
    { cwd: repo, env: nativeEnv, encoding: "utf8" },
  );
  const token = output.match(/verb_at_[A-Za-z0-9_-]+/)[0];
  const reader = { "X-Verb-Token": token, Connection: "close" };
  expect((await request.get(`${origin}/api/browser`, { headers: reader })).status()).toBe(403);
  expect((await request.post(`${origin}/api/browser`, { headers: reader, data: { op: "start", client: "reader-fixture" } })).status()).toBe(403);
  const raster = await request.get(`${origin}/api/studio/image?path=sample.png`, { headers: reader });
  expect(raster.ok()).toBeTruthy();
  const cap = await request.get(`${origin}/api/studio/workspace`, {
    headers: reader,
  });
  expect(cap.ok()).toBeTruthy();
  expect((await cap.json()).editing.available).toBe(false);
  expect(
    (
      await request.get(`${origin}/api/studio/file?path=app.html`, {
        headers: reader,
      })
    ).ok(),
  ).toBeTruthy();
  expect(
    (
      await request.post(`${origin}/api/studio/file`, {
        headers: reader,
        data: {},
      })
    ).status(),
  ).toBe(403);
  expect(
    (
      await request.post(`${origin}/api/studio/preview`, {
        headers: reader,
        data: {},
      })
    ).status(),
  ).toBe(403);
});

test("phone editing keeps per-file drafts and previews only the explicitly saved file", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await openFile(page, "app.html");
  const editor = page.getByLabel("File contents · UTF-8 text editor", {
    exact: true,
  });
  const appDraft = (await editor.inputValue()).replace(
    "<h1>Fieldnotes</h1>",
    "<h1>Fieldnotes on a phone</h1>",
  );
  await editor.fill(appDraft);
  await page.getByRole("button", { name: "Current work", exact: true }).click();
  await page.getByRole("button", { name: "Canvas", exact: true }).click();
  await expect(editor).toHaveValue(appDraft);
  await page
    .getByLabel("Canvas file", { exact: true })
    .selectOption("conflict.txt");
  await page.getByRole("button", { name: "Edit file", exact: true }).click();
  await editor.fill("a second unsaved file draft");
  await page
    .getByLabel("Canvas file", { exact: true })
    .selectOption("app.html");
  await page.getByRole("button", { name: "Edit file", exact: true }).click();
  await expect(editor).toHaveValue(appDraft);
  await page.getByRole("button", { name: "Save file", exact: true }).click();
  await expect(page.locator("#studio-draft-state")).toContainText(
    "Saved on disk",
  );
  expect(readFileSync(join(repo, "conflict.txt"), "utf8")).not.toBe(
    "a second unsaved file draft",
  );
  await page.getByRole("button", { name: "App preview", exact: true }).click();
  await expect(
    page
      .frameLocator("#studio-app-frame iframe")
      .getByRole("heading", { name: "Fieldnotes on a phone", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(() => document.documentElement.scrollWidth),
  ).toBeLessThanOrEqual(390);
  await page.screenshot({
    path: test.info().outputPath("native-preview-phone.png"),
    fullPage: true,
  });
  await page.getByRole("button", { name: "Close preview", exact: true }).click();
  await expect(page.locator("#studio-app-frame iframe")).toHaveCount(0);
});


test("console tabs, image zoom and keyboard resize preserve editable drafts", async ({ page, request }) => {
  await page.goto(url);
  await page.getByRole("button", { name: "Studio", exact: true }).click();
  await expect(page.locator("#studio-document h1")).toHaveText("Native Studio");
  const work = await page.locator("#studio-work-panel").boundingBox();
  const preview = await page.locator("#studio-canvas-panel").boundingBox();
  expect(work.x).toBeLessThan(preview.x);
  const divider = page.getByRole("separator", { name: "Resize work and preview" });
  await divider.focus(); await divider.press("ArrowRight");
  await expect(divider).toHaveAttribute("aria-valuenow", "48");
  await page.getByLabel("Canvas file", { exact: true }).selectOption("sample.png");
  await expect(page.locator("#studio-image-state")).toContainText("1 × 1");
  await expect(page.getByRole("button", { name: "Edit file", exact: true })).toBeDisabled();
  await page.getByRole("button", { name: "Zoom image in", exact: true }).click();
  await expect(page.locator("#studio-image-scale")).toHaveText("125%");
  await page.getByRole("button", { name: "Fit image", exact: true }).click();
  await expect(page.locator("#studio-image-scale")).toHaveText("Fit");
  const response = await request.get(`${origin}/api/studio/image?path=disguised.png`, { headers });
  expect(response.ok()).toBe(false);
  await page.getByRole("button", { name: "Close current preview", exact: true }).click();
  await expect(page.getByRole("tab", { name: /sample.png/ })).toHaveCount(0);
  await page.getByLabel("Canvas file", { exact: true }).selectOption("app.html");
  await page.getByRole("tab", { name: /README.md/ }).click();
  await page.getByRole("tab", { name: /README.md/ }).press("ArrowRight");
  await expect(page.getByRole("tab", { name: /app.html/ })).toHaveAttribute("aria-selected", "true");
  await page.getByRole("button", { name: "Edit file", exact: true }).click();
  const editor = page.getByLabel("File contents · UTF-8 text editor", { exact: true });
  await expect(editor).toBeVisible(); await editor.fill("an unsaved tab draft");
  await page.getByRole("button", { name: "Close current preview", exact: true }).click();
  await expect(editor).toHaveValue("an unsaved tab draft");
  await expect(page.locator("#studio-message")).toContainText("Your text is retained");
});

test("preview dock reviews real Git diffs and expands without losing the work thread", async ({ page }, testInfo) => {
  await openFile(page, "app.html");
  await page.getByRole("button", { name: "Document", exact: true }).click();
  await page.getByRole("button", { name: "Show changes", exact: true }).click();
  await expect(page.locator("#studio-changed-files")).toContainText("app.html");
  await page.locator("#studio-changed-files").getByRole("button", { name: /app.html/ }).click();
  await expect(page.locator("#studio-diff")).toContainText("Fieldnotes");
  await page.getByRole("button", { name: "Back to preview", exact: true }).click();
  await page.getByRole("button", { name: "Expand preview", exact: true }).click();
  await expect(page.locator("#studio-work-panel")).not.toBeVisible();
  await page.getByRole("button", { name: "Restore split", exact: true }).press("Escape");
  await expect(page.locator("#studio-work-panel")).toBeVisible();
  await page.getByRole("button", { name: "Edit file", exact: true }).click();
  await page.getByLabel("File contents · UTF-8 text editor", { exact: true }).fill(`<!doctype html><html lang="en"><meta charset="utf-8"><style>body{margin:0;padding:38px;background:#f4f0e5;color:#243b32;font:16px/1.6 system-ui}small{text-transform:uppercase;letter-spacing:.16em;color:#68776b}h1{font:46px/1.1 Georgia,serif;letter-spacing:-1px;margin:32px 0 18px}p{max-width:340px}.details{border-top:1px solid #cbd1c5;padding:20px 0;margin:24px 0}button{border:0;border-radius:4px;padding:13px 19px;background:#284e3b;color:white;font:inherit}</style><small>Fieldnotes / local walks</small><h1>A slower Sunday.<br>A different way home.</h1><p>Small groups. Open air. Explore the familiar with fresh eyes.</p><div class="details">Sunday 1 Nov · 2 hours<br><strong>₹850 per person</strong></div><button id="book">See walk details</button><p id="details" hidden>Meet at the park gate at 9am. Comfortable shoes and curiosity welcome.</p><script>document.querySelector('#book').onclick=()=>document.querySelector('#details').hidden=false;</script></html>`);
  await page.getByRole("button", { name: "Save file", exact: true }).click();
  await expect(page.locator("#studio-draft-state")).toContainText("Saved on disk");
  if (await page.locator("#studio-intent-form").isHidden())
    await page.getByRole("button", { name: "Define another change", exact: true }).click();
  await page.getByLabel("Describe a new change", { exact: true }).fill("Make the walk booking clearer\nShow the date, duration and price before someone books.");
  await page.getByRole("button", { name: "Define change", exact: false }).click();
  await page.locator('#spec-dialog [name="criteria"]').fill("The date, duration and price are visible before booking");
  await page.locator('#spec-dialog button[type="submit"]').click();
  await expect(page.locator("#spec-dialog")).not.toBeVisible();
  await expect(page.locator("#studio-work-activity")).not.toBeEmpty();
  await page.getByRole("button", { name: "App preview", exact: true }).click();
  const frame = page.frameLocator("#studio-app-frame iframe");
  await expect(frame.getByRole("heading", { name: /A slower Sunday/ })).toBeVisible();
  await frame.getByRole("button", { name: "See walk details" }).click();
  await expect(frame.locator("#details")).toBeVisible();
  for (let step = 0; step < 3 && await page.locator("html").getAttribute("data-resolved-theme") !== "dark"; step++) await page.locator("#theme-toggle").click();
  await expect(page.locator("html")).toHaveAttribute("data-resolved-theme", "dark");
  await expect(page.locator(".toast.visible")).toHaveCount(0, { timeout: 8000 });
  await page.screenshot({ path: testInfo.outputPath("console-preview-desktop.png") });
  writeFileSync(join(repo, "walk-preview.png"), await page.locator("#studio-app-frame").screenshot());
  await page.getByRole("button", { name: "Refresh canvas", exact: true }).click();
  await expect(page.getByLabel("Canvas file", { exact: true }).locator('option[value="walk-preview.png"]')).toHaveCount(1);
  await page.getByLabel("Canvas file", { exact: true }).selectOption("walk-preview.png");
  await expect(page.locator("#studio-image-state")).toContainText("KB");
  await expect(page.locator("#studio-preview-image")).toBeVisible();
  const imageBounds = await page.locator("#studio-preview-image").boundingBox();
  const imageViewport = await page.locator(".studio-image-viewport").boundingBox();
  expect(imageBounds.height).toBeLessThanOrEqual(imageViewport.height);
  expect(imageBounds.width).toBeLessThanOrEqual(imageViewport.width);
  await page.screenshot({ path: testInfo.outputPath("console-image-desktop.png") });

});


test("Files and review stay in Studio and retain drafts on desktop and phone", async ({ page }) => {
  for (const width of [1440, 390]) {
    await page.setViewportSize({ width, height: 900 });
    await openFile(page, "typing.txt");
    await page.getByRole("button", { name: "Edit file", exact: true }).click();
    const editor = page.getByRole("textbox", { name: "File contents · UTF-8 text editor", exact: true });
    await editor.fill(`Keep my unsaved ${width} draft`);
    await page.getByRole("button", { name: "Files", exact: true }).click();
    await expect(page.locator("#view-studio")).toBeVisible();
    await expect(page.getByRole("searchbox", { name: "Find a canvas file", exact: true })).toBeFocused();
    await expect(page.locator("#studio-preview-content")).toBeVisible();
    await page.getByRole("button", { name: "Edit file", exact: true }).click();
    await expect(editor).toHaveValue(`Keep my unsaved ${width} draft`);
    await page.getByRole("button", { name: "Review Git changes", exact: true }).click();
    await expect(page.getByRole("region", { name: "Git changes preview", exact: true })).toBeFocused();
    await expect(page.locator("#commit-dialog")).toBeHidden();
    await page.getByRole("button", { name: "Files", exact: true }).click();
    await page.getByRole("button", { name: "Edit file", exact: true }).click();
    await expect(editor).toHaveValue(`Keep my unsaved ${width} draft`);
  }
});


test("Studio prioritises selected work, keeps details optional and preserves the next-change draft", async ({ page, request }) => {
  const response = await request.post(`${origin}/api/specs`, { headers, data: { title: "Review the reading experience", problem: "Make the task easy to understand without losing access to its tools.", criteria: ["A person can review the actual wording change"] } });
  const spec = await response.json();
  await request.post(`${origin}/api/specs/${spec.id}/stage`, { headers, data: { stage: "review", expectedRevision: spec.revision, note: "UX fixture, not a completion claim" } });
  for (const width of [1440, 390]) {
    await page.setViewportSize({ width, height: 900 });
    await page.goto(url);
    await page.getByRole("button", { name: "Studio", exact: true }).click();
    if (width === 390) await page.getByRole("button", { name: "Current work", exact: true }).click();
    await page.getByRole("combobox", { name: "Current work", exact: true }).selectOption(spec.id);
    await expect(page.locator("#studio-next-hint")).toHaveText("Inspect the actual changes before deciding what to keep.");
    await expect(page.locator("#studio-intent-form")).toBeHidden();
    if (width === 1440) await expect(page.locator("#studio-work-details")).not.toHaveAttribute("open", "");
    else if (await page.locator("#studio-work-details").evaluate(node => node.open))
      await page.getByText("Task details & acceptance criteria", { exact: true }).click();
    await expect(page.locator("#studio-history")).not.toHaveAttribute("open", "");
    await page.getByText("Task details & acceptance criteria", { exact: true }).click();
    await expect(page.locator("#studio-criteria")).toContainText("A person can review");
    await page.getByRole("button", { name: "Define another change", exact: true }).click();
    const draft = page.getByRole("textbox", { name: "Describe a new change", exact: true });
    await draft.fill(`Next idea for ${width}`);
    await page.getByRole("button", { name: "Hide new change", exact: true }).click();
    await page.getByRole("button", { name: "Define another change", exact: true }).click();
    await expect(draft).toHaveValue(`Next idea for ${width}`);
    await page.getByRole("button", { name: "Hide new change", exact: true }).click();
    await page.getByRole("button", { name: "Review changes", exact: true }).click();
    await expect(page.getByRole("region", { name: "Git changes preview", exact: true })).toBeVisible();
    await expect(page.locator("#commit-dialog")).toBeHidden();
    await expect(page.locator("#studio-changed-files button").first()).toBeVisible();
    await page.locator("#studio-changed-files button").first().click();
    await expect(page.locator("#studio-diff-path")).not.toHaveText("Select a changed file");
    if (width === 390) {
      expect((await page.locator(".studio-diff-wrap").boundingBox()).width).toBeGreaterThan(300);
      expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
    }
    await page.screenshot({ path: test.info().outputPath(`studio-task-review-${width}.png`) });
  }
});

test("Studio visual themes keep readable hierarchy and phone-sized controls", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" }); // Measure settled colours, not theme-transition interpolation.
  for (const width of [1440, 390]) {
    await page.setViewportSize({ width, height: 900 });
    // Seed a task through the normal form/API fixture so the hierarchy is exercised in isolation too.
    const created = await page.request.post(`${origin}/api/specs`, { headers, data: { title: "Make the workspace welcoming", problem: "Help someone understand the task and confidently choose their next action.", criteria: ["The next action is readable and reachable"] } });
    const spec = await created.json();
    await page.goto(url);
    await page.getByRole("button", { name: "Studio", exact: true }).click();
    if (width === 390) await page.getByRole("button", { name: "Current work", exact: true }).click();
    await page.getByRole("combobox", { name: "Current work", exact: true }).selectOption(spec.id);
    await expect(page.locator("#studio-next-action")).toBeVisible();
    for (const theme of ["dark", "light"]) {
      for (let i = 0; i < 3 && await page.locator("html").getAttribute("data-resolved-theme") !== theme; i++)
        await page.getByRole("button", { name: "Change theme", exact: true }).click();
      await expect(page.locator("html")).toHaveAttribute("data-resolved-theme", theme);
      expect(await page.locator("#studio-next-action").evaluate(node => getComputedStyle(node).transitionDuration)).toBe("0s");
      const contrast = await page.locator("#studio-next-hint, #studio-next-action, #studio-stage-label, #studio-conversation-empty").evaluateAll(nodes => {
        const luminance = color => {
          const channels = color.match(/[\d.]+/g).slice(0, 3).map(Number).map(v => v / 255).map(v => v <= .04045 ? v / 12.92 : ((v + .055) / 1.055) ** 2.4);
          return .2126 * channels[0] + .7152 * channels[1] + .0722 * channels[2];
        };
        return nodes.map(node => {
          let parent = node, bg;
          while (parent) {
            bg = getComputedStyle(parent).backgroundColor;
            if (bg !== "rgba(0, 0, 0, 0)" && bg !== "transparent") break;
            parent = parent.parentElement;
          }
          const a = luminance(getComputedStyle(node).color), b = luminance(bg);
          return { id: node.id, fg: getComputedStyle(node).color, bg, ratio: (Math.max(a, b) + .05) / (Math.min(a, b) + .05) };
        });
      });
      contrast.forEach(sample => expect(sample.ratio, `${theme} text contrast ${JSON.stringify(sample)}`).toBeGreaterThanOrEqual(4.5));
      if (width === 390) {
        expect((await page.locator("#studio-next-action").boundingBox()).height).toBeGreaterThanOrEqual(44);
        expect((await page.locator("#studio-thread-title").boundingBox()).y).toBeLessThan((await page.locator("#studio-new-change-toggle").boundingBox()).y);
        expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
      }
      await page.screenshot({ path: test.info().outputPath(`studio-finish-${theme}-${width}.png`) });
    }
  }
});

test("find and replace stays in the draft, supports undo and explicit save", async ({ page }) => {
  await openFile(page, "app.html");
  const editor = page.locator("#studio-editor-text");
  const disk = readFileSync(join(repo, "app.html"), "utf8");
  const draft = "Verb verb VERB\nsecond verb\n$& and a.*b";
  await editor.fill(draft);
  await editor.press("ControlOrMeta+f");
  const query = page.getByRole("textbox", { name: "Find in this file", exact: true });
  await expect(query).toBeFocused();
  await query.fill("verb");
  await expect(page.locator("#studio-find-count")).toHaveText("1 of 4");
  await query.press("Enter");
  await expect(page.locator("#studio-find-count")).toHaveText("2 of 4");
  await query.press("Shift+Enter");
  await expect(page.locator("#studio-find-count")).toHaveText("1 of 4");
  await page.getByRole("checkbox", { name: "Match case", exact: true }).check();
  await expect(page.locator("#studio-find-count")).toHaveText("1 of 2");
  await page.getByRole("checkbox", { name: "Match case", exact: true }).uncheck();
  await page.getByRole("textbox", { name: "Replace with", exact: true }).fill("$&");
  await page.getByRole("button", { name: "Replace all", exact: true }).click();
  await expect(editor).toHaveValue("$& $& $&\nsecond $&\n$& and a.*b");
  expect(readFileSync(join(repo, "app.html"), "utf8")).toBe(disk);
  await page.getByRole("button", { name: "Undo replacement", exact: true }).click();
  await expect(editor).toHaveValue(draft);
  await query.fill("a.*b");
  await page.getByRole("textbox", { name: "Replace with", exact: true }).fill("literal");
  await page.getByRole("button", { name: "Replace", exact: true }).click();
  await expect(editor).toHaveValue(draft.replace("a.*b", "literal"));
  await page.locator("#studio-find-panel").screenshot({ path: "/private/tmp/verb-find-desktop.png" });
  await editor.fill("newer edit must survive");
  await expect(page.getByRole("button", { name: "Undo replacement", exact: true })).toBeDisabled();
  await query.fill("missing");
  await expect(page.locator("#studio-find-count")).toHaveText("No matches");
  await expect(page.getByRole("button", { name: "Replace all", exact: true })).toBeDisabled();
  await query.press("Escape");
  await expect(page.locator("#studio-find-panel")).toBeHidden();
  await expect(editor).toBeFocused();
  await page.getByRole("button", { name: "Save file", exact: true }).click();
  await expect(page.locator("#studio-draft-state")).toContainText("Saved on disk");
  expect(readFileSync(join(repo, "app.html"), "utf8")).toBe("newer edit must survive");
});

test("file find fits a phone and undo cannot cross files", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await openFile(page, "app.html");
  const editor = page.locator("#studio-editor-text");
  await editor.fill("one one");
  await page.getByRole("button", { name: "Find & replace", exact: true }).click();
  await page.getByRole("textbox", { name: "Find in this file", exact: true }).fill("one");
  await page.getByRole("textbox", { name: "Replace with", exact: true }).fill("two");
  await page.getByRole("button", { name: "Replace all", exact: true }).click();
  await expect(editor).toHaveValue("two two");
  await expect(page.getByRole("button", { name: "Undo replacement", exact: true })).toBeEnabled();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  for (const button of await page.locator("#studio-find-panel button").all())
    expect(await button.evaluate((el) => el.getBoundingClientRect().height)).toBeGreaterThanOrEqual(44);
  await page.locator("#studio-find-panel").screenshot({ path: "/private/tmp/verb-find-phone.png" });
  await page.locator("#studio-file").selectOption("conflict.txt");
  await page.getByRole("button", { name: "Edit file", exact: true }).click();
  await expect(page.getByRole("button", { name: "Undo replacement", exact: true })).toBeDisabled();
  await page.locator("#studio-file").selectOption("app.html");
  await page.getByRole("button", { name: "Edit file", exact: true }).click();
  await expect(editor).toHaveValue("two two");
  await editor.fill(readFileSync(join(repo, "app.html"), "utf8"));
});


test("disk reload clears replacement undo and oversized replacement retains the draft", async ({ page }) => {
  await openFile(page, "typing.txt");
  const editor = page.locator("#studio-editor-text");
  await editor.fill("one one");
  await page.getByRole("button", { name: "Find & replace", exact: true }).click();
  await page.getByRole("textbox", { name: "Find in this file", exact: true }).fill("one");
  const replacement = page.getByRole("textbox", { name: "Replace with", exact: true });
  await replacement.fill("two");
  await page.getByRole("button", { name: "Replace all", exact: true }).click();
  await page.getByRole("button", { name: "Save file", exact: true }).click();
  await expect(page.locator("#studio-draft-state")).toContainText("Saved on disk");
  await expect(page.getByRole("button", { name: "Undo replacement", exact: true })).toBeEnabled();
  await page.getByRole("button", { name: "Load disk version", exact: true }).click();
  await expect(page.locator("#studio-editor-status")).toHaveText("Disk version loaded.");
  await expect(page.getByRole("button", { name: "Undo replacement", exact: true })).toBeDisabled();
  await expect(editor).toHaveValue("two two");
  await editor.fill("a".repeat(204800));
  await page.getByRole("textbox", { name: "Find in this file", exact: true }).fill("a");
  await replacement.fill("too large");
  await page.getByRole("button", { name: "Replace all", exact: true }).click();
  await expect(page.locator("#studio-find-count")).toContainText("exceed the 200 KB");
  await expect(editor).toHaveValue("a".repeat(204800));
  expect(readFileSync(join(repo, "typing.txt"), "utf8")).toBe("two two");
  await editor.fill("two two");
});

for (const width of [1440, 390]) test(`a person commits chosen files at ${width}px and stale review never commits newer bytes`, async ({ page, request }) => {
  await page.setViewportSize({ width, height: 900 });
  const finished = `finished-${width}.txt`, experiment = `experiment-${width}.txt`;
  writeFileSync(join(repo, finished), "ready for review\n");
  writeFileSync(join(repo, experiment), "unfinished\n");
  await page.goto(url);
  await page.keyboard.press("Alt+KeyC");
  const dialog = page.locator("#commit-dialog");
  await expect(dialog.locator(`input[data-commit-path="${finished}"]`)).toBeVisible();
  for (const input of await dialog.locator("input[data-commit-path]:enabled").all()) await input.uncheck();
  await expect(page.locator("#commit-dialog-submit")).toBeDisabled();
  await dialog.locator(`input[data-commit-path="${finished}"]`).check();
  await expect(page.locator("#commit-selection-count")).toContainText("1 of");
  await dialog.locator(`li:has(input[data-commit-path="${finished}"])`).getByRole("button", { name: "View diff" }).click();
  await expect(page.locator("#commit-diff-text")).toContainText("ready for review");
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  if (width === 390) {
    expect(await page.locator("#commit-dialog-submit").evaluate(el => el.getBoundingClientRect().height)).toBeGreaterThanOrEqual(44);
    await dialog.screenshot({ path: "/private/tmp/verb-select-phone.png" });
  }
  await page.locator("#commit-message").fill("finish one file");
  await page.locator("#commit-dialog-submit").click();
  await expect(dialog).toBeHidden();
  const tree = execFileSync("git", ["-C", repo, "show", "--format=", "--name-only", "HEAD"], { env: nativeEnv, encoding: "utf8" });
  expect(tree.trim()).toBe(finished);
  expect(readFileSync(join(repo, experiment), "utf8")).toBe("unfinished\n");
  expect(execFileSync("git", ["-C", repo, "diff", "--cached", "--name-only"], { env: nativeEnv, encoding: "utf8" }).trim()).toBe("");
  const review = await request.get(`${origin}/api/git/commit-review`, { headers }).then(r => r.json());
  writeFileSync(join(repo, experiment), "newer content\n");
  const before = execFileSync("git", ["-C", repo, "rev-parse", "HEAD"], { env: nativeEnv, encoding: "utf8" });
  const result = await request.post(`${origin}/api/git/commit`, { headers, data: { message: "stale", selection: { version: review.version, paths: [experiment] } } });
  expect(result.ok()).toBe(false);
  expect(execFileSync("git", ["-C", repo, "rev-parse", "HEAD"], { env: nativeEnv, encoding: "utf8" })).toBe(before);
});
