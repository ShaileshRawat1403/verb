import assert from "node:assert/strict";
import { test } from "node:test";
import { readApiResponse } from "../src/api-response.js";

test("an Access redirect gives sign-in guidance without following or parsing it", async () => {
  await assert.rejects(readApiResponse({ type: "opaqueredirect", status: 0 }), {
    code: "access_redirect",
    status: 302,
    message: /Reload the page and sign in/,
  });
});

test("proxy HTML and malformed JSON give safe messages", async () => {
  for (const type of ["text/html", "application/json"]) {
    await assert.rejects(readApiResponse(new Response("<!DOCTYPE html>", {
      headers: { "content-type": type },
    })), { status: 200, message: /unexpected response/ });
  }
  await assert.rejects(readApiResponse(new Response("<!DOCTYPE html>", {
    status: 502,
    headers: { "content-type": "text/html" },
  })), { status: 502, message: /HTTP 502/ });
});

test("JSON success and structured errors keep their existing contract", async () => {
  assert.deepEqual(await readApiResponse(Response.json({ sessions: [] })), { sessions: [] });
  await assert.rejects(readApiResponse(Response.json({
    error: "All terminal slots are in use.",
    code: "capacity",
  }, { status: 503 })), { status: 503, code: "capacity", message: /terminal slots/ });
});
