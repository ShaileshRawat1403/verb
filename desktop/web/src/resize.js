// Adjustable panels: the navigation width, the spec's proof rail and the terminal height. Each is a
// CSS variable set by dragging a separator (or with arrow keys on it), remembered in this browser.
// Double-click, or Home on the keyboard, puts it back.

/** Keeps a size inside its bounds. */
export function clampSize(value, min, max) {
  return Math.min(max, Math.max(min, Math.round(value)));
}

const read = (key) => {
  try {
    const v = Number.parseInt(localStorage.getItem(key) ?? "", 10);
    return Number.isFinite(v) ? v : null;
  } catch {
    return null;
  }
};
const write = (key, value) => {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, String(value));
  } catch {}
};

/**
 * Wires one separator. `target` gets the CSS variable `name`. `measure(event, startValue, startPos)`
 * turns a pointer position into a size; `direction` is +1 when moving right/down grows the panel.
 */
export function makeResizer({ handle, target, name, key, min, max, initial, axis, direction = 1, onChange }) {
  const apply = (value) => {
    if (value === null) target.style.removeProperty(name);
    else target.style.setProperty(name, `${value}px`);
    handle.setAttribute("aria-valuenow", String(value ?? initial()));
    onChange?.();
  };
  handle.setAttribute("role", "separator");
  handle.setAttribute("tabindex", "0");
  handle.setAttribute("aria-orientation", axis === "x" ? "vertical" : "horizontal");
  handle.setAttribute("aria-valuemin", String(min));
  handle.setAttribute("aria-valuemax", String(max));
  apply(read(key));

  const current = () => read(key) ?? initial();
  const set = (value) => {
    const v = clampSize(value, min, max);
    write(key, v);
    apply(v);
  };
  const reset = () => {
    write(key, null);
    apply(null);
  };

  handle.addEventListener("pointerdown", (event) => {
    if (event.button !== 0) return;
    event.preventDefault();
    const start = axis === "x" ? event.clientX : event.clientY;
    const from = current();
    handle.setPointerCapture(event.pointerId);
    document.documentElement.classList.add("resizing", `resizing-${axis}`);
    const move = (e) => {
      const pos = axis === "x" ? e.clientX : e.clientY;
      const v = clampSize(from + (pos - start) * direction, min, max);
      target.style.setProperty(name, `${v}px`);
      handle.setAttribute("aria-valuenow", String(v));
      onChange?.();
    };
    const up = (e) => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", up);
      handle.removeEventListener("pointercancel", up);
      document.documentElement.classList.remove("resizing", `resizing-${axis}`);
      const pos = axis === "x" ? e.clientX : e.clientY;
      set(from + (pos - start) * direction);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", up);
    handle.addEventListener("pointercancel", up);
  });
  handle.addEventListener("dblclick", reset);
  handle.addEventListener("keydown", (event) => {
    const step = event.shiftKey ? 64 : 16;
    const grow = axis === "x" ? ["ArrowRight", "ArrowLeft"] : ["ArrowDown", "ArrowUp"];
    if (event.key === grow[0]) set(current() + step * direction);
    else if (event.key === grow[1]) set(current() - step * direction);
    else if (event.key === "Home") reset();
    else return;
    event.preventDefault();
  });
  return { set, reset };
}
