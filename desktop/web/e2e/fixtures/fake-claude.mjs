#!/usr/bin/env node
// A stand-in for Claude Code in end-to-end tests. It does only what Verb relies on, in Claude Code's
// own formats: a status file under ~/.claude/sessions/<pid>.json, a per-project JSONL log written
// from the first message, bracketed paste, and a numbered permission prompt. Words in a message
// script the cases that are hard to reach with a real agent:
//   [slow]       wait 4 s before the log shows the message
//   [no-id]      log the message without a turn id
//   [crash]      exit before the log shows the message
//   [approve]    ask for permission first (answer 1 to allow, 2 or Esc to decline)
//   [tool-fail]  the edit fails
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import crypto from "node:crypto";

const args = process.argv.slice(2);
const resume = args.indexOf("--resume");
const sessionId = resume >= 0 ? args[resume + 1] : crypto.randomUUID();
const home = os.homedir();
const cwd = process.cwd();
const logDir = path.join(home, ".claude", "projects", cwd.replace(/[^A-Za-z0-9]/g, "-"));
const logPath = path.join(logDir, `${sessionId}.jsonl`);
const statusDir = path.join(home, ".claude", "sessions");
const statusPath = path.join(statusDir, `${process.pid}.json`);
fs.mkdirSync(statusDir, { recursive: true });

const now = () => new Date().toISOString();
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
function status(value) {
  fs.writeFileSync(`${statusPath}.tmp`, JSON.stringify({ pid: process.pid, sessionId, cwd, status: value, statusUpdatedAt: Date.now(), version: "fake" }));
  fs.renameSync(`${statusPath}.tmp`, statusPath);
}
function append(record) {
  fs.mkdirSync(logDir, { recursive: true });
  fs.appendFileSync(logPath, `${JSON.stringify({ sessionId, timestamp: now(), uuid: crypto.randomUUID(), ...record })}\n`);
}
const out = (text) => process.stdout.write(text);
const prompt = () => out("\r\n> ");
process.on("exit", () => {
  try {
    fs.unlinkSync(statusPath);
  } catch {}
});

// A completed fixture bootstrap turn corroborates the initial idle status.
// A real conversation without completed history must start in the terminal.
append({ type: "system", subtype: "turn_duration" });
status("idle");
out("\x1b[?2004h");
out(`Fake Claude Code · ${resume >= 0 ? "resumed" : "new"} conversation ${sessionId}`);
prompt();

let buffer = "";
let pasting = false;
let answer = null;
process.stdin.setRawMode?.(true);
process.stdin.resume();
process.stdin.on("data", (chunk) => {
  let s = chunk.toString("utf8");
  while (s.length) {
    if (s.startsWith("\x1b[200~")) {
      pasting = true;
      s = s.slice(6);
      continue;
    }
    if (s.startsWith("\x1b[201~")) {
      pasting = false;
      s = s.slice(6);
      continue;
    }
    const ch = s[0];
    s = s.slice(1);
    if (answer) {
      if (ch === "1" || ch === "y") answer(true);
      else if (ch === "2" || ch === "n" || ch === "\x1b" || ch === "\x03") answer(false);
      continue;
    }
    if (ch === "\x03") {
      out("^C");
      buffer = "";
      prompt();
      continue;
    }
    if (ch === "\r" && !pasting) {
      const text = buffer.trim();
      buffer = "";
      if (text) turn(text);
      else prompt();
      continue;
    }
    buffer += ch;
    out(ch === "\n" ? "\r\n" : ch);
  }
});

async function turn(text) {
  status("busy");
  out("\r\n");
  if (text.includes("[crash]")) process.exit(3);
  if (text.includes("[slow]")) await sleep(4000);
  const user = { type: "user", promptSource: "typed", origin: { kind: "human" }, message: { role: "user", content: text } };
  if (text.includes("[no-id]")) user.uuid = undefined;
  append(user);
  if (text.includes("[approve]")) {
    out("Edit notes.md\r\nDo you want to proceed?\r\n > 1. Yes\r\n   2. No, and tell Claude what to do differently (esc)\r\n");
    const allowed = await new Promise((resolve) => (answer = resolve));
    answer = null;
    // Like Claude Code, the dialog is redrawn away once it is answered.
    out("\x1b[4A\r\x1b[J");
    if (!allowed) {
      append({ type: "assistant", message: { content: [{ type: "text", text: "Okay, I did not change anything." }] } });
      finish();
      return;
    }
  }
  const tool = `toolu_${crypto.randomUUID().slice(0, 8)}`;
  append({ type: "assistant", message: { content: [{ type: "tool_use", id: tool, name: "Edit", input: { file_path: path.join(cwd, "notes.md") } }] } });
  const failed = text.includes("[tool-fail]");
  if (!failed) fs.appendFileSync(path.join(cwd, "notes.md"), `- ${text}\n`);
  await sleep(300);
  append({ type: "user", message: { content: [{ type: "tool_result", tool_use_id: tool, is_error: failed }] } });
  append({ type: "assistant", message: { content: [{ type: "text", text: failed ? "The edit failed: notes.md is read-only." : `Done. I noted: ${text}` }] } });
  finish();
}

function finish() {
  append({ type: "system", subtype: "turn_duration" });
  status("idle");
  out("Done.");
  prompt();
}
