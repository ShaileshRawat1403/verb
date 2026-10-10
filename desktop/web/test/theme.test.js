import { test } from "node:test";
import assert from "node:assert/strict";
import { TERMINAL_THEMES, nextTheme, resolveTheme } from "../src/theme.js";

test("explicit choices win; system follows the OS", () => {
  assert.equal(resolveTheme("light", false), "light");
  assert.equal(resolveTheme("dark", true), "dark");
  assert.equal(resolveTheme("system", true), "light");
  assert.equal(resolveTheme("system", false), "dark");
  assert.equal(resolveTheme("bogus", false), "dark");
});

test("the toggle cycles system → light → dark", () => {
  assert.equal(nextTheme("system"), "light");
  assert.equal(nextTheme("light"), "dark");
  assert.equal(nextTheme("dark"), "system");
});

// WCAG relative luminance, to keep the light terminal readable.
const lum = (hex) => {
  const c = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  const [r, g, b] = c.map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};
const contrast = (a, b) => {
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
};

test("normal terminal colours stay readable in both themes", () => {
  for (const [name, t] of Object.entries(TERMINAL_THEMES)) {
    for (const key of ["foreground", "red", "green", "yellow", "blue", "magenta", "cyan"]) {
      assert.ok(contrast(t[key], t.background) >= 4.5, `${name}.${key} ${contrast(t[key], t.background).toFixed(2)}`);
    }
  }
});
