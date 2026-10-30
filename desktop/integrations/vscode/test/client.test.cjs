const { test } = require("node:test");
const assert = require("node:assert/strict");
const http = require("node:http");
const {
  localOrigin,
  capabilities,
  readWorkspace,
  WorkspaceConnection,
} = require("../client.cjs");
const valid = () => ({
  schemaVersion: 1,
  workspaceId: "fixture-project",
  transport: "native-local",
  editing: { available: true, maxBytes: 204800 },
  preview: { available: true, kind: "single-file-html" },
});
async function host(t, handler) {
  const server = http.createServer(handler);
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  t.after(() => {
    server.closeAllConnections();
    server.close();
  });
  return `http://127.0.0.1:${server.address().port}`;
}
test("only loopback origins without secret-bearing URL components are accepted", () => {
  assert.equal(localOrigin("http://127.0.0.1:4873/"), "http://127.0.0.1:4873");
  assert.equal(localOrigin("http://[::1]:4873"), "http://[::1]:4873");
  for (const url of [
    "https://example.com",
    "http://localhost:12",
    "http://192.168.1.1",
    "file:///tmp/x",
    "http://x:y@127.0.0.1",
    "http://127.0.0.1/#secret",
    "http://127.0.0.1/?token=x",
    "http://127.0.0.1/api",
  ])
    assert.throws(() => localOrigin(url));
});
test("contracts are strict and output contains only known metadata", () => {
  assert.deepEqual(
    capabilities({ ...valid(), token: "secret", reason: "untrusted prose" }),
    {
      workspaceId: "fixture-project",
      transport: "native-local",
      editingAvailable: true,
      previewAvailable: true,
      maxFileBytes: 204800,
    },
  );
  for (const value of [
    null,
    {},
    { ...valid(), schemaVersion: 2 },
    { ...valid(), workspaceId: "line\nbreak" },
    { ...valid(), editing: { available: "true", maxBytes: 204800 } },
    { ...valid(), transport: "unknown" },
  ])
    assert.throws(() => capabilities(value));
});
test("reads exactly the capability endpoint with token in header, never URL", async (t) => {
  const calls = [];
  const origin = await host(t, (req, res) => {
    calls.push(req.url);
    assert.equal(req.method, "GET");
    assert.equal(req.headers["x-verb-token"], "fixture-token");
    assert.equal(req.headers.origin, origin);
    res.end(JSON.stringify(valid()));
  });
  assert.equal(
    (await readWorkspace(origin, "fixture-token")).workspaceId,
    "fixture-project",
  );
  assert.deepEqual(calls, ["/api/studio/workspace"]);
});
test("redirects, permission failures and server text never escape", async (t) => {
  for (const status of [302, 401, 403, 500]) {
    let calls = 0;
    const origin = await host(t, (req, res) => {
      ++calls;
      res.writeHead(status, { Location: "/credential-capture" });
      res.end("SECRET-SERVER-TEXT");
    });
    await assert.rejects(
      readWorkspace(origin, "fixture-token"),
      (error) => !error.message.includes("SECRET"),
    );
    assert.equal(calls, 1);
  }
});
test("invalid token refuses before any HTTP request", () => {
  for (const token of ["", "x\r\ny", "token with spaces"])
    assert.throws(() => readWorkspace("http://127.0.0.1:1", token));
});
test("oversized and malformed responses fail closed", async (t) => {
  for (const body of [
    "x".repeat(20000),
    "{broken",
    JSON.stringify({ schemaVersion: 9 }),
  ]) {
    const origin = await host(t, (req, res) => res.end(body));
    await assert.rejects(readWorkspace(origin, "fixture-token"));
  }
});
test("absolute deadline bounds a response that never ends", async (t) => {
  const origin = await host(t, (req, res) => {
    res.writeHead(200);
    res.write("{");
  });
  await assert.rejects(
    readWorkspace(origin, "fixture-token", { timeoutMs: 40 }),
    /deadline/,
  );
});
test("changed project disconnects instead of rebinding silently", async (t) => {
  let id = "one";
  const origin = await host(t, (req, res) =>
    res.end(JSON.stringify({ ...valid(), workspaceId: id })),
  );
  const client = new WorkspaceConnection();
  assert.equal(
    (await client.connect(origin, "fixture-token")).workspaceId,
    "one",
  );
  id = "two";
  await assert.rejects(client.refresh(), /different project/);
  await assert.rejects(client.refresh(), /Connect/);
});
test("disconnect invalidates a pending connection and clears credentials", async (t) => {
  let response;
  const ready = new Promise((resolve) => {
    response = resolve;
  });
  const origin = await host(t, (req, res) => {
    response(res);
  });
  const client = new WorkspaceConnection();
  const pending = client.connect(origin, "fixture-token");
  const res = await ready;
  client.disconnect();
  res.end(JSON.stringify(valid()));
  await assert.rejects(pending, /cancelled/);
  await assert.rejects(client.refresh(), /Connect/);
});
test("file snapshots require exact workspace, path, byte count and SHA256", () => {
  const { document, relativePath } = require("../client.cjs");
  const text = "hello 🌿\n",
    revision = require("node:crypto")
      .createHash("sha256")
      .update(text)
      .digest("hex");
  const value = {
    workspaceId: "one",
    file: {
      path: "src/hello.txt",
      text,
      bytes: Buffer.byteLength(text),
      revision,
    },
  };
  assert.equal(document(value, "one", "src/hello.txt").text, text);
  for (const bad of [
    "../secret",
    "/etc/passwd",
    "a//b",
    "a/./b",
    "a\\b",
    "a\nb",
  ])
    assert.throws(() => relativePath(bad));
  assert.equal(relativePath("src/a #?.txt"), "src/a #?.txt");
  assert.throws(() => document(value, "two", "src/hello.txt"));
  assert.throws(() => document(value, "one", "another.txt"));
  assert.throws(() =>
    document(
      { ...value, file: { ...value.file, bytes: text.length } },
      "one",
      "src/hello.txt",
    ),
  );
  assert.throws(() =>
    document(
      { ...value, file: { ...value.file, revision: "0".repeat(64) } },
      "one",
      "src/hello.txt",
    ),
  );
});
test("disconnect during a file read prevents returning its content", async (t) => {
  let fileResponse;
  const ready = new Promise((resolve) => {
    fileResponse = resolve;
  });
  const origin = await host(t, (req, res) => {
    if (req.url === "/api/studio/workspace") res.end(JSON.stringify(valid()));
    else fileResponse(res);
  });
  const client = new WorkspaceConnection();
  await client.connect(origin, "fixture-token");
  const pending = client.readFile("README.md");
  const res = await ready;
  client.disconnect();
  res.end("{}");
  await assert.rejects(pending, /Connection changed/);
});
