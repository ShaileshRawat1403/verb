import { test as base } from "@playwright/test";

export { chromium, expect } from "@playwright/test";

// Existing journeys exercise their original views during the navigation transition.
export function useLegacyLayout(context) {
  return context.addInitScript(() => {
    if (location.hostname !== "127.0.0.1" && location.hostname !== "localhost") return;
    if (localStorage.getItem("verb.layout.navigation") === null) {
      localStorage.setItem("verb.layout.navigation", "legacy");
    }
  });
}

export const test = base.extend({
  legacyLayout: [async ({ context }, use) => {
    await useLegacyLayout(context);
    await use();
  }, { auto: true }],
});
