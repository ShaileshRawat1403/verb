// End-to-end tests: the real `verb web` binary, a throwaway Git repository, headless Chromium.
// Run: VERB_BIN=../target/debug/verb npm run e2e   (cargo build first)
import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "e2e",
  timeout: 45_000,
  fullyParallel: false,
  workers: 1,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
  use: { trace: "retain-on-failure", viewport: { width: 1440, height: 900 } },
});
