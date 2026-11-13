# Bench handoff for slice 7b

Functional work through `6d5d8ad` is on `design/bench-consolidation`.
PR #25 targets `codex/reconcile-local-work`.

## Completed

- Slice 0 recorded the design and work plan.
- Slices 1 and 2 delivered the desktop shell, terminal tabs and context strip.
- Slices 3 through 5 delivered Plan, Review and Preview, including work-item
  drafting, proof, handoff and file preview.
- Slice 6 delivered the Board and Needs you.
- Slice 7a delivered Context, Settings and functional cutover.
- Follow-ups covered first run, shared file reads, Git review, proof after
  commit, terminal choice prompts and attribution of published planning drafts.
- Usability screenshots are under `screens/usability-fixes/`.

Six focused browser checks passed in the functional follow-up run:

1. Moved workflows at 1440 px, light.
2. Moved workflows at 1440 px, dark.
3. Moved workflows at 390 px, light.
4. Moved workflows at 390 px, dark.
5. Files and Preview share a file-list request while the host is busy.
6. Git transfer checks, uploads and downloads without losing diverged work.

At `6d5d8ad`, the proof-after-commit and terminal-choice browser regressions
also passed locally, along with the publication receipt unit test.

## Product rules

- Keep terminals central and visually unchanged.
- Keep VS Code editor and browser scope frozen.
- Design for desktop web first.
- Gates warn and record, never block.
- Needs you shows observed signals only.
- Agent output stays marked “reported, unverified”.

## Browser polish notes for 7b

1. BRD/PRD Warnings reasons are hover-only. Show them inline.
2. Opening a spec can shift sidebar content left and clip it. Check horizontal
   scrolling inside the sidebar.
3. Needs you briefly shows its empty state before waiting cards appear.
4. On first run, the right panel asks for a spec or work item when none exist.
   Point to Start a work item.
5. Starting a work item currently requires drafting the BRD with an agent.
   Add a “write it myself” path.
6. Old planning reports stack up in Needs you. Group them or allow bulk clearing.

## Local development

- UI entry point is `desktop/web/index.html`. JavaScript and CSS are in
  `desktop/web/src/`, including `bench-shell.js`, `bench-shell.css`,
  `bench-review.js` and `bench-terminals.js`.
- The Rust web host is `desktop/src/web.rs`. Browser tests are in
  `desktop/web/e2e/`. Rust integration tests are in `desktop/tests/`.
- From the repository root, install and rebuild with
  `npm ci --prefix desktop/web` and `npm run build --prefix desktop/web`.
- Run `npm test --prefix desktop/web`, then run `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings` and `cargo test --all-targets`
  from `desktop/`.
- Build Verb with `cargo build --manifest-path desktop/Cargo.toml --bin verb`.
  Install Chromium from `desktop/web/` with `npx playwright install chromium`.
  Run `VERB_BIN=../target/debug/verb npm run e2e` from `desktop/web/`.

For a browser test instance, clone a local project into a throwaway directory.
For first run, use a new directory with only `git init`. From a fresh login shell
with your real HOME and agent PATH, run:

```sh
cd /absolute/path/to/throwaway-project
VERB_STATE_DIR="$(mktemp -d)" /absolute/path/to/verb web --port 0
```

Open the printed localhost URL including its access key. Each instance gets
its own state directory outside the project. Stop a foreground instance with
Ctrl-C, or send `kill -INT <pid>` to its background process.
