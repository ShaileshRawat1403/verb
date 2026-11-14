import { test } from "node:test";
import assert from "node:assert/strict";
import { browserOrigin } from "../runtime/browser-origin.mjs";
test("public companion origins permit ordinary default-port websites, but refuse Verb", () => {
  const own = "https://verb.example.invalid";
  assert.equal(
    browserOrigin("https://docs.example.invalid/path", own),
    "https://docs.example.invalid",
  );
  assert.equal(
    browserOrigin("http://localhost:5173/", own),
    "http://localhost:5173",
  );
  for (const url of [
    own,
    "http://verb.example.invalid/",
    "file:///etc/passwd",
    "data:text/html,test",
    "https://name:secret@other.example.invalid",
  ])
    assert.throws(() => browserOrigin(url, own));
});
test("local host port refusal covers aliases without blocking other development ports", () => {
  const own = "http://127.0.0.1:3005";
  for (const url of [own, "http://localhost:3005", "https://[::1]:3005"])
    assert.throws(() => browserOrigin(url, own));
  assert.equal(
    browserOrigin("http://localhost:5173", own),
    "http://localhost:5173",
  );
});
