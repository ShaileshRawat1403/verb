// Keystroke-to-echo latency through Verb's web terminal path (WebSocket -> PTY -> WebSocket),
// measured inside a real browser page against a real `verb web`, idle and while another terminal
// floods output like a full-screen TUI redrawing.
//
// Run from desktop/web:  node perf/latency.mjs [path/to/verb]
import { chromium } from "@playwright/test";
import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const bin = resolve(process.argv[2] ?? "../target/debug/verb");
const TOKEN = "bench-token-0000000000000000000000000000";
const repo = mkdtempSync(join(tmpdir(), "verb-bench-"));
execFileSync("git", ["init", "-q", repo]);
const port = 5200 + Math.floor(Math.random() * 300);
const server = spawn(bin, ["web", "--port", String(port)], { cwd: repo, env: { ...process.env, VERB_TOKEN: TOKEN }, stdio: ["ignore", "pipe", "pipe"] });
await new Promise((ok) => server.stdout.on("data", (c) => String(c).includes("Verb web UI") && ok()));
const base = `http://127.0.0.1:${port}`;
const api = async (method, path, body) => {
  const r = await fetch(base + path, { method, headers: { "X-Verb-Token": TOKEN, "Content-Type": "application/json" }, body: body ? JSON.stringify(body) : undefined });
  return r.json();
};
const a = (await api("POST", "/api/terminals", { agent: "shell", isolated: false })).sessionId;
const b = (await api("POST", "/api/terminals", { agent: "shell", isolated: false })).sessionId;
await new Promise((r) => setTimeout(r, 1500));

const browser = await chromium.launch();
const page = await browser.newPage();
await page.goto(base + "/favicon.svg");
const result = await page.evaluate(async ({ port, TOKEN, a, b }) => {
  const enc = new TextEncoder();
  const ws = new WebSocket(`ws://127.0.0.1:${port}/api/terminals/ws?token=${TOKEN}`);
  ws.binaryType = "arraybuffer";
  await new Promise((ok) => (ws.onopen = ok));
  const frame = (id, text) => {
    const idb = enc.encode(id), d = enc.encode(text);
    const f = new Uint8Array(1 + idb.length + d.length);
    f[0] = idb.length; f.set(idb, 1); f.set(d, 1 + idb.length);
    return f;
  };
  let waiting = null;
  const unacked = { [a]: 0, [b]: 0 };
  const received = { [a]: 0, [b]: 0 };
  ws.onmessage = (e) => {
    if (typeof e.data === "string") return;
    const u = new Uint8Array(e.data);
    const id = new TextDecoder().decode(u.slice(1, 1 + u[0]));
    const body = u.slice(1 + u[0]);
    unacked[id] += body.length;
    received[id] += body.length;
    if (unacked[id] >= 16384) { ws.send(JSON.stringify({ type: "ack", id, bytes: unacked[id] })); unacked[id] = 0; }
    if (id === a && waiting && new TextDecoder().decode(body).includes(waiting.mark)) { waiting.done(performance.now() - waiting.t0); waiting = null; }
  };
  ws.send(JSON.stringify({ type: "attach", id: a, rows: 40, cols: 160 }));
  const ws2 = new WebSocket(`ws://127.0.0.1:${port}/api/terminals/ws?token=${TOKEN}`); await new Promise((ok) => (ws2.onopen = ok));
  await new Promise((r) => setTimeout(r, 500));
  ws.send(frame(a, "cat\r"));
  await new Promise((r) => setTimeout(r, 500));
  const measure = async (n) => {
    const out = [];
    for (let i = 0; i < n; i++) {
      const mark = String.fromCharCode(97 + (i % 26));
      const ms = await new Promise((done) => { waiting = { mark, t0: performance.now(), done }; ws.send(frame(a, mark)); setTimeout(() => done(9999), 3000); });
      out.push(ms);
      await new Promise((r) => setTimeout(r, 40));
    }
    out.sort((x, y) => x - y);
    return { p50: +out[Math.floor(n * 0.5)].toFixed(1), p95: +out[Math.floor(n * 0.95)].toFixed(1), max: +out[n - 1].toFixed(1) };
  };
  const idle = await measure(60);
  // A full-screen TUI redraw storm in the other terminal.
  ws2.send(frame(b, "yes '\\033[32m=============================================================================================================================\\033[0m'\r"));
  await new Promise((r) => setTimeout(r, 1000));
  const before = received[b];
  const t0 = performance.now();
  const loaded = await measure(60);
  loaded.floodKBps = Math.round((received[b] - before) / 1024 / ((performance.now() - t0) / 1000));
  ws2.send(frame(b, "\x03"));
  return { idle, loaded };
}, { port, TOKEN, a, b });
console.log(JSON.stringify(result));
await browser.close();
server.kill();
