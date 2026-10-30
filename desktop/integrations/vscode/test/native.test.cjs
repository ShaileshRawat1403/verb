const { test } = require("node:test");
const assert = require("node:assert/strict");
const { spawn, execFileSync } = require("node:child_process");
const { mkdtempSync, mkdirSync, writeFileSync, rmSync } = require("node:fs");
const { tmpdir } = require("node:os");
const { join, resolve } = require("node:path");
const { createServer } = require("node:net");
const { readWorkspace, WorkspaceConnection } = require("../client.cjs");
test(
  "adapter reads the actual Rust host, preserving project identity and refusing wrong tokens",
  { skip: !process.env.VERB_BIN },
  async (t) => {
    const root = mkdtempSync(join(tmpdir(), "verb-ide-adapter-"));
    const project = join(root, "project"),
      home = join(root, "home");
    mkdirSync(project);
    mkdirSync(home);
    const env = {
      PATH: process.env.PATH,
      HOME: home,
      LANG: "C",
      GIT_CONFIG_NOSYSTEM: "1",
      GIT_CONFIG_GLOBAL: "/dev/null",
      VERB_TOKEN: "verb-adapter-disposable-fixture-token-000000",
      VERB_STATE_DIR: join(root, "state"),
    };
    execFileSync("git", ["init", "-q", project], { env });
    writeFileSync(join(project, "README.md"), "# Synthetic workspace\n");
    const port = await new Promise((resolve) => {
      const s = createServer().listen(0, "127.0.0.1", () => {
        const p = s.address().port;
        s.close(() => resolve(p));
      });
    });
    const child = spawn(
      resolve(process.env.VERB_BIN),
      ["web", "--port", String(port)],
      { cwd: project, env, stdio: ["ignore", "pipe", "pipe"] },
    );
    t.after(async () => {
      if (child.exitCode === null) {
        child.kill();
        await new Promise((resolve) => child.once("exit", resolve));
      }
      rmSync(root, { recursive: true, force: true });
    });
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        child.kill();
        reject(new Error("Synthetic host startup timed out"));
      }, 15000);
      child.stdout.on("data", (data) => {
        if (String(data).includes("Verb web UI")) {
          clearTimeout(timer);
          resolve();
        }
      });
      child.once("error", () => {
        clearTimeout(timer);
        reject(new Error("Synthetic host launch failed"));
      });
      child.once("exit", () => {
        clearTimeout(timer);
        reject(new Error("Synthetic host exited"));
      });
    });
    const origin = `http://127.0.0.1:${port}`;
    const connection = new WorkspaceConnection();
    const first = await connection.connect(origin, env.VERB_TOKEN);
    assert.equal(first.transport, "native-local");
    assert.equal(first.editingAvailable, true);
    assert.equal((await connection.refresh()).workspaceId, first.workspaceId);
    await assert.rejects(readWorkspace(origin, "wrong-token"), /refused/);
    const file = await connection.readFile("README.md");
    assert.equal(file.text, "# Synthetic workspace\n");
    assert.equal(file.workspaceId, first.workspaceId);
    await assert.rejects(connection.readFile("../secret"), /relative/);
    connection.disconnect();
    await assert.rejects(connection.refresh(), /Connect/);
  },
);
