#!/usr/bin/env bash
# Disposable real Linux privilege-separation regression; never runs the fixture on the host.
set -euo pipefail
cd "$(dirname "$0")/.."
binary=${1:-target/release/verb}
binary=$(cd "$(dirname "$binary")" && pwd)/$(basename "$binary")
[[ -f "$binary" ]] || { echo 'build a Linux release binary first' >&2; exit 1; }
docker run --rm --memory=512m --cpus=2 --pids-limit=256 \
  -e VERB_DISPOSABLE_TEST=1 \
  -v "$binary:/tmp/verb:ro" \
  -v "$PWD/scripts/verb-agent-launch:/tmp/verb-agent-launch:ro" \
  -v "$PWD/scripts/test-agent-isolation.py:/tmp/test-agent-isolation.py:ro" \
  ubuntu:24.04 bash -c '
    set -euo pipefail
    apt-get update -qq
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq sudo python3 git acl procps >/tmp/dependencies.log 2>&1
    install -o root -g root -m 755 /tmp/verb /usr/local/bin/verb
    python3 /tmp/test-agent-isolation.py
  '
