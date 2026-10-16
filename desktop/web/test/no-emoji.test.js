// The UI uses one line-icon set (src/icons.js). No emoji or pictographic glyphs, anywhere.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";

// Emoji, dingbats, misc symbols and geometric/technical pictographs. Arrows (U+2190–21FF) and the
// multiplication sign used for "close" are allowed: they are typography, not pictures.
const PICTOGRAPHIC = /[\u{1F000}-\u{1FAFF}\u{2600}-\u{27BF}\u{2B00}-\u{2BFF}\u{25A0}-\u{25FF}\u{2300}-\u{23FF}\u{2440}-\u{245F}]/u;

test("no emoji or pictographic glyphs in the UI source", () => {
  const files = ["index.html", ...readdirSync("src").filter((f) => f.endsWith(".js")).map((f) => `src/${f}`)];
  const hits = [];
  for (const file of files) {
    readFileSync(file, "utf8")
      .split("\n")
      .forEach((line, i) => {
        // Code comments may describe keys (⌘K is shown as text in <kbd>, which is allowed below).
        const visible = line.replace(/⌘K|⌘/g, "");
        const m = visible.match(PICTOGRAPHIC);
        if (m) hits.push(`${file}:${i + 1} ${JSON.stringify(m[0])}`);
      });
  }
  assert.deepEqual(hits, []);
});
