#!/usr/bin/env sh
set -eu

repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$repo_dir"
exec cargo run --manifest-path desktop/Cargo.toml --release -- ui
