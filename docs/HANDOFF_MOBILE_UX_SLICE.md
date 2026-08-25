# Handoff — Mobile UX slice

> **Status of this document (2026-08-25): a dated snapshot.** Written at the close of the
> mobile touch-and-layout slice on branch `ux/mobile-touch-and-layout-slice`. `docs/HANDOFF.md`
> (2026-08-20) is its predecessor and is kept as written. Current product scope lives in
> `docs/ROADMAP.md`; the live backlog is `docs/BACKLOG.md`.

## State

- Branch `ux/mobile-touch-and-layout-slice`, clean, HEAD `4994fda`, **not pushed**.
- Three commits ahead of `codex/verb-0.1.0-beta.2` (where the branch started):
  - `b6c39ab` Make the mobile terminal usable under touch (first slice)
  - `ba0679c` Fix the mobile dock: visible keys, compact input, tap-true focus (regressions)
  - `4994fda` Diagnostics log follows new entries only while read near the bottom
- `513 tests, 0 failures` on both `fullCliDebug` and `playDebug`; both APKs build.
- Production `device` build installed and verified on the connected phone.
- Validation device: Vivo I2202 (Android 14, arm64, 1080x2400, ~2.0 density), USB-connected.

## What the slice fixed (all verified on the device)

1. **Scrolling no longer opens the keyboard.** The old watcher opened the IME on every
   `Press`; scroll, fling, pinch and long-press selection all begin with one.
2. **The dock survives the IME.** Suggestion chrome (first action, stopped banner, first-run
   hint) collapses *animated* while typing; the canvas has a 96dp floor.
3. **The essential key row is visible** — the first attempt drew both edge-fade gradients
   backwards and the two `DstIn` passes intersected at zero alpha, erasing the whole row.
4. **The command field is a 42dp `BasicTextField`**, not Material's 56dp-minimum
   `OutlinedTextField`; focus ring, placeholder, empty-backspace handling and the
   `terminal_input_field` test tag (on the editable node — the workspace tests target it)
   are preserved.
5. **Tap-to-focus rides the Termux view's own `onSingleTapUp`** via a new
   `TermuxTerminalRuntimeAdapter.onCanvasTap` callback (see lesson 1 below for why).
6. Restarting a **live** session asks first (status pill and overflow menu); restarting a
   dead one stays immediate.
7. `RunsSheet` consumes navigation-bar insets (the exact bug `TerminalDiagnosticsSheet`
   had already documented and fixed; Runs had the same fraction-height pattern).
8. AI explanation sheet scrolls; key rows fade at their scroll edges; key presses tick
   haptically (`HapticFeedbackConstants.KEYBOARD_TAP`); overflow button is a 48dp target.
9. Output auto-scroll is scoped to the Compose fallback view only — with the real canvas
   mounted it no longer spawns per-output animations that fight the user's finger.

Device-verified behavior: tap opens the IME, back hides it, scroll and long-press selection
keep it hidden, keys render above the keyboard, dock sits flush with the IME.

## Hard-won facts the next session must not re-derive

1. **An interop View that claims the touch stream never offers Compose the `Release`.**
   A `pointerInput` watcher on the terminal `AndroidView` sees `Press` and then nothing —
   the Termux `TerminalView` returns true from `onTouchEvent` and takes the rest of the
   gesture. Any Compose-side tap detection there is dead on arrival; gesture verdicts must
   come from the view's own recognizer (`TerminalViewClient.onSingleTapUp` /
   `onLongPress`). Proven with event logging: `event=Press n=1` and silence after.
2. **Vendored `TerminalView.setTextSize(int)` takes pixels**, despite its javadoc saying dp
   (`TerminalRenderer` feeds it straight into `TextPaint.setTextSize`). The existing 9dp
   default is converted with `TypedValue.applyDimension` before the call.
3. **This Vivo ROM suppresses logcat entirely for non-debuggable apps.** The `device`
   build produces zero app logs. Debug on-device with the `fullCliDebug` build (same
   application id and signing key, installs over with `-r`), or verify behaviorally via
   `dumpsys input_method` (`mInputShown`) and screenshots.
4. **`Brush.horizontalGradient` clamps to its end colors beyond its span.** With `DstIn`
   that means a gradient drawn "the wrong way round" erases everything past its span, and
   two opposing passes erase the entire layer. Edge fades must be transparent-at-edge,
   opaque-toward-middle.
5. **`PointerEventType.Cancel` does not exist in Compose 1.7** (BOM 2024.09); cancellation
   surfaces as a consumed release.
6. M3 `Surface(onClick=…)` already enforces the 48dp minimum touch target, so the small
   header pills are larger targets than they look.

## Environment

```bash
export JAVA_HOME=/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home
ADB=/opt/homebrew/share/android-commandlinetools/platform-tools/adb   # adb is NOT on PATH
$ADB devices        # 13958598980007E
./gradlew :app:assembleFullCliDevice        # production build for the phone
./gradlew :app:assembleFullCliDebug         # debuggable build (logs visible)
./gradlew :app:testFullCliDebugUnitTest :app:testPlayDebugUnitTest
$ADB install -r app/build/outputs/apk/fullCli/device/app-fullCli-device.apk
```

Desktop Verb (Rust TUI) for cross-checking behavior: `cargo run --manifest-path
desktop/Cargo.toml -- ui` (see `desktop/README.md`).

## Next up — scoped, not started

These were audited and designed during this session but deliberately left for the next one:

1. **Persist terminal font size and pinch zoom.** Today the size is a hardcoded 9dp and
   zoom resets when the view is recreated. Design already worked out against lesson 2:
   store the applied **px** int in SharedPreferences; at `bindTerminalView` load it (fall
   back to 9dp) and remember it as `baseTextSizePx`; in
   `TermuxTerminalRuntimeAdapter.onScale(scale)` compute
   `newPx = (baseTextSizePx * scale)` clamped to 6–20dp-in-px, `setTextSize` when it
   changed, and persist. `scale` is the accumulated factor since view creation, so
   `base * factor` stays consistent. Do not read `mRenderer.mTextSize` (package-private).
2. **Fling feel:** `TerminalView.java:203` damps flings to `SCALE = 0.25f`; bump to ~0.35f.
   Vendored file — one-line change, keep the comment.
3. **Ask history is rendered eagerly and unbounded** (`AskScreen.kt`, `historyList.drop(1)
   .forEach` inside one `verticalScroll` Column). Cap to the latest ~10 with an
   "…and N earlier runs this session" line, or make it lazy.
4. **Record the slice in `docs/BACKLOG.md`** (a "Completed sprint — Mobile touch and
   layout" section), and note that the NEXT_SPRINT "same layout pattern exists elsewhere"
   item is closed (Runs was the last fraction-height sheet; SemanticLens/Verb/Project/
   FileExplorer sheets were audited clean).

## Still open in the wider backlog (unchanged)

- Landscape layout pass (nothing adapts today).
- Theme ignores the system setting (`VerbTheme` hardcodes dark; light scheme is dead code).
- Agent Runtime "Choose" buttons under the scrolling thumb (NEXT_SPRINT carry-over,
  cosmetic).
- Everything in `docs/BACKLOG.md` §B (device-blocked), §E (paused Android), §G2
  (continuity round-trip acceptance), beta-closure items 4–6.
