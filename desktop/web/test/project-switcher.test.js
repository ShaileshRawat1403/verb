import assert from "node:assert/strict";
import { test } from "node:test";
import { projectDestination } from "../src/project-switcher.js";

const current = "http://127.0.0.1:4870";

test("opens another local or HTTPS workspace with only the supplied URL", () => {
  assert.equal(projectDestination(" http://127.0.0.1:4871/#destination ", current), "http://127.0.0.1:4871/#destination");
  assert.equal(projectDestination("https://verb.example/#destination", current), "https://verb.example/#destination");
  assert.equal(projectDestination("http://[::1]:4871", current), "http://[::1]:4871/");
});

test("rejects current, relative, credentialed, non-root and insecure remote URLs", () => {
  for (const value of [
    current, current + "/#other", "/#other", "https:verb.example", "javascript:alert(1)",
    "data:text/html,x", "http://verb.example/", "https://u:p@verb.example/",
    "https://verb.example/path", "https://verb.example/?query=1",
  ]) {
    assert.equal(projectDestination(value, current), null, value);
  }
});
