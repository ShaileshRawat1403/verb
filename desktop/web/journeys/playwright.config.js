// User journeys against a real, running Verb (the node candidate before a release, or any host).
// They act as a person does and only read the project: nothing is committed or edited. The one
// thing they start is a plain shell terminal, which they end again with `exit`.
//
//   VERB_URL=http://127.0.0.1:4007 VERB_TOKEN=… npx playwright test -c journeys
//
// The token is read from the environment and never printed; traces and videos are off so it is
// not recorded either. Results: journeys-report/summary.json plus a screenshot of any failure.
import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  testMatch: /.*\.journey\.js/,
  globalSetup: "./global-setup.js",
  timeout: 90_000,
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [["list"]],
  outputDir: "../journeys-report/artifacts",
  use: {
    viewport: { width: 1440, height: 900 },
    trace: "off",
    video: "off",
    screenshot: "only-on-failure",
  },
});
