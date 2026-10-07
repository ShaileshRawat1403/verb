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
  dark: {
    background: "#101521",
    foreground: "#e5e7f0",
    cursor: "#aa9dff",
    cursorAccent: "#101521",
    selectionBackground: "#665ba677",
    black: "#121623",
    red: "#ff7e91",
    green: "#9cdbba",
    yellow: "#f5cc86",
    blue: "#9cabff",
    magenta: "#c7a6ff",
    cyan: "#80d7e1",
    white: "#e7e8f0",
    brightBlack: "#687184",
    brightRed: "#ff9aa9",
    brightGreen: "#b7ebcf",
    brightYellow: "#ffdca3",
    brightBlue: "#b8c3ff",
    brightMagenta: "#dcc6ff",
    brightCyan: "#a6e6ee",
    brightWhite: "#ffffff",
  },
  // Tuned for contrast on a near-white background: every colour clears 4.5:1 except the bright
  // variants, which programs use for emphasis on top of normal text.
  light: {
    background: "#fbfbfd",
    foreground: "#1f2533",
    cursor: "#6f57e6",
    cursorAccent: "#fbfbfd",
    selectionBackground: "#6f57e633",
    black: "#1f2533",
    red: "#c0283f",
    green: "#16794a",
    yellow: "#8a5a0a",
    blue: "#2d5fd0",
    magenta: "#8b3fc4",
    cyan: "#0e7383",
    white: "#5c6578",
    brightBlack: "#7a8396",
    brightRed: "#d9455b",
    brightGreen: "#1f9158",
    brightYellow: "#a36c12",
    brightBlue: "#4a78e0",
    brightMagenta: "#a35bd8",
    brightCyan: "#178a9b",
    brightWhite: "#2a3142",
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
