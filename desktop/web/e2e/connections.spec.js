// UI → owner API → official Rust MCP client → official TypeScript SDK server.
// The HTTP loopback exception is compiled only in the synthetic test binary, never releases.
import { test, expect } from "@playwright/test";
import {
  McpServer,
  WebStandardStreamableHTTPServerTransport,
} from "@modelcontextprotocol/server";
import { z } from "zod";
import { createServer } from "node:http";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
const TOKEN = "mcp-fixture-owner-0000000000000000000000";
let host,
  remote,
  base,
  endpoint,
  calls = 0,
  mode = "json",
  lastText = "",
  id,
  context,
  bin,
  repo,
  nativeEnv;
const auth = {
  "X-Verb-Token": TOKEN,
  "Content-Type": "application/json",
  Connection: "close",
};
async function owner(body) {
  const r = await fetch(base + "/api/connections", {
    method: body ? "POST" : "GET",
    headers: auth,
    ...(body ? { body: JSON.stringify(body) } : {}),
  });
  return { status: r.status, body: await r.json() };
}
test.describe.configure({ mode: "serial" });
test.skip(
  !process.env.VERB_SYNTHETIC_BIN,
  "Requires the explicitly opted-in synthetic test binary",
);
test.beforeAll(async () => {
  remote = createServer(async (req, res) => {
    const chunks = [];
    for await (const chunk of req) chunks.push(chunk);
    const server = new McpServer({
      name: "Official SDK fixture",
      version: "1.0.0",
    });
    server.registerTool(
      "echo",
      {
        title: "Echo approved text",
        description: "Records exactly one reviewed request.",
        inputSchema: z.object({ text: z.string() }),
      },
      async ({ text }) => {
        calls++;
        lastText = text;
        if (mode === "delay") await new Promise((r) => setTimeout(r, 900));
        if (mode === "lost") {
          res.destroy();
          return { content: [] };
        }
        return { content: [{ type: "text", text }], isError: false };
      },
    );
    server.registerResource(
      "Reference",
      "memo://reference",
      { mimeType: "text/plain" },
      async () => ({
        contents: [
          {
            uri: "memo://reference",
            mimeType: "text/plain",
            text: "Reference from the official SDK",
          },
        ],
      }),
    );
    const transport = new WebStandardStreamableHTTPServerTransport({
      sessionIdGenerator: undefined,
      enableJsonResponse: mode !== "sse",
    });
    try {
      await server.connect(transport);
      const response = await transport.handleRequest(
        new Request(endpoint, {
          method: req.method,
          headers: req.headers,
          ...(chunks.length ? { body: Buffer.concat(chunks) } : {}),
        }),
      );
      if (!res.destroyed) {
        res.writeHead(response.status, Object.fromEntries(response.headers));
        if (response.body) await pipeline(Readable.fromWeb(response.body), res);
        else res.end();
      }
    } catch {
      if (!res.destroyed) {
        res.writeHead(500);
        res.end();
      }
    } finally {
      await server.close();
    }
  });
  await new Promise((r) => remote.listen(0, "127.0.0.1", r));
  endpoint = `http://127.0.0.1:${remote.address().port}/mcp`;
  const root = mkdtempSync(join(tmpdir(), "verb-mcp-e2e-")),
    project = join(root, "project"),
    home = join(root, "home");
  mkdirSync(project);
  mkdirSync(home);
  execFileSync("git", ["init", "-q", "-b", "main", project]);
  writeFileSync(join(project, "README.md"), "# MCP acceptance workspace\n");
  bin = resolve(process.env.VERB_SYNTHETIC_BIN);
  repo = project;
  nativeEnv = {
    PATH: process.env.PATH,
    HOME: home,
    VERB_TOKEN: TOKEN,
    VERB_STATE_DIR: join(root, "state"),
    VERB_SYNTHETIC_MCP_ENDPOINT: endpoint,
  };
  host = spawn(bin, ["web", "--port", "0"], {
    cwd: project,
    env: nativeEnv,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let output = "";
  await new Promise((ok, fail) => {
    const timer = setTimeout(
      () => fail(Error("Verb host did not start")),
      20000,
    );
    host.stdout.on("data", (b) => {
      output += b;
      const match = output.match(/http:\/\/127\.0\.0\.1:(\d+)/);
      if (match) {
        base = `http://127.0.0.1:${match[1]}`;
        clearTimeout(timer);
        ok();
      }
    });
    host.on("exit", () => fail(Error("Host exited")));
  });
});
test.afterAll(async () => {
  if (id) await owner({ op: "disconnect", id }).catch(() => {});
  host?.kill("SIGTERM");
  remote?.closeAllConnections();
  remote?.close();
  await context?.close();
});

test("connections are owner-only and refuse private endpoints and Verb credentials", async () => {
  const denied = await fetch(base + "/api/connections");
  expect(denied.status).toBe(403);
  for (const scope of ["read", "drive"]) {
    const output = execFileSync(
      bin,
      [
        "token",
        "create",
        "--name",
        `mcp-${scope}-fixture`,
        "--scope",
        scope,
        "--expires",
        "1h",
      ],
      {
        cwd: repo,
        env: nativeEnv,
        encoding: "utf8",
        stdio: ["ignore", "pipe", "pipe"],
      },
    );
    const token = output.match(/verb_at_[A-Za-z0-9_-]+/)[0];
    for (const method of ["GET", "POST"]) {
      const r = await fetch(base + "/api/connections", {
        method,
        headers: { "X-Verb-Token": token, "Content-Type": "application/json" },
        ...(method === "POST" ? { body: '{"op":"connect"}' } : {}),
      });
      expect(r.status).toBe(403);
    }
  }
  expect(
    (
      await owner({
        op: "connect",
        name: "Private",
        url: "http://127.0.0.1:1/mcp",
      })
    ).status,
  ).not.toBe(200);
  expect(
    (
      await owner({
        op: "connect",
        name: "Credential error",
        url: endpoint,
        credential: TOKEN,
      })
    ).status,
  ).toBe(400);
});

test("a person connects, reviews a call, inspects the result and attaches a Studio draft", async ({
  browser,
}, testInfo) => {
  context = await browser.newContext();
  const page = await context.newPage();
  await page.goto(base + "/#" + TOKEN);
  await page.locator("#nav-more").evaluate((el) => (el.open = true));
  await page.getByRole("button", { name: "Connections", exact: true }).click();
  await page.getByText("Connect a service", { exact: true }).click();
  await page
    .getByLabel("Connection name", { exact: true })
    .fill("Documentation");
  await page.getByLabel("MCP endpoint", { exact: true }).fill(endpoint);
  await page
    .getByRole("button", { name: "Connect and inspect", exact: true })
    .click();
  await expect(page.locator("#connection-title")).toHaveText("Documentation", {
    timeout: 20000,
  });
  await expect(page.locator("#connection-server")).toContainText(
    "Official SDK fixture",
  );
  id = (await owner()).body.connections[0].id;
  await page
    .getByRole("textbox", { name: "Text", exact: true })
    .fill("A draft to preserve");
  await page.getByLabel("Edit inputs as JSON", { exact: true }).check();
  await page.locator("#connection-arguments").fill('{"text":"Advanced draft"}');
  await page.getByLabel("Edit inputs as JSON", { exact: true }).uncheck();
  await expect(
    page.getByRole("textbox", { name: "Text", exact: true }),
  ).toHaveValue("Advanced draft");
  await page.getByLabel("Edit inputs as JSON", { exact: true }).check();
  await page
    .locator("#connection-arguments")
    .fill('{"text":"Keep this","extra":1}');
  await page.getByLabel("Edit inputs as JSON", { exact: true }).click();
  await expect(
    page.getByLabel("Edit inputs as JSON", { exact: true }),
  ).toBeChecked();
  await expect(page.locator("#connection-arguments")).toHaveValue(
    '{"text":"Keep this","extra":1}',
  );
  await page
    .locator("#connection-arguments")
    .fill('{"text":"Exactly what I approved"}');
  await page.getByLabel("Edit inputs as JSON", { exact: true }).uncheck();

  await page
    .getByRole("button", { name: "Review request", exact: true })
    .click();
  await expect(page.locator("#connection-review")).toBeVisible();
  expect(calls).toBe(0);
  await expect(
    page.getByRole("button", { name: "Approve and run once", exact: true }),
  ).toBeFocused();
  await page
    .getByRole("button", { name: "Approve and run once", exact: true })
    .click();
  await expect(page.locator("#connection-result")).toContainText(
    "Exactly what I approved",
  );
  expect(calls).toBe(1);
  expect(lastText).toBe("Exactly what I approved");
  await page.screenshot({
    path: testInfo.outputPath("mcp-desktop.png"),
    fullPage: true,
  });
  await page
    .getByRole("button", { name: "Use in Studio draft", exact: true })
    .click();
  await expect(page.locator("#studio-context-text")).toContainText(
    "Exactly what I approved",
  );
  await expect(page.locator("#studio-message")).toContainText(
    "Nothing was sent to an agent",
  );
});

test("a pasted Verb credential in tool input is refused before external dispatch", async () => {
  const before = calls;
  const r = await owner({
    op: "review",
    id,
    kind: "tools",
    name: "echo",
    arguments: { text: `Pasted from a terminal: ${TOKEN}` },
  });
  expect(r.status).toBe(400);
  expect(calls).toBe(before);
});

test("SSE results work and the same approval can never run twice", async () => {
  mode = "sse";
  const r = await owner({
    op: "review",
    id,
    kind: "tools",
    name: "echo",
    arguments: { text: "SSE approved" },
  });
  expect(r.status).toBe(200);
  const first = await owner({
    op: "execute",
    review: r.body.review,
    arguments: { text: "Unreviewed" },
  });
  expect(first.body.outcome).toBe("received");
  expect(JSON.stringify(first.body.result)).toContain("SSE approved");
  const before = calls;
  expect(
    (await owner({ op: "execute", review: r.body.review })).body.outcome,
  ).toBe("not_sent");
  expect(calls).toBe(before);
  mode = "json";
});

test("resource selection reads the official SDK resource without invoking a tool", async () => {
  const page = context.pages()[0];
  await page.locator("#nav-more").evaluate((el) => (el.open = true));
  await page.getByRole("button", { name: "Connections", exact: true }).click();
  await page.locator("#connection-item").selectOption({ label: "Reference" });
  await expect(page.locator("#connection-arguments-wrap")).toBeHidden();
  const before = calls;
  await page
    .getByRole("button", { name: "Review request", exact: true })
    .click();
  await expect(page.locator("#connection-review-text")).toContainText(
    "memo://reference",
  );
  await page
    .getByRole("button", { name: "Approve and run once", exact: true })
    .click();
  await expect(page.locator("#connection-result")).toHaveText(
    "Reference from the official SDK",
  );
  expect(calls).toBe(before);
});

test("phone review remains usable and disconnect clears visible results", async ({
  browser,
}, testInfo) => {
  const phone = await browser.newContext({
    viewport: { width: 390, height: 844 },
  });
  const page = await phone.newPage();
  await page.goto(base + "/#" + TOKEN);
  await page.locator("#nav-more").evaluate((el) => (el.open = true));
  await page.getByRole("button", { name: "Connections", exact: true }).click();
  await page.getByRole("button", { name: /Documentation.*Connected/ }).click();
  await expect(page.locator("#connection-title")).toHaveText("Documentation");
  await page
    .getByRole("textbox", { name: "Text", exact: true })
    .fill("Phone request");
  await page
    .getByRole("button", { name: "Review request", exact: true })
    .click();
  await expect(page.locator("#connection-review-text")).toContainText(
    "Phone request",
  );
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  await page
    .getByRole("button", { name: "Approve and run once", exact: true })
    .click();
  await expect(page.locator("#connection-result")).toContainText(
    "Phone request",
  );
  await page.screenshot({
    path: testInfo.outputPath("mcp-phone.png"),
    fullPage: true,
  });
  await page.screenshot({
    path: testInfo.outputPath("mcp-phone-viewport.png"),
  });
  await page.getByRole("button", { name: "Disconnect", exact: true }).click();
  await expect(page.locator("#connection-result-wrap")).toBeHidden();
  await expect(page.locator("#connections-list")).toContainText(
    "No connections",
  );
  id = null;
  await phone.close();
});

test("in-flight inputs are locked and disconnect cannot restore a late result", async ({
  browser,
}) => {
  mode = "delay";
  const connected = await owner({
    op: "connect",
    name: "Recovery",
    url: endpoint,
  });
  expect(connected.status).toBe(200);
  id = connected.body.id;
  const local = await browser.newContext();
  const page = await local.newPage();
  await page.goto(base + "/#" + TOKEN);
  await page.locator("#nav-more").evaluate((el) => (el.open = true));
  await page.getByRole("button", { name: "Connections", exact: true }).click();
  await page.getByRole("button", { name: /Recovery.*Connected/ }).click();
  await page
    .getByRole("textbox", { name: "Text", exact: true })
    .fill("Late content must be discarded");
  await page
    .getByRole("button", { name: "Review request", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Approve and run once", exact: true })
    .click();
  await expect(
    page.getByRole("textbox", { name: "Text", exact: true }),
  ).toBeDisabled();
  await expect(page.locator("#connection-item")).toBeDisabled();
  await page.getByRole("button", { name: "Disconnect", exact: true }).click();
  await expect(page.locator("#connections-list")).toContainText(
    "No connections",
  );
  await expect(page.locator("#connection-result-wrap")).toBeHidden();
  await page.waitForTimeout(1000);
  await expect(page.locator("#connection-result-wrap")).toBeHidden();
  mode = "json";
  id = null;
  await local.close();
});

test("lost replies show unknown delivery without automatically repeating the call", async ({
  browser,
}) => {
  mode = "lost";
  const connected = await owner({
    op: "connect",
    name: "Unknown delivery",
    url: endpoint,
  });
  expect(connected.status).toBe(200);
  id = connected.body.id;
  const local = await browser.newContext();
  const page = await local.newPage();
  await page.goto(base + "/#" + TOKEN);
  await page.locator("#nav-more").evaluate((el) => (el.open = true));
  await page.getByRole("button", { name: "Connections", exact: true }).click();
  await page
    .getByRole("button", { name: /Unknown delivery.*Connected/ })
    .click();
  await page
    .getByRole("textbox", { name: "Text", exact: true })
    .fill("One request only");
  await page
    .getByRole("button", { name: "Review request", exact: true })
    .click();
  const before = calls;
  await page
    .getByRole("button", { name: "Approve and run once", exact: true })
    .click();
  await expect(page.locator("#connections-status")).toContainText(
    /Outcome unknown|Outcome unknown/i,
  );
  await expect(page.locator("#connection-result-wrap")).toBeHidden();
  await expect(page.locator("#connection-run")).toBeDisabled();
  expect(calls).toBe(before + 1);
  await page.getByRole("button", { name: "Disconnect", exact: true }).click();
  id = null;
  mode = "json";
  await local.close();
});

test("revoking from another device clears this view and releases its busy state", async ({
  browser,
}) => {
  const connected = await owner({
    op: "connect",
    name: "Other device",
    url: endpoint,
  });
  expect(connected.status).toBe(200);
  id = connected.body.id;
  const local = await browser.newContext();
  const page = await local.newPage();
  await page.goto(base + "/#" + TOKEN);
  await page.locator("#nav-more").evaluate((el) => (el.open = true));
  await page.getByRole("button", { name: "Connections", exact: true }).click();
  await page.getByRole("button", { name: /Other device.*Connected/ }).click();
  await page
    .getByRole("textbox", { name: "Text", exact: true })
    .fill("Received before revocation");
  await page
    .getByRole("button", { name: "Review request", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Approve and run once", exact: true })
    .click();
  await expect(page.locator("#connection-result")).toHaveText(
    "Received before revocation",
  );
  let release, ready, fulfilled;
  const held = new Promise((resolve) => {
    release = resolve;
  });
  const responseReady = new Promise((resolve) => {
    ready = resolve;
  });
  const responseFulfilled = new Promise((resolve) => {
    fulfilled = resolve;
  });
  await page.route("**/api/connections", async (route) => {
    if (
      route.request().method() === "POST" &&
      route.request().postDataJSON()?.op === "execute"
    ) {
      const response = await route.fetch({ maxRetries: 0 });
      ready();
      await held;
      await route.fulfill({ response });
      fulfilled();
    } else await route.continue();
  });
  await page
    .getByRole("textbox", { name: "Text", exact: true })
    .fill("A late response from the old connection");
  await page
    .getByRole("button", { name: "Review request", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Approve and run once", exact: true })
    .click();
  await responseReady;
  await expect(
    page.getByRole("textbox", { name: "Text", exact: true }),
  ).toBeDisabled();
  await owner({ op: "disconnect", id });
  id = null;
  await expect(page.locator("#connection-result-wrap")).toBeHidden({
    timeout: 10000,
  });
  await expect(page.locator("#connections-status")).toContainText(
    "Connection disconnected",
  );
  await expect(page.locator("#connection-connect")).toBeEnabled();
  release();
  await responseFulfilled;
  await expect(page.locator("#connection-result-wrap")).toBeHidden();
  await local.close();
});
