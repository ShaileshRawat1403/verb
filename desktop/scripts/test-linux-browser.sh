#!/usr/bin/env bash
# Real Linux browser journeys against a Linux debug build with synthetic-boundary enabled.
# No phone, credentials, provider requests, privileged container or service changes.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
binary=${VERB_LINUX_BIN:-"$root/desktop/target/aarch64-unknown-linux-gnu/debug/verb"}
[[ "$binary" = /* && -f "$binary" ]] || { echo 'VERB_LINUX_BIN must name an absolute Linux binary path' >&2; exit 1; }
[[ "$binary" = "$root/"* ]] || { echo 'Linux binary must be inside this worktree' >&2; exit 1; }
[[ -d "$root/desktop/web/node_modules/@playwright/test" ]] || { echo 'Install browser test dependencies first' >&2; exit 1; }
image=verb-browser-linux:local
results="$root/desktop/web/test-results/linux-browser"
mkdir -p "$results"
docker build --iidfile "$results/image.id" -t "$image" -f "$root/desktop/scripts/Dockerfile.browser-linux" "$root/desktop/scripts"
image_id=$(cat "$results/image.id")
docker run --rm --init --memory=2g --cpus=2 --pids-limit=512 --shm-size=256m \
  --mount "type=bind,src=$root,dst=$root,readonly" \
  --mount "type=bind,src=$results,dst=/verb-browser-results" \
  --workdir "$root/desktop/web" \
  -e "VERB_BIN=$binary" -e "VERB_SYNTHETIC_BIN=$binary" \
  -e VERB_TEST_CHROMIUM=/usr/bin/chromium \
  "$image_id" sh -c 'node --version; chromium --version; exec npm run e2e -- --output=/verb-browser-results "$@"' fixture "$@"
