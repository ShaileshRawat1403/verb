export { test, expect, chromium } from "@playwright/test";

export async function runPaletteCommand(page, command) {
  await page.getByRole("button", { name: "Search or run", exact: true }).click();
  await page.locator("#palette-input").fill(command);
  await page.getByRole("option").filter({ has: page.getByText(command, { exact: true }) }).click();
}

export function openWorkspaceView(page, name) {
  return runPaletteCommand(page, `Open ${name}`);
}

export async function terminalAction(page, name) {
  const menu = page.locator("#bench-terminal-menu");
  if (!(await menu.evaluate(node => node.open))) await menu.locator("summary").click();
  return menu.getByRole("button", { name, exact: true });
}
