#!/usr/bin/env sh
set -eu

repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$repo_dir"
if [ ! -d desktop/web/node_modules ]; then
  npm ci --prefix desktop/web
fi
npm run build --prefix desktop/web
exec cargo run --manifest-path desktop/Cargo.toml --release -- web "$@"
