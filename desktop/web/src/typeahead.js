// Local echo for slow links (typeahead prediction), as VS Code Remote and mosh do it.
//
// Over Cloudflare a keystroke's echo took ~250 ms to come back from the phone. When the measured
// round trip is slow, characters you type appear at once as a faint overlay at the cursor, and give
// way to the real echo when it arrives. The overlay is an xterm decoration: it never writes into the
// terminal's contents, so a wrong guess can never corrupt the screen. Every guess is checked against
// what the program then draws; a guess that is not confirmed in time counts as a miss, and two
// misses (a password prompt, a program that draws its own cursor) turn prediction off for a minute.

const SLOW_MS = 40; // below this round trip, nothing is predicted
const MISSES_TO_PAUSE = 2;
const PAUSE_MS = 60_000;

/** Whether one chunk of input is something Verb may draw ahead of time: one printable character. */
export function predictable(data) {
  if (data.length !== 1) return false;
  const code = data.charCodeAt(0);
  return code >= 0x20 && code !== 0x7f;
}

/** The pure part: the round-trip estimate, what is predicted, and when to stop predicting. */
export class Predictor {
  constructor() {
    this.rtt = null;
    this.sentAt = null;
    this.pending = "";
    this.misses = 0;
    this.pausedUntil = 0;
  }

  /** A keystroke left for the server. The next output measures the round trip. */
  sent(now) {
    if (this.sentAt === null) this.sentAt = now;
  }

  /** Output arrived. */
  received(now) {
    if (this.sentAt === null) return;
    const sample = now - this.sentAt;
    this.rtt = this.rtt === null ? sample : this.rtt * 0.7 + sample * 0.3;
    this.sentAt = null;
  }

  active(now) {
    return this.rtt !== null && this.rtt >= SLOW_MS && now >= this.pausedUntil;
  }

  /** How long a guess may wait for the real echo before it counts as a miss. */
  patience() {
    return Math.max(800, (this.rtt ?? 0) * 4);
  }

  /** `actual` is what the screen now shows where the pending characters were drawn. */
  confirm(actual) {
    let k = 0;
    while (k < this.pending.length && actual[k] === this.pending[k]) k += 1;
    this.pending = this.pending.slice(k);
    if (k > 0) this.misses = 0;
    return k;
  }

  miss(now) {
    this.pending = "";
    this.misses += 1;
    if (this.misses >= MISSES_TO_PAUSE) {
      this.misses = 0;
      this.pausedUntil = now + PAUSE_MS;
    }
  }
}

/**
 * Attaches prediction to an xterm.js terminal. Call `input(data)` for every chunk sent to the PTY
 * and `written()` after every chunk of output has been written to the terminal.
 */
export function attachTypeahead(term) {
  const p = new Predictor();
  let anchor = null; // { marker, x }
  let decoration = null;
  let timer = null;

  const clearOverlay = () => {
    decoration?.dispose();
    decoration = null;
  };
  const reset = () => {
    p.pending = "";
    anchor?.marker.dispose();
    anchor = null;
    clearOverlay();
    clearTimeout(timer);
  };
  const render = () => {
    clearOverlay();
    if (!p.pending || !anchor || anchor.marker.isDisposed) return;
    const width = p.pending.length + 1; // the characters and a caret after them
    decoration = term.registerDecoration({ marker: anchor.marker, x: anchor.x, width, layer: "top" });
    decoration?.onRender((el) => {
      el.classList.add("typeahead");
      const cell = el.getBoundingClientRect().width / width || 8;
      el.replaceChildren(
        ...[...p.pending].map((ch) => {
          const span = document.createElement("span");
          span.textContent = ch;
          span.style.width = `${cell}px`;
          return span;
        }),
        Object.assign(document.createElement("i"), { className: "typeahead-caret" }),
      );
      el.style.fontFamily = term.options.fontFamily;
      el.style.fontSize = `${term.options.fontSize}px`;
      el.style.lineHeight = el.style.height;
    });
  };
  const arm = () => {
    clearTimeout(timer);
    timer = setTimeout(() => {
      if (p.pending) {
        p.miss(performance.now());
        reset();
      }
    }, p.patience());
  };

  return {
    input(data) {
      const now = performance.now();
      p.sent(now);
      if (!p.active(now)) return reset();
      if (data === "\x7f" && p.pending) {
        p.pending = p.pending.slice(0, -1);
        if (!p.pending) return reset();
        return render();
      }
      if (!predictable(data)) return reset();
      const buffer = term.buffer.active;
      if (!p.pending) {
        anchor = { marker: term.registerMarker(0), x: buffer.cursorX };
      }
      if (!anchor || anchor.x + p.pending.length + 2 >= term.cols) return reset();
      p.pending += data;
      render();
      arm();
    },
    written() {
      p.received(performance.now());
      if (!p.pending || !anchor || anchor.marker.isDisposed) return;
      const line = term.buffer.active.getLine(anchor.marker.line)?.translateToString(false) ?? "";
      const k = p.confirm(line.slice(anchor.x, anchor.x + p.pending.length));
      if (k === 0) return; // other output (a spinner, a redraw) arrived first; keep waiting
      if (!p.pending) return reset();
      anchor.x += k;
      render();
      arm();
    },
    get rtt() {
      return p.rtt;
    },
    dispose: reset,
  };
}
