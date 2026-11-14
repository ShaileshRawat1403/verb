const { test } = require("node:test");
const assert = require("node:assert/strict");
const http = require("node:http");
const { activate } = require("../extension.cjs");
function ide(inputs = [], trusted = true) {
  const commands = new Map(),
    lines = [],
    errors = [],
    prompts = [];
  const context = { subscriptions: [] };
  let provider;
  const documents = [],
    shown = [];
  const vscode = {
    EventEmitter: class {
      event = () => {};
      fire() {}
      dispose() {}
    },
    Uri: {
      from: (parts) => ({
        toString: () => "verb-snapshot://" + parts.authority + parts.path,
      }),
    },
    workspace: {
      isTrusted: trusted,
      registerTextDocumentContentProvider(scheme, value) {
        provider = value;
        return { dispose() {} };
      },
      async openTextDocument(uri) {
        const doc = { uri, text: provider.provideTextDocumentContent(uri) };
        documents.push(doc);
        return doc;
      },
    },
    commands: {
      registerCommand(name, fn) {
        commands.set(name, fn);
        return { dispose() {} };
      },
    },
    window: {
      async showTextDocument(doc) {
        shown.push(doc);
      },
      createOutputChannel() {
        return {
          clear() {
            lines.length = 0;
          },
          appendLine(line) {
            lines.push(line);
          },
          show() {},
          dispose() {},
        };
      },
      async showInputBox(options) {
        prompts.push(options);
        const value = inputs.shift();
        return typeof value === "function" ? value() : value;
      },
      showErrorMessage(error) {
        errors.push(error);
      },
      showInformationMessage() {},
    },
  };
  activate(context, vscode);
  return {
    commands,
    lines,
    errors,
    prompts,
    context,
    documents,
    shown,
    get provider() {
      return provider;
    },
  };
}
test("untrusted workspaces never request credentials or connect", async () => {
  const ui = ide([], false);
  await ui.commands.get("verb.connect")();
  assert.equal(ui.prompts.length, 0);
  assert.match(ui.errors[0], /Trust/);
});
test("disconnect while a password prompt is open cancels the later connection", async () => {
  let release, opened;
  const waiting = new Promise((resolve) => {
    opened = resolve;
  });
  const ui = ide([
    "http://127.0.0.1:1",
    () =>
      new Promise((resolve) => {
        release = resolve;
        opened();
      }),
  ]);
  const pending = ui.commands.get("verb.connect")();
  await waiting;
  ui.commands.get("verb.disconnect")();
  release("fixture-token");
  await pending;
  assert.deepEqual(ui.lines, ["Disconnected. In-memory token cleared."]);
  assert.equal(ui.prompts[1].password, true);
});
test("commands display real capability metadata and clear it on refused refresh", async (t) => {
  let allowed = true;
  const server = http.createServer((req, res) => {
    assert.equal(req.method, "GET");
    assert.equal(req.url, "/api/studio/workspace");
    if (!allowed) {
      res.writeHead(403);
      return res.end("do not display me");
    }
    res.end(
      JSON.stringify({
        schemaVersion: 1,
        workspaceId: "demo",
        transport: "native-local",
        editing: { available: true, maxBytes: 204800 },
        preview: { available: true, kind: "single-file-html" },
      }),
    );
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  t.after(() => {
    server.closeAllConnections();
    server.close();
  });
  const ui = ide([
    `http://127.0.0.1:${server.address().port}`,
    "fixture-token",
  ]);
  await ui.commands.get("verb.connect")();
  assert.ok(ui.lines.some((line) => line === "Verb project: demo"));
  assert.ok(ui.lines.some((line) => line.includes("has not been matched")));
  assert.ok(!ui.lines.join("\n").includes("fixture-token"));
  allowed = false;
  await ui.commands.get("verb.inspect")();
  assert.ok(!ui.lines.join("\n").includes("Verb project: demo"));
  assert.ok(!ui.errors.join("\n").includes("do not display me"));
  ui.commands.get("verb.disconnect")();
});
test("explicit snapshot command uses a read-only provider and disconnect clears its content", async (t) => {
  const text = "# Hello\n",
    calls = [];
  const revision = require("node:crypto")
    .createHash("sha256")
    .update(text)
    .digest("hex");
  const server = http.createServer((req, res) => {
    calls.push(req.method + " " + req.url);
    res.end(
      JSON.stringify(
        req.url === "/api/studio/workspace"
          ? {
              schemaVersion: 1,
              workspaceId: "demo",
              transport: "native-local",
              editing: { available: true, maxBytes: 204800 },
              preview: { available: true, kind: "single-file-html" },
            }
          : {
              workspaceId: "demo",
              file: {
                path: "README.md",
                text,
                bytes: Buffer.byteLength(text),
                revision,
              },
            },
      ),
    );
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  t.after(() => {
    server.closeAllConnections();
    server.close();
  });
  const ui = ide([
    `http://127.0.0.1:${server.address().port}`,
    "fixture-token",
    "README.md",
  ]);
  await ui.commands.get("verb.connect")();
  await ui.commands.get("verb.openSnapshot")();
  assert.equal(ui.shown.length, 1);
  assert.equal(ui.shown[0].text, text);
  assert.match(ui.lines[0], /Read-only saved-file snapshot/);
  assert.ok(calls.every((value) => value.startsWith("GET ")));
  assert.ok(calls.includes("GET /api/studio/file?path=README.md"));
  ui.commands.get("verb.disconnect")();
  assert.match(
    ui.provider.provideTextDocumentContent(ui.shown[0].uri),
    /cleared/,
  );
});
