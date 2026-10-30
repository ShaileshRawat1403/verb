// Outbound browser-only companion. No control listener, personal profile, project or terminal access.
import { spawn, execFile } from "node:child_process";
import { createInterface } from "node:readline";
import { promisify } from "node:util";
import { isAbsolute } from "node:path";
import { statSync } from "node:fs";
import { Writable } from "node:stream";
import { fileURLToPath } from "node:url";

const args = process.argv.slice(2);
const option = (name) => {
  const i = args.indexOf(name);
  return i < 0 ? null : args[i + 1];
};
if (args.includes("--help")) {
  console.log(
    "node browser-companion.mjs --server https://your-verb-host --chromium /absolute/path/to/chromium [--cloudflare-access]\nPaste a one-use code from Studio → Browser → Connect Mac browser. Keep this process open. Ctrl-C closes the browser and revokes this connection.",
  );
  process.exit(0);
}
let server,
  worker,
  key,
  closing = false,
  closeTask;
let inputLines;
function readCode() {
  return new Promise((resolve, reject) => {
    const rl = createInterface({
      input: process.stdin,
      output: process.stdin.isTTY
        ? new Writable({
            write(_chunk, _encoding, done) {
              done();
            },
          })
        : process.stdout,
      terminal: Boolean(process.stdin.isTTY),
    });
    if (process.stdin.isTTY) {
      process.stderr.write(
        "Paste the one-use browser pairing code, then press Enter: ",
      );
    }
    rl.once("line", (line) => {
      resolve(line.trim());
      rl.close();
      if (process.stdin.isTTY) process.stderr.write("\n");
    });
    rl.once("error", reject);
    rl.once("close", () => reject(new Error("Pairing code required.")));
  });
}
let accessToken;
async function request(path, body, authenticated = true) {
  const response = await fetch(new URL(path, server), {
    method: "POST",
    redirect: "manual",
    signal: AbortSignal.timeout(12000),
    headers: {
      "Content-Type": "application/json",
      ...(authenticated ? { "X-Verb-Browser-Key": key } : {}),
      ...(accessToken ? { "cf-access-token": accessToken } : {}),
    },
    body: JSON.stringify(body),
  });
  if (!response.ok)
    throw new Error(
      "Companion connection refused or expired. Sign in to Cloudflare Access if needed, then pair again.",
    );
  const reader = response.body.getReader();
  let size = 0,
    parts = [];
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    size += value.length;
    if (size > 3 * 1024 * 1024) {
      await reader.cancel();
      throw new Error("Companion response exceeded its limit.");
    }
    parts.push(Buffer.from(value));
  }
  try {
    return JSON.parse(Buffer.concat(parts).toString("utf8"));
  } catch {
    throw new Error("Invalid companion response.");
  }
}
function close() {
  if (closeTask) return closeTask;
  closing = true;
  closeTask = (async () => {
    const exited = () => worker.exitCode !== null || worker.signalCode !== null;
    const wait = async () => {
      if (!exited())
        await Promise.race([
          new Promise((r) => worker.once("exit", r)),
          new Promise((r) => setTimeout(r, 2500)),
        ]);
    };
    if (worker) {
      worker.stdin.end();
      await wait();
      if (!exited()) {
        worker.kill("SIGTERM");
        await wait();
      }
      if (!exited()) worker.kill("SIGKILL");
    }
    if (key)
      await request("/api/browser/companion", { op: "disconnect" }).catch(
        () => {},
      );
  })();
  return closeTask;
}
process.on("SIGINT", () => {
  void close().finally(() => process.exit(0));
});
process.on("SIGTERM", () => {
  void close().finally(() => process.exit(0));
});
try {
  server = new URL(option("--server"));
  if (
    server.username ||
    server.password ||
    server.pathname !== "/" ||
    server.search ||
    server.hash ||
    !["http:", "https:"].includes(server.protocol)
  )
    throw new Error(
      "Use the Verb origin without a path, credentials, query or fragment.",
    );
  if (
    server.protocol !== "https:" &&
    !["localhost", "127.0.0.1", "[::1]"].includes(server.hostname)
  )
    throw new Error("Remote Verb requires HTTPS.");
  if (process.env.NODE_TLS_REJECT_UNAUTHORIZED === "0")
    throw new Error("TLS verification must remain enabled.");
  const executable = option("--chromium");
  if (!executable || !isAbsolute(executable) || !statSync(executable).isFile())
    throw new Error("Supply an existing absolute Chromium executable path.");
  if (args.includes("--cloudflare-access")) {
    let stdout;
    try {
      ({ stdout } = await promisify(execFile)(
        "cloudflared",
        ["access", "token", `-app=${server.origin}`],
        { timeout: 15000, maxBuffer: 16384 },
      ));
    } catch {
      throw new Error(
        "Cloudflare sign-in unavailable. Run cloudflared access login for this Verb origin, then retry.",
      );
    }
    accessToken = stdout.trim();
    if (!/^[A-Za-z0-9_.-]+$/.test(accessToken))
      throw new Error(
        "Cloudflare sign-in token unavailable. Run cloudflared access login for this Verb origin.",
      );
  }
  const code = await readCode();
  if (!/^[a-f0-9]{64}$/.test(code))
    throw new Error("Pairing code must contain 64 hexadecimal characters.");
  const claimed = await request(
    "/api/browser/companion/claim",
    { op: "claim", code },
    false,
  );
  key = claimed.key;
  if (typeof key !== "string" || !/^[a-f0-9]{64}$/.test(key))
    throw new Error("Invalid companion identity.");
  worker = spawn(
    process.execPath,
    [
      fileURLToPath(new URL("./browser-worker.mjs", import.meta.url)),
      server.origin,
    ],
    {
      env: { PATH: "/usr/bin:/bin", VERB_BROWSER_EXECUTABLE: executable },
      stdio: ["pipe", "pipe", "ignore"],
    },
  );
  inputLines = createInterface({ input: worker.stdout, crlfDelay: Infinity })[
    Symbol.asyncIterator
  ]();
  worker.on("error", () => {
    void close();
  });
  worker.stdin.on("error", () => {
    void close();
  });
  console.log(
    "Mac browser connected. Start it from Studio. Logins are temporary; disconnect closes the isolated session.",
  );
  const seen = new Set();
  while (!closing) {
    const poll = await request("/api/browser/companion", { op: "poll" });
    if (!poll.job) {
      await new Promise((r) => setTimeout(r, 500));
      continue;
    }
    const { id, input } = poll.job;
    if (
      typeof id !== "string" ||
      seen.has(id) ||
      !input ||
      JSON.stringify(input).length > 3500
    )
      throw new Error(
        "Invalid or repeated browser action. Nothing was replayed.",
      );
    seen.add(id);
    if (seen.size > 50000)
      throw new Error("Browser action limit reached; pair a new session.");
    worker.stdin.write(JSON.stringify(input) + "\n");
    const next = await Promise.race([
      inputLines.next(),
      new Promise((_, reject) =>
        setTimeout(
          () =>
            reject(
              new Error(
                "Browser timed out. Action outcome is unknown; nothing was replayed.",
              ),
            ),
          22000,
        ).unref(),
      ),
    ]);
    if (next.done || Buffer.byteLength(next.value) > 3 * 1024 * 1024)
      throw new Error("Browser worker ended or exceeded its response limit.");
    let result;
    try {
      result = JSON.parse(next.value);
    } catch {
      throw new Error("Invalid browser worker response.");
    }
    await request("/api/browser/companion", { op: "reply", id, result });
  }
} catch (error) {
  console.error(
    error.message === "fetch failed"
      ? "Browser connection lost. Action outcome may be unknown; pair again."
      : error.message,
  );
  process.exitCode = 1;
} finally {
  await close();
}
