#!/usr/bin/env bash
# Every Desktop check CI runs, locally, stopping at the first failure. Commit only behind it:
#   desktop/scripts/check.sh && git commit ...
# Clippy runs on the CI-pinned toolchain through rustup (Homebrew's cargo-clippy is older and
# misses lints) and on whatever `cargo` is on PATH, because the code must build on both.
set -euo pipefail
cd "$(dirname "$0")/.."
PIN="$(sed -n 's/^  RUST_TOOLCHAIN: "\(.*\)"/\1/p' ../.github/workflows/desktop-ci.yml)"
step() { printf '\n\033[1m== %s\033[0m\n' "$*"; }

step "format";               cargo fmt --check
step "clippy (Rust $PIN)";   PATH="$HOME/.cargo/bin:$PATH" cargo "+$PIN" clippy --all-targets -- -D warnings
step "clippy (local cargo)"; cargo clippy --all-targets -- -D warnings
step "rust tests";           cargo test --all-targets
step "web unit tests";       npm test --prefix web
step "web bundle is current"
npm run build --prefix web >/dev/null
git diff --exit-code --quiet -- web/dist || { echo "web/dist changed: commit the rebuilt bundle"; }
step "browser tests (SHELL unset, as on CI and Node 1)"
cargo build --quiet
(cd web && env -u SHELL VERB_BIN="$PWD/../target/debug/verb" npx playwright test)
printf '\n\033[1;32mAll Desktop checks passed.\033[0m\n'
