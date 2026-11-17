// A terminal stream whose Cloudflare Access sign-in ends must close once with the signed-out code
// (4401), show one clear message and never reconnect. Before, the host dropped the socket without
// flushing its Close frame; the page saw 1006 and retried every two seconds, forever.
import { test, expect } from "@playwright/test";
import { execFileSync, spawn } from "node:child_process";
import { generateKeyPairSync, createSign } from "node:crypto";
import { mkdirSync, mkdtempSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const EMAIL = "owner@example.invalid";
const ISS = "https://example.cloudflareaccess.com";
const AUD = "verb-e2e-audience";
const KID = "e2e-key";

const freePort = () => new Promise((resolvePort) => {
  const socket = createServer().listen(0, "127.0.0.1", () => {
    const { port } = socket.address();
    socket.close(() => resolvePort(port));
  });
});

const b64url = (value) => Buffer.from(value).toString("base64url");
function signJwt(privateKey, exp) {
  const header = b64url(JSON.stringify({ alg: "RS256", kid: KID, typ: "JWT" }));
  const claims = b64url(JSON.stringify({ aud: [AUD], iss: ISS, email: EMAIL, exp, sub: "e2e", type: "app" }));
  const signer = createSign("RSA-SHA256");
  signer.update(`${header}.${claims}`);
  return `${header}.${claims}.${signer.sign(privateKey).toString("base64url")}`;
}

test("a terminal stream whose sign-in expires closes once with 4401 and does not retry", async ({ page, playwright }) => {
  test.skip(process.platform === "win32", "POSIX fixture");
  const fixture = mkdtempSync(join(tmpdir(), "verb-signed-out-"));
  const repo = join(fixture, "project");
  const home = join(fixture, "home");
  for (const path of [repo, home]) mkdirSync(path, { mode: 0o700 });
  const environment = { PATH: process.env.PATH, HOME: home, LANG: "C", SHELL: "/bin/bash" };
  execFileSync("git", ["init", "-q", "-b", "main", repo], { env: environment });

  const { privateKey, publicKey } = generateKeyPairSync("rsa", { modulusLength: 2048 });
  const jwk = { ...publicKey.export({ format: "jwk" }), alg: "RS256", use: "sig", kid: KID };
  const port = await freePort();
  const origin = `http://127.0.0.1:${port}`;
  const server = spawn(resolve(process.env.VERB_BIN ?? "../target/debug/verb"), ["web", "--port", String(port)], {
    cwd: repo,
    env: {
      ...environment,
      VERB_STATE_DIR: join(fixture, "state"),
      VERB_ALLOWED_EMAIL: EMAIL,
      VERB_CF_ACCESS_ISS: ISS,
      VERB_CF_ACCESS_AUD: AUD,
      VERB_CF_JWKS_JSON: JSON.stringify({ keys: [jwk] }),
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  try {
    await new Promise((ready, reject) => {
      const timer = setTimeout(() => reject(new Error("host did not start")), 15_000);
      server.stdout.on("data", (data) => {
        if (String(data).includes("Verb web UI")) { clearTimeout(timer); ready(); }
      });
      server.once("exit", () => { clearTimeout(timer); reject(new Error("host exited")); });
    });

    // Cloudflare's edge signs every request in with the current Access assertion. The login ends
    // 2 s after the page first asks for a terminal ticket. Requests go through a separate request
    // context with its own cookie jar, so the host's 24-hour session cookie never reaches the
    // browser and the stream carries the Access expiry.
    const edge = await playwright.request.newContext();
    let loginEnds = null;
    // Only API requests: a document answered by the test would count as a public page, and Chrome's
    // local network checks would then block its WebSocket.
    await page.route(`${origin}/api/**`, async (route) => {
      const request = route.request();
      const now = Math.floor(Date.now() / 1000);
      if (loginEnds === null && new URL(request.url()).pathname === "/api/terminals/ticket") loginEnds = now + 2;
      const headers = { ...request.headers(), "cf-access-jwt-assertion": signJwt(privateKey, loginEnds ?? now + 3600) };
      delete headers.cookie;
      const response = await edge.fetch(request.url(), { method: request.method(), headers, data: request.postDataBuffer() ?? undefined, maxRedirects: 0 });
      const replyHeaders = { ...response.headers() };
      delete replyHeaders["set-cookie"];
      await route.fulfill({ status: response.status(), headers: replyHeaders, body: await response.body() });
    });

    const streams = [];
    page.on("websocket", (socket) => { if (new URL(socket.url()).pathname === "/api/terminals/ws") streams.push(socket); });
    const closes = [];
    page.on("console", (message) => { if (message.text().startsWith("Verb terminal stream closed:")) closes.push(message.text()); });

    await page.goto(`${origin}/`);
    await expect.poll(() => streams.length, { timeout: 5_000 }).toBe(1);
    const notice = page.locator("#signed-out");
    await expect(notice).toBeVisible({ timeout: 6_000 });
    await expect(notice).toContainText("Signed out. Reload to sign in again.");
    await expect(notice.getByRole("button", { name: "Reload" })).toBeVisible();
    expect(closes).toEqual(["Verb terminal stream closed: 4401 sign-in expired"]);

    // Several old retry periods later: still one stream, one close, no retry.
    await page.waitForTimeout(5_000);
    expect(streams.length).toBe(1);
    expect(closes.length).toBe(1);
  } finally {
    await page.unrouteAll({ behavior: "ignoreErrors" });
    server.kill();
  }
});
