#!/usr/bin/env bash
# Prepares the package-builder output directory without allowing a legacy
# combined cache to carry one runtime profile's .debs into another profile.
set -euo pipefail

output_dir=${1:?output directory is required}
legacy_cache_matched_key=${2:-}

# actions/cache reports cache-hit=false for a prefix-key restore. A non-empty
# matched key—not cache-hit—is the authoritative signal that legacy data was
# restored and its generated output must be discarded.
if [[ -n "$legacy_cache_matched_key" ]]; then
  rm -rf "$output_dir"
fi

mkdir -p "$output_dir"

if [[ -n "$legacy_cache_matched_key" ]] && [[ -n "$(find "$output_dir" -mindepth 1 -maxdepth 1 -print -quit)" ]]; then
  echo "Legacy runtime output cache was not cleared: $output_dir" >&2
  exit 1
fi
