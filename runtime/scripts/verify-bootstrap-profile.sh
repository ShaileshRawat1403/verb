#!/usr/bin/env bash
# Validate the package manifest produced for a selected Verb runtime profile.
# Keep this separate from the expensive package build so the profile contract can
# be exercised by the workflow's seconds-long preflight job.

set -euo pipefail

manifest=''
profile=''

usage() {
  echo "Usage: $0 --manifest <path> --profile <profile>" >&2
  exit 2
}

while (($#)); do
  case "$1" in
    --manifest)
      (($# >= 2)) || usage
      manifest=$2
      shift 2
      ;;
    --profile)
      (($# >= 2)) || usage
      profile=$2
      shift 2
      ;;
    *)
      usage
      ;;
  esac
done

[[ -n "$manifest" && -n "$profile" ]] || usage
[[ -s "$manifest" ]] || {
  echo "Bootstrap package manifest is missing or empty: $manifest" >&2
  exit 1
}

require_package() {
  if ! grep -Fqx -- "$1" "$manifest"; then
    echo "Required package missing from generated bootstrap: $1" >&2
    exit 1
  fi
}

forbid_package() {
  if grep -Fqx -- "$1" "$manifest"; then
    echo "Package forbidden by the selected runtime profile: $1" >&2
    exit 1
  fi
}

for package in ca-certificates libcurl git tar; do
  require_package "$package"
done
forbid_package openssh

case "$profile" in
  core-shell-git-release)
    # The upstream bootstrap always includes command-not-found. At this pinned
    # Termux revision its dependency closure includes Python, so Python is not a
    # profile discriminator. Node and npm are the developer-only additions.
    for forbidden in nodejs-lts npm; do
      forbid_package "$forbidden"
    done
    ;;
  developer-runtime-release)
    for required in nodejs-lts npm python; do
      require_package "$required"
    done
    ;;
  *)
    echo "Unsupported runtime profile: $profile" >&2
    exit 2
    ;;
esac
