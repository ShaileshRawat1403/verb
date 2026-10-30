import { test, expect, chromium } from "@playwright/test";
import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { createServer as httpServer } from "node:http";
import { resolve, join } from "node:path";
import { tmpdir } from "node:os";
const TOKEN = "companion-fixture-owner-000000000000000";
let host, peer, base, site, target, session, view;
const freePort = () =>
  new Promise((r) => {
    const s = createServer().listen(0, "127.0.0.1", () => {
      const p = s.address().port;
      s.close(() => r(p));
    });
  });
const headers = {
  "X-Verb-Token": TOKEN,
  "Content-Type": "application/json",
  Connection: "close",
};
async function owner(path, body) {
  const response = await fetch(base + path, {
    method: body ? "POST" : "GET",
    headers,
    ...(body ? { body: JSON.stringify(body) } : {}),
  });
  return { status: response.status, body: await response.json() };
}
async function action(op, extra = {}) {
  const r = await owner("/api/browser", {
    op,
    client: "companion-test-client",
    session,
    generation: view?.generation,
    ...extra,
  });
  expect(r.status, JSON.stringify(r.body)).toBe(200);
  expect(r.body.error, JSON.stringify(r.body)).toBeFalsy();
  view = r.body;
  session = view.session;
  return view;
}
test.describe.configure({ mode: "serial" });
test.beforeAll(async () => {
  const root = mkdtempSync(join(tmpdir(), "verb-companion-e2e-")),
    repo = join(root, "project"),
    home = join(root, "home");
  mkdirSync(repo);
  mkdirSync(home);
  execFileSync("git", ["init", "-q", repo]);
  writeFileSync(join(repo, "README.md"), "# Companion fixture\n");
  target = httpServer((req, res) => {
    res.writeHead(200, {
      "Content-Type": "text/html",
      "X-Frame-Options": "DENY",
    });
    res.end(
      '<!doctype html><title>Companion page</title><h1>Browser runs on the Mac</h1><button style="position:absolute;left:20px;top:90px" onclick="this.textContent=\'Passed\'">Click me</button>',
    );
  });
  await new Promise((r) => target.listen(0, "127.0.0.1", r));
  site = `http://127.0.0.1:${target.address().port}`;
  const port = await freePort();
  base = `http://127.0.0.1:${port}`;
  // No browser executable, worker or Node configuration on the host; external mode refuses local browsing.
  host = spawn(
    resolve(process.env.VERB_BIN ?? "../target/debug/verb"),
    ["web", "--port", String(port)],
    {
      cwd: repo,
      env: {
        PATH: process.env.PATH,
        HOME: home,
        VERB_TOKEN: TOKEN,
        VERB_STATE_DIR: join(root, "state"),
        VERB_ALLOWED_ORIGIN: "https://public-verb.example.invalid",
      },
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  await new Promise((ok, fail) => {
    const timer = setTimeout(() => fail(Error("Host did not start")), 20000);
    host.stdout.on("data", (c) => {
      if (String(c).includes("Verb web UI")) {
        clearTimeout(timer);
        ok();
      }
    });
    host.on("exit", () => fail(Error("Host exited")));
  });
});
test.afterAll(async () => {
  await owner("/api/browser/pairing", { op: "revoke" }).catch(() => {});
  peer?.kill("SIGTERM");
  host?.kill("SIGTERM");
  target?.closeAllConnections();
  target?.close();
});

test("external hosts cannot start local Chromium and pairing is owner-only and one-use", async () => {
  expect((await owner("/api/browser")).status).toBe(403);
  const wrong = await fetch(base + "/api/browser/pairing", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ op: "pair" }),
  });
  expect(wrong.status).toBe(403);
  const pairing = await owner("/api/browser/pairing", { op: "pair" });
  expect(pairing.status).toBe(200);
  expect(pairing.body.code).toMatch(/^[a-f0-9]{64}$/);
  const bad = await fetch(base + "/api/browser/companion/claim", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ op: "claim", code: "wrong" }),
  });
  expect(bad.status).toBe(403);
  let output = "",
    diagnostics = "";
  peer = spawn(
    process.execPath,
    [
      resolve("runtime/browser-companion.mjs"),
      "--server",
      base,
      "--chromium",
      chromium.executablePath(),
    ],
    { stdio: ["pipe", "pipe", "pipe"] },
  );
  peer.stdout.on("data", (b) => {
    output += b;
  });
  peer.stderr.on("data", (b) => {
    diagnostics += b;
  });
  peer.stdin.end(pairing.body.code + "\n");
  await expect
    .poll(() => output, {
      timeout: 15000,
      message: "Companion should connect without exposing credentials",
    })
    .toContain("Mac browser connected");
  const reused = await fetch(base + "/api/browser/companion/claim", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ op: "claim", code: pairing.body.code }),
  });
  expect(reused.status).toBe(403);
  expect((await owner("/api/browser")).body.location).toBe(
    "Paired Mac companion",
  );
});

test("real browser commands cross the relay without giving the companion general API access", async () => {
  const v = await action("start");
  expect(v.running).toBe(true);
  expect(v.image.length).toBeGreaterThan(100);
  await action("open", { url: site, allow: true });
  await action("click", { x: 45, y: 100 });
  expect((await action("inspect")).text).toContain("Passed");
  const forbidden = await owner("/api/browser", {
    op: "open",
    client: "companion-test-client",
    session: "old-session",
    url: site,
    allow: true,
    generation: view.generation,
  });
  expect(forbidden.body.error).toContain("session changed");
  // A browser-only peer key never substitutes for an owner token, even if presented that way.
  const oldSession = session;
  await action("stop");
  await action("start");
  expect(session).not.toBe(oldSession);
  const oldStop = await owner("/api/browser", {
    op: "stop",
    client: "companion-test-client",
    session: oldSession,
  });
  expect(oldStop.body.error).toContain("session changed");
  await action("stop");
  await owner("/api/browser/pairing", { op: "revoke" });
  await expect
    .poll(() => peer.exitCode ?? peer.signalCode, { timeout: 10000 })
    .not.toBeNull();
  const pairing = (await owner("/api/browser/pairing", { op: "pair" })).body;
  const response = await fetch(base + "/api/browser/companion/claim", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ op: "claim", code: pairing.code }),
  });
  const { key } = await response.json();
  const state = await fetch(base + "/api/state", {
    headers: { "X-Verb-Token": key, Connection: "close" },
  });
  expect(state.status).toBe(403);
  const project = await fetch(base + "/api/terminals", {
    method: "POST",
    headers: {
      "X-Verb-Browser-Key": key,
      "Content-Type": "application/json",
      Connection: "close",
    },
    body: "{}",
  });
  expect(project.status).toBe(403);
  await owner("/api/browser/pairing", { op: "revoke" });
});

test("Studio pairs, discovers the Mac location and revokes a running browser", async ({
  page,
}) => {
  await page.goto(base + "/#" + TOKEN);
  await page.getByRole("button", { name: "Studio", exact: true }).click();
  await page.getByRole("button", { name: "Browser", exact: true }).click();
  await page.locator("#studio-browser-pairing summary").click();
  await page
    .getByRole("button", { name: "Create pairing code", exact: true })
    .click();
  const field = page.getByLabel(
    "One-use pairing code · expires in two minutes",
    { exact: true },
  );
  await expect(field).toHaveValue(/^[a-f0-9]{64}$/);
  const code = await field.inputValue();
  peer = spawn(
    process.execPath,
    [
      resolve("runtime/browser-companion.mjs"),
      "--server",
      base,
      "--chromium",
      chromium.executablePath(),
    ],
    { stdio: ["pipe", "pipe", "pipe"] },
  );
  peer.stdin.end(code + "\n");
  await expect
    .poll(async () => (await owner("/api/browser/pairing")).body.connected)
    .toBe(true);
  await page
    .getByRole("button", { name: "Check connection", exact: true })
    .click();
  await expect(page.locator("#studio-browser-pair-code")).toHaveValue("");
  await page
    .getByRole("button", { name: "Start browser", exact: true })
    .click();
  await expect(page.locator("#studio-browser-stage img")).toBeVisible({
    timeout: 20000,
  });
  await expect(page.locator("#studio-browser-identity")).toContainText(
    "your paired Mac",
  );
  await page
    .getByRole("textbox", { name: "Website address", exact: true })
    .fill(site);
  await page
    .getByRole("button", { name: "Allow site & open", exact: true })
    .click();
  await expect(page.locator("#studio-browser-current-url")).toHaveText(
    site + "/",
  );
  await page.getByRole("button", { name: "Inspect page", exact: true }).click();
  await expect(page.locator("#studio-browser-text")).toContainText(
    "Browser runs on the Mac",
  );
  const owned = [];
  function descendants(pid) {
    let ids = [];
    try {
      ids = execFileSync("pgrep", ["-P", String(pid)], {
        encoding: "utf8",
        stdio: ["ignore", "pipe", "ignore"],
      })
        .trim()
        .split(/\s+/)
        .filter(Boolean)
        .map(Number);
    } catch {}
    for (const id of ids) {
      if (owned.length >= 32) break;
      owned.push(id);
      descendants(id);
    }
  }
  descendants(peer.pid);
  expect(owned.length).toBeGreaterThan(1);
  await page
    .getByRole("button", { name: "Disconnect companion", exact: true })
    .click();
  await expect
    .poll(() => peer.exitCode ?? peer.signalCode, { timeout: 10000 })
    .not.toBeNull();
  await expect(page.locator("#studio-browser-stage img")).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Start browser", exact: true }),
  ).toBeDisabled();
  expect((await owner("/api/browser/pairing")).body.connected).toBe(false);
  await expect
    .poll(
      () =>
        owned.filter((pid) => {
          try {
            process.kill(pid, 0);
            return true;
          } catch {
            return false;
          }
        }).length,
      { timeout: 10000 },
    )
    .toBe(0);
});
