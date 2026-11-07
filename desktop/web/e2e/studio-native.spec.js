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
  expect(
    await page.locator("#studio-app-frame iframe").getAttribute("sandbox"),
  ).toBe("allow-scripts");
  await expect(page.locator("#studio-app-caption")).toContainText(
    "saved snapshot",
  );
  await page
    .getByRole("button", { name: "Review Git changes", exact: true })
    .click();
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
  await page.getByRole("button", { name: "Stop preview", exact: true }).click();
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
