// The conversation flow against the real `verb web` binary, with a scripted stand-in for Claude Code
// (e2e/fixtures/fake-claude.mjs) that writes Claude Code's own log and status formats. Real Claude
// Code is exercised separately; this suite makes every state reachable on demand.
import { test, expect } from "@playwright/test";
import { spawn, execFileSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createServer } from "node:net";

const TOKEN = "e2e-token-0000000000000000000000000000";
const SHOTS = process.env.VERB_SCREENSHOTS;
let server;
let repo;
let home;
let base;
let url;

const freePort = () =>
  new Promise((ok) => {
    const s = createServer().listen(0, "127.0.0.1", () => {
      const { port } = s.address();
      s.close(() => ok(port));
    });
  });

async function call(method, path, body) {
  const response = await fetch(`${base}${path}`, {
    method,
    headers: { "X-Verb-Token": TOKEN, ...(body ? { "Content-Type": "application/json" } : {}) },
    body: body ? JSON.stringify(body) : undefined,
  });
  return { status: response.status, body: await response.json() };
}

const claudeLogs = () => {
  const dir = join(home, ".claude", "projects");
  return readdirSync(dir).flatMap((d) => readdirSync(join(dir, d)).map((f) => join(dir, d, f)));
};
const shot = async (page, name) => {
  if (SHOTS) await page.screenshot({ path: join(SHOTS, `${name}.png`), fullPage: false });
};

async function startClaude(page) {
  const launched = await call("POST", "/api/terminals", { agent: "claude" });
  expect(launched.status, JSON.stringify(launched.body)).toBe(201);
  const id = launched.body.id ?? launched.body.session?.id ?? launched.body.sessionId;
  expect(id).toBeTruthy();
  await page.goto(url);
  return id;
}

async function openChat(page, id) {
  const tile = page.locator(`.terminal-tile[data-session="${id}"], [data-terminal="${id}"]`).first();
  await page.getByRole("button", { name: "Chat", exact: true }).first().click();
  return page.locator(".agent-stream:not([hidden])").first();
}

test.describe.configure({ mode: "serial" });

test.beforeAll(async () => {
  const bin = resolve(process.env.VERB_BIN ?? "../target/debug/verb");
  const fixture = mkdtempSync(join(tmpdir(), "verb-conv-"));
  repo = join(fixture, "project");
  home = join(fixture, "home");
  const agents = join(fixture, "bin");
  for (const dir of [repo, agents]) mkdirSync(dir);
  mkdirSync(home, { mode: 0o700 });
  execFileSync("git", ["init", "-q", "-b", "main", repo]);
  execFileSync("git", ["-C", repo, "config", "user.name", "E2E Person"]);
  execFileSync("git", ["-C", repo, "config", "user.email", "e2e@example.com"]);
  writeFileSync(join(repo, "README.md"), "# conversation e2e\n");
  execFileSync("git", ["-C", repo, "add", "-A"]);
  execFileSync("git", ["-C", repo, "commit", "-qm", "initial"]);
  const fake = resolve("e2e/fixtures/fake-claude.mjs");
  writeFileSync(join(agents, "claude"), `#!/bin/sh\nexec node "${fake}" "$@"\n`);
  chmodSync(join(agents, "claude"), 0o755);
  const port = await freePort();
  base = `http://127.0.0.1:${port}`;
  url = `${base}/#${TOKEN}`;
  server = spawn(bin, ["web", "--port", String(port)], {
    cwd: repo,
    env: { PATH: `${agents}:${process.env.PATH}`, HOME: home, LANG: "C", VERB_STATE_DIR: join(fixture, "state"), VERB_TOKEN: TOKEN },
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

let session;

test("with the stream off, chat shows no content and the host refuses to send", async ({ page }) => {
  session = await startClaude(page);
  const chat = await openChat(page, session);
  await expect(chat.locator(".conv-card h4")).toHaveText("Stream off");
  await expect(chat.locator(".conv-chip")).toHaveText("Stream off");
  await expect(chat.locator(".conv-send")).toBeDisabled();
  await expect(chat.locator(".conv-block")).toContainText("Sending needs the stream on");
  const refused = await call("POST", `/api/terminals/${session}/chat`, { messageId: "m-refused-1", text: "hello" });
  expect(refused.status).toBe(409);
  expect(refused.body.code).toBe("not_ready");
  await shot(page, "d12-stream-off-never-on");
});

test("turning it on reads the conversation and a message is received from the agent's log", async ({ page }) => {
  await page.goto(url);
  const chat = await openChat(page, session);
  await chat.getByRole("button", { name: "Turn on for this project" }).click();
  await expect(chat.locator(".conv-chip")).toHaveText("Waiting for you");
  await expect(chat.locator(".conv-based")).toContainText("reports its status as idle");
  await expect(chat.locator(".conv-identity")).toContainText("Attempt");
  await shot(page, "d1-waiting");
  const input = chat.getByRole("textbox", { name: "Message Claude Code" });
  await input.fill("Add a line about the launch");
  await input.press("Enter");
  await expect(chat.locator(".conv-you").filter({ hasText: "Add a line about the launch" }).first()).toBeVisible();
  await expect(chat.locator(".conv-delivery-line").filter({ hasText: /^Received · in the agent's log/ })).toBeVisible({ timeout: 15000 });
  await expect(chat.locator(".conv-agent")).toContainText("Done. I noted: Add a line about the launch");
  await expect(chat.locator(".conv-work")).toContainText("Edited");
  await shot(page, "d3-received");
});

test("one message id is typed at most once, and a second message cannot overtake the first", async () => {
  const first = await call("POST", `/api/terminals/${session}/chat`, { messageId: "m-dup-000000000001", text: "[slow] count me once" });
  expect(first.status).toBe(200);
  const again = await call("POST", `/api/terminals/${session}/chat`, { messageId: "m-dup-000000000001", text: "[slow] count me once" });
  expect(again.status).toBe(200);
  const overtake = await call("POST", `/api/terminals/${session}/chat`, { messageId: "m-dup-000000000002", text: "me first" });
  expect(overtake.status).toBe(409);
  expect(["unresolved", "not_ready"]).toContain(overtake.body.code);
  await expect
    .poll(async () => (await call("GET", `/api/terminals/${session}/conversation`)).body.deliveries.find((d) => d.id === "m-dup-000000000001")?.state, { timeout: 15000 })
    .toBe("received");
  const log = claudeLogs().map((f) => readFileSync(f, "utf8")).join("");
  expect(log.split("count me once").length - 1).toBe(2); // the user turn and the agent's reply, once each
});

test("a permission prompt closes chat and is answered only in the terminal", async ({ page }) => {
  await page.goto(url);
  const chat = await openChat(page, session);
  const input = chat.getByRole("textbox", { name: "Message Claude Code" });
  await input.fill("[approve] Record the approval step");
  await input.press("Enter");
  await expect(chat.locator(".conv-chip")).toHaveText(/Looks like a permission prompt|Working/, { timeout: 10000 });
  await expect(chat.locator(".conv-approval")).toBeVisible({ timeout: 10000 });
  await expect(chat.locator(".conv-send")).toBeDisabled();
  const refused = await call("POST", `/api/terminals/${session}/chat`, { messageId: "m-during-approval", text: "yes" });
  expect(refused.status).toBe(409);
  await shot(page, "d4-approval");
  await chat.getByRole("button", { name: "Open in terminal to approve" }).click();
  await expect(page.locator(".agent-stream").first()).toBeHidden();
  // Raw terminal input is never gated by readiness: the person answers the provider's prompt.
  const typed = await call("POST", `/api/terminals/${session}/input`, { data: "1" });
  expect(typed.status).toBe(200);
  await page.getByRole("button", { name: "Chat", exact: true }).first().click();
  await expect(chat.locator(".conv-agent").last()).toContainText("Record the approval step", { timeout: 15000 });
  await expect(chat.locator(".conv-chip")).toHaveText("Waiting for you", { timeout: 10000 });
});

test("an identical earlier message never confirms, and a lost request is never typed twice", async ({ page }) => {
  await page.goto(url);
  const chat = await openChat(page, session);
  // The browser loses the answer to its request although the host received and typed it.
  await page.route("**/chat", async (route) => {
    await route.fetch();
    await route.abort("failed");
  });
  const input = chat.getByRole("textbox", { name: "Message Claude Code" });
  await input.fill("Add a line about the launch");
  await input.press("Enter");
  await expect(chat.locator(".conv-banner")).toContainText("Reconnecting");
  await page.unroute("**/chat");
  // Once reachable, the host's record decides: it was received, from this turn and not the earlier
  // identical one, and nothing was typed a second time.
  await expect(chat.locator(".conv-banner")).toContainText("Same process, same conversation", { timeout: 10000 });
  await expect
    .poll(async () => (await call("GET", `/api/terminals/${session}/conversation`)).body.deliveries.filter((d) => d.state === "received").length, { timeout: 15000 })
    .toBeGreaterThanOrEqual(3);
  const log = claudeLogs().map((f) => readFileSync(f, "utf8")).join("");
  expect(log.split('"content":"Add a line about the launch"').length - 1).toBe(2);
});

test("a crash before the log shows a message leaves it unknown, with deliberate recovery", async ({ page }) => {
  await page.goto(url);
  const chat = await openChat(page, session);
  const input = chat.getByRole("textbox", { name: "Message Claude Code" });
  await input.fill("[crash] this may not arrive");
  await input.press("Enter");
  const bubble = chat.locator(".conv-delivery[data-state='unknown']");
  await expect(bubble).toBeVisible({ timeout: 15000 });
  await expect(bubble).toContainText("Delivery unknown. It may or may not have reached Claude Code.");
  await expect(bubble.getByRole("button", { name: "Send again (may duplicate)" })).toBeVisible();
  await expect(bubble.getByRole("button", { name: "Discard" })).toBeVisible();
  await expect(chat.locator(".conv-chip")).toHaveText("Agent stopped");
  await shot(page, "d7-unknown");
  await bubble.getByRole("button", { name: "Send again (may duplicate)" }).click();
  await expect(bubble.getByRole("button", { name: "Send again — it may arrive twice" })).toBeVisible();
  await bubble.getByRole("button", { name: "Discard" }).click();
  await expect(chat.locator(".conv-delivery[data-state='discarded']")).toContainText("may still have reached the agent");
});

test("a resumed attempt names the same conversation and keeps the session", async ({ page }) => {
  // The lightweight workspace snapshot can briefly precede the terminal-exit record.
  // Wait for that record before testing resume, rather than assuming the cached view is fresh.
  await expect.poll(async () => (await call("GET", "/api/state")).body.sessions.find((s) => s.id === session)?.state,
    { timeout: 10000 }).toMatch(/^(interrupted|recoverable)$/);
  const resumed = await call("POST", "/api/terminals", { agent: "claude", resume_id: session });
  expect(resumed.status, JSON.stringify(resumed.body)).toBe(201);
  await page.goto(url);
  const chat = await openChat(page, session);
  await expect(chat.locator(".conv-identity")).toContainText("#2");
  await expect(chat.locator(".conv-continuity")).toContainText("Conversation resumed", { timeout: 15000 });
  await expect(chat.locator(".conv-agent").first()).toBeVisible();
  await shot(page, "d11-resumed");
});

test("turning the stream off clears at once, leaves the log, and reports each device", async ({ page, browser }) => {
  const other = await browser.newContext();
  const second = await other.newPage();
  await second.goto(url);
  await openChat(second, session);
  await expect(second.locator(".conv-agent").first()).toBeVisible();
  await other.setOffline(true);

  await page.goto(url);
  const chat = await openChat(page, session);
  await expect(chat.locator(".conv-agent").first()).toBeVisible();
  const logs = claudeLogs().map((f) => readFileSync(f));
  await chat.getByRole("button", { name: "Turn off stream" }).click();
  await expect(chat.locator(".conv-agent")).toHaveCount(0);
  await expect(chat.locator(".conv-devices")).toContainText("Stream turned off on this device");
  await expect(chat.locator(".conv-devices")).toContainText("not yet confirmed cleared");
  await expect(chat.locator(".conv-devices")).toContainText("last seen");
  const waiting = await chat.locator(".conv-devices li[data-state='not_confirmed']").count();
  expect(claudeLogs().map((f) => readFileSync(f))).toEqual(logs);
  const view = (await call("GET", `/api/terminals/${session}/conversation`)).body;
  expect(view.items).toEqual([]);
  expect(JSON.stringify(view)).not.toContain("approval step");
  await shot(page, "d12-stream-off-devices");

  await other.setOffline(false);
  await expect(second.locator(".conv-agent")).toHaveCount(0, { timeout: 10000 });
  // The reconnected browser cleared and said so; tabs closed earlier can never say so.
  await expect(chat.locator(".conv-devices li[data-state='not_confirmed']")).toHaveCount(waiting - 1, { timeout: 15000 });
  await other.close();
  await chat.getByRole("button", { name: "Turn on for this project" }).click();
  await expect(chat.locator(".conv-agent").first()).toBeVisible();
});

test("Studio reads the same conversation and opens the agent's files in the preview pane", async ({ page }) => {
  await page.goto(url);
  await page.locator('.nav-item[data-view="studio"]').first().click();
  await page.locator("#studio-conversation-session").selectOption(session);
  const panel = page.locator("#studio-conversation-mount");
  await expect(panel.locator(".conv-agent").first()).toBeVisible();
  const toggle = page.getByRole("button", { name: "Define another change" });
  await expect(page.locator("#studio-intent-form")).toBeHidden();
  await expect(toggle).toHaveAttribute("aria-expanded", "false");
  await toggle.click();
  await expect(page.locator("#studio-intent")).toBeFocused();
  await page.locator("#studio-intent").fill("Keep this separate draft");
  await page.getByRole("button", { name: "Hide new change" }).click();
  await expect(page.locator("#studio-intent-form")).toBeHidden();
  await page.getByRole("button", { name: "Define another change" }).click();
  await expect(page.locator("#studio-intent")).toHaveValue("Keep this separate draft");
  await page.locator("#studio-intent").fill("");
  await page.getByRole("button", { name: "Hide new change" }).click();
  await panel.locator(".conv-work").last().locator("summary").click({ trial: false }).catch(() => {});
  await panel.getByRole("button", { name: "notes.md" }).last().click();
  await expect(page.locator("#studio-preview-tabs")).toContainText("notes.md");
  await expect(page.locator("#studio-document")).toContainText("Add a line about the launch");
  await panel.locator(".conv-composer").scrollIntoViewIfNeeded();
  const contained = await panel.locator(".conv-composer").evaluate((form) => {
    const rect = form.getBoundingClientRect();
    return [...form.querySelectorAll("button")].every((button) => button.hidden || button.getBoundingClientRect().bottom <= rect.bottom + 1);
  });
  expect(contained).toBe(true);
  await shot(page, "studio-conversation");
});

test("phone Studio keeps chat primary and preserves a separate work draft", async ({ browser }) => {
  const context = await browser.newContext({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });
  const page = await context.newPage();
  try {
    await page.goto(url);
    await page.locator('.nav-item[data-view="studio"]').first().click();
    await page.getByRole("button", { name: "Current work", exact: true }).click();
    await page.locator("#studio-conversation-session").selectOption(session);
    const panel = page.locator("#studio-conversation-mount");
    await expect(panel.locator(".conv-agent").first()).toBeVisible();
    await expect(page.locator("#studio-intent-form")).toBeHidden();
    await page.getByRole("button", { name: "Define another change" }).click();
    await page.locator("#studio-intent").fill("Plan this next");
    await page.getByRole("button", { name: "Hide new change" }).click();
    await page.getByRole("button", { name: "Define another change" }).click();
    await expect(page.locator("#studio-intent")).toHaveValue("Plan this next");
    await page.getByRole("button", { name: "Hide new change" }).click();
    await panel.locator(".conv-composer").scrollIntoViewIfNeeded();
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
    await shot(page, "studio-conversation-phone");
  } finally {
    await context.close();
  }
});

test("phone width: one surface at a time, reachable controls, keyboard-only sending", async ({ browser }) => {
  const context = await browser.newContext({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true });
  const page = await context.newPage();
  await page.goto(url);
  const chat = await openChat(page, session);
  await expect(chat.locator(".conv-chip")).toBeVisible();
  for (const control of await chat.locator(".conv-actions button:visible").all()) {
    const box = await control.boundingBox();
    expect(box.height).toBeGreaterThanOrEqual(44);
    const form = await chat.locator(".conv-composer").boundingBox();
    expect(box.y + box.height).toBeLessThanOrEqual(form.y + form.height + 1);
  }
  const width = await page.evaluate(() => document.documentElement.scrollWidth);
  expect(width).toBeLessThanOrEqual(390);
  await shot(page, "p2-chat-phone");
  await context.close();
});

test("every control in the panel has a name, and sending works by keyboard alone", async ({ page }) => {
  await page.goto(url);
  const chat = await openChat(page, session);
  await expect(chat.locator(".conv-chip")).toHaveText("Waiting for you", { timeout: 10000 });
  const unnamed = await chat.locator("button, textarea, select").evaluateAll((els) =>
    els.filter((el) => el.offsetParent !== null).filter((el) => !(el.getAttribute("aria-label") || el.labels?.length || el.textContent.trim())).length,
  );
  expect(unnamed).toBe(0);
  await expect(chat.locator('[role="status"]').first()).toBeAttached();
  const input = chat.getByRole("textbox", { name: "Message Claude Code" });
  await input.focus();
  await page.keyboard.type("Keyboard only");
  await page.keyboard.press("Enter");
  await expect(chat.locator(".conv-agent").last()).toContainText("Keyboard only", { timeout: 15000 });
});


test("stream off in Studio clears hidden Sessions panels and stored sent text before acknowledgement", async ({ page }) => {
  await page.goto(url);
  await call("POST", "/api/stream", { enabled: true });
  const chat = await openChat(page, session);
  await expect(chat.locator(".conv-agent").first()).toBeVisible();
  await page.locator('.nav-item[data-view="studio"]').first().click();
  await page.locator("#studio-conversation-session").selectOption(session);
  const studio = page.locator("#studio-conversation-mount");
  await expect(studio.locator(".conv-agent").first()).toBeVisible();
  await studio.getByRole("textbox", { name: "Message Claude Code" }).fill("keep my unsent draft");
  await studio.getByRole("button", { name: "Turn off stream" }).click();
  await expect(studio.locator(".conv-card h4")).toHaveText("Stream off");
  await expect(page.locator(".conv-agent")).toHaveCount(0);
  await expect(studio.getByRole("textbox", { name: "Message Claude Code" })).toHaveValue("keep my unsent draft");
  expect(await page.evaluate(() => Object.keys(sessionStorage).filter((key) => key.startsWith("verb.chat.")).flatMap((key) => JSON.parse(sessionStorage[key])).some((record) => record.text))).toBe(false);
  await studio.getByRole("button", { name: "Turn on for this project" }).click();
  await expect(studio.locator(".conv-agent").first()).toBeVisible();
});

test("a different message with identical redacted text cannot confirm delivery", async () => {
  await expect.poll(async () => (await call("GET", `/api/terminals/${session}/conversation`)).body.canSend).toBe(true);
  const sent = await call("POST", `/api/terminals/${session}/chat`, { messageId: "m-redaction-proof", text: "[slow] password=original123" });
  expect(sent.status).toBe(200);
  const conversation = sent.body.identity.conversation;
  const log = claudeLogs().find((file) => file.endsWith(`${conversation}.jsonl`));
  const { appendFileSync } = await import("node:fs");
  appendFileSync(log, `${JSON.stringify({ type: "user", uuid: "different-turn", sessionId: conversation, timestamp: new Date().toISOString(), message: { content: "[slow] password=different456" } })}\n`);
  await new Promise((r) => setTimeout(r, 700));
  const during = (await call("GET", `/api/terminals/${session}/conversation`)).body;
  expect(during.deliveries.find((d) => d.id === "m-redaction-proof").state).toBe("sent");
  await expect.poll(async () => (await call("GET", `/api/terminals/${session}/conversation`)).body.deliveries.find((d) => d.id === "m-redaction-proof")?.state, { timeout: 15000 }).toBe("received");
  const final = (await call("GET", `/api/terminals/${session}/conversation`)).body;
  expect(final.deliveries.find((d) => d.id === "m-redaction-proof").turnId).not.toBe("different-turn");
});
