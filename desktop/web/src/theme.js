// Light and dark themes. The CSS reads every colour from tokens (style.css top); this module picks
// which set applies and gives xterm.js a matching palette, which it cannot read from CSS.

export const THEMES = ["system", "light", "dark"];

/** The theme actually shown for a preference. */
export function resolveTheme(preference, systemPrefersLight) {
  if (preference === "light" || preference === "dark") return preference;
  return systemPrefersLight ? "light" : "dark";
}

/** The next preference when cycling the toggle. */
export function nextTheme(preference) {
  return THEMES[(THEMES.indexOf(preference) + 1) % THEMES.length] ?? "system";
}

export const TERMINAL_THEMES = {
  // Monochrome like the rest of Verb. ANSI blue still has to read as distinct (directories in `ls`,
  // branches in git), so it is a desaturated slate rather than a saturated blue.
  dark: {
    background: "#080808",
    foreground: "#e4e4e4",
    cursor: "#fafafa",
    cursorAccent: "#080808",
    selectionBackground: "#ffffff26",
    black: "#1a1a1a",
    red: "#ff7a7a",
    green: "#8fcd7f",
    yellow: "#f2c94c",
    blue: "#a9b6c6",
    magenta: "#cfaedb",
    cyan: "#94cbc6",
    white: "#d4d4d4",
    brightBlack: "#6b6b6b",
    brightRed: "#ffa0a0",
    brightGreen: "#addd9f",
    brightYellow: "#f7da7c",
    brightBlue: "#c5cdd8",
    brightMagenta: "#dfc6e8",
    brightCyan: "#b3dcd8",
    brightWhite: "#ffffff",
  },
  // Tuned for contrast on a near-white background: every colour clears 4.5:1 except the bright
  // variants, which programs use for emphasis on top of normal text.
  light: {
    background: "#fbfbfa",
    foreground: "#1d1d1d",
    cursor: "#161616",
    cursorAccent: "#fbfbfa",
    selectionBackground: "#16161624",
    black: "#1d1d1d",
    red: "#b42f2f",
    green: "#2c6b27",
    yellow: "#7a5300",
    blue: "#46546a",
    magenta: "#7a3f8f",
    cyan: "#1d6662",
    white: "#5f5f5f",
    brightBlack: "#8a8a8a",
    brightRed: "#c43535",
    brightGreen: "#2f7a2a",
    brightYellow: "#946400",
    brightBlue: "#5d6a7d",
    brightMagenta: "#8f4fa6",
    brightCyan: "#2a7f79",
    brightWhite: "#111111",
  },
};


export function createThemeController({ onChange } = {}) {
  const media = window.matchMedia?.("(prefers-color-scheme: light)");
  let preference = "system";
  try {
    preference = localStorage.getItem("verb.theme") || "system";
  } catch {
    // Without storage the system setting decides.
  }
  const resolved = () => resolveTheme(preference, Boolean(media?.matches));
  const apply = () => {
    const root = document.documentElement;
    if (preference === "system") root.removeAttribute("data-theme");
    else root.setAttribute("data-theme", preference);
    root.dataset.resolvedTheme = resolved();
    // A short, scoped transition so switching feels smooth but nothing animates on load.
    root.classList.add("theme-switching");
    clearTimeout(apply.timer);
    apply.timer = setTimeout(() => root.classList.remove("theme-switching"), 350);
    onChange?.(resolved(), preference);
  };
  media?.addEventListener?.("change", () => preference === "system" && apply());
  document.documentElement.dataset.resolvedTheme = resolved();
  const controller = {
    get preference() {
      return preference;
    },
    get resolved() {
      return resolved();
    },
    set(next) {
      preference = THEMES.includes(next) ? next : "system";
      try {
        localStorage.setItem("verb.theme", preference);
      } catch {
        // The choice still applies for this page.
      }
      apply();
    },
    cycle() {
      controller.set(nextTheme(preference));
    },
  };
  if (preference !== "system") document.documentElement.setAttribute("data-theme", preference);
  return controller;
}
