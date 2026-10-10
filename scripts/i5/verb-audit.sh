#!/usr/bin/env bash
# Verb I-5 compatibility audit driver.
#
# Evidence only. Run it from a real Verb terminal -- never through `run-as`, whose sandbox differs
# from the app's and would make every result below meaningless.
#
#   bash verb-audit.sh env                 record runtime identity
#   bash verb-audit.sh all                 tools, then every workload, resuming where it stopped
#   bash verb-audit.sh unit <unit>         one tool or workload (e.g. tool-bun, dax)
#   bash verb-audit.sh stage <unit> <stage> [workaround-command]
#                                          re-run one stage; an explicit workaround is recorded as such
#   bash verb-audit.sh summary             print the summary table
#
# Launch it from the project directory: repositories are cloned beneath $PWD/verb-audit-work, which
# is the project in both runtimes (the Agent Runtime mounts it at /workspace).
#
# Nothing here classifies a failure. It records what happened; classification is done afterwards,
# by a person, against the log lines this writes.

set -u

STAGE_BUDGET_S="${VERB_AUDIT_BUDGET_S:-1800}"
SCRIPT_PATH="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"

# ---------------------------------------------------------------- pinned workloads (I-5 lock)
DAX_URL="https://github.com/ShaileshRawat1403/dax.git"
DAX_SHA="2e399e62143676a32b80dc56452d8506e9818784"
LLM_URL="https://github.com/simonw/llm.git"
LLM_SHA="bd4d3f185e443f52a6919b9aa1a6d5418f88efa2"   # tag 0.35
GH_URL="https://github.com/cli/cli.git"
GH_SHA="45437bc7eeeb3359bbfddd1742f79de7652fd3e2"    # tag v2.100.0
NEXT_VERSION="16.3.4"                                 # create-next-app and next
BUN_VERSION="1.4.0"                                   # DAX packageManager

# ---------------------------------------------------------------- runtime identity
detect_runtime() {
  if [ -f /etc/debian_version ] && [ -d /workspace ]; then
    echo "agent-runtime"
  elif [ -n "${PREFIX:-}" ] && [ -x "${PREFIX}/bin/bash" ]; then
    echo "userland"
  else
    echo "unknown"
  fi
}

RUNTIME="$(detect_runtime)"
if [ "$RUNTIME" = "unknown" ]; then
  echo "verb-audit: cannot tell which Verb runtime this is; refusing to record evidence." >&2
  exit 2
fi

# A variant is a non-shipped launch of the same runtime (I-5: the Agent Runtime under `proot -q`).
# It selects the same stage definitions as its base runtime, but never shares its evidence.
LABEL="$RUNTIME${VERB_AUDIT_VARIANT:+ +$VERB_AUDIT_VARIANT}"
LABEL="${LABEL// /}"
WORK_ROOT="${VERB_AUDIT_WORK:-$PWD/verb-audit-work}/$LABEL"

# Evidence goes to shared storage when this runtime can see it. The Agent Runtime cannot (only the
# project, its home and the app directory are bound), so it writes beside the work and `sync` from a
# userland terminal mirrors it out.
if [ -d /sdcard/Download ] && touch /sdcard/Download/.verb-audit-probe 2>/dev/null; then
  rm -f /sdcard/Download/.verb-audit-probe
  EVID_ROOT="/sdcard/Download/verb-audit/evidence/$LABEL"
else
  EVID_ROOT="${VERB_AUDIT_WORK:-$PWD/verb-audit-work}/evidence/$LABEL"
fi
OVERRIDES="$EVID_ROOT/env.overrides"
SUMMARY="$EVID_ROOT/summary.tsv"
export OVERRIDES EVID_ROOT WORK_ROOT RUNTIME LABEL
mkdir -p "$WORK_ROOT" "$EVID_ROOT"
[ -f "$OVERRIDES" ] || : > "$OVERRIDES"
[ -f "$SUMMARY" ] || printf 'runtime\tunit\tstage\tattempt\tkind\tstatus\texit\tstart_utc\tduration_s\tdisk_delta_kb\tblocked_by\n' > "$SUMMARY"

now_utc()   { date -u +%Y-%m-%dT%H:%M:%SZ; }
now_epoch() { date +%s; }
disk_kb()   { [ -e "$1" ] && du -sk "$1" 2>/dev/null | awk '{print $1}' || echo 0; }
avail_kb()  { df -k "$WORK_ROOT" 2>/dev/null | awk 'NR==2{print $4}'; }

record_env() {
  local f="$EVID_ROOT/runtime-identity.txt"
  {
    echo "recorded_utc=$(now_utc)"
    echo "runtime=$RUNTIME"
    echo "label=$LABEL (variant=${VERB_AUDIT_VARIANT:-none})"
    echo "script_sha256=$(sha256sum "$SCRIPT_PATH" 2>/dev/null | awk '{print $1}')"
    echo "pwd=$PWD"
    echo "work_root=$WORK_ROOT"
    echo "evidence_root=$EVID_ROOT"
    echo "--- uname"; uname -a
    echo "--- id"; id
    echo "--- os-release"; cat /etc/os-release 2>/dev/null || echo "none"
    echo "--- libc"
    if command -v ldd >/dev/null 2>&1; then ldd --version 2>&1 | head -1; fi
    [ -e /system/bin/linker64 ] && echo "bionic linker present: /system/bin/linker64"
    echo "--- PREFIX=${PREFIX:-unset} HOME=${HOME:-unset}"
    echo "--- PATH=$PATH"
    echo "--- nproc"; nproc 2>/dev/null || grep -c ^processor /proc/cpuinfo
    echo "--- meminfo"; grep -E 'MemTotal|MemAvailable|SwapTotal' /proc/meminfo
    echo "--- df"; df -k "$WORK_ROOT" 2>/dev/null
    echo "--- tools"
    for t in git curl bash make cc clang gcc node npm npx python3 python pip uv rustc cargo go bun; do
      p="$(command -v "$t" 2>/dev/null)"; printf '%-8s %s\n' "$t" "${p:-MISSING}"
    done
  } > "$f" 2>&1
  echo "identity -> $f"
}

# ---------------------------------------------------------------- stage definitions
#
# Each stage names a primary command (the standard path a developer would take in this runtime),
# at most one pre-declared workaround (run only when the primary fails and did not time out), and
# what it requires. A timeout is never "worked around": it is evidence of cost.

UNITS_TOOLS="tool-git tool-node tool-python tool-cc tool-rust tool-go tool-bun"
UNITS_WORKLOADS="dax llm next gh"

stages_of() {
  case "$1" in
    tool-*) echo "provision" ;;
    next)   echo "scaffold install build run" ;;
    *)      echo "clone install build run" ;;
  esac
}

# Sets STAGE_P (primary), STAGE_W (workaround, may be empty) and STAGE_R (space separated unit/stage
# requirements). Globals, not printed lines: a command may itself span several lines.
stage_def() {
  local unit="$1" stage="$2" r="$RUNTIME"
  local P="" W="" R=""
  case "$r:$unit:$stage" in
    # ---------------- tools, userland (Termux-derived apt)
    userland:tool-git:provision)    P='command -v git || pkg install -y git'; ;;
    userland:tool-node:provision)   P='command -v node || pkg install -y nodejs-lts; node --version && npm --version' ;;
    userland:tool-python:provision) P='command -v python || pkg install -y python; python --version && python -m venv --help >/dev/null' ;;
    userland:tool-cc:provision)     P='pkg install -y clang make pkg-config binutils; cc --version && make --version | head -1' ;;
    userland:tool-rust:provision)   P='command -v cargo || pkg install -y rust; rustc --version && cargo --version' ;;
    userland:tool-go:provision)     P='command -v go || pkg install -y golang; go version' ;;
    userland:tool-bun:provision)
      P="npm install -g bun@$BUN_VERSION && bun --version"
      W="curl -fsSL https://bun.sh/install | bash -s bun-v$BUN_VERSION && \"\$HOME/.bun/bin/bun\" --version && echo 'PATH=\$HOME/.bun/bin:\$PATH' >> \"\$OVERRIDES\""
      R="tool-node/provision" ;;
    # ---------------- tools, Agent Runtime (Debian bookworm glibc under qemu, not root)
    agent-runtime:tool-git:provision)    P='git --version' ;;
    agent-runtime:tool-node:provision)
      P='node -e "console.log(process.version)" && npm --version'
      W='NODE_OPTIONS=--jitless node -e "console.log(process.version)" && NODE_OPTIONS=--jitless npm --version && echo "export NODE_OPTIONS=--jitless" >> "$OVERRIDES"' ;;
    agent-runtime:tool-python:provision)
      P='command -v python3 || apt-get install -y python3 python3-venv python3-pip; python3 --version && python3 -m venv --help >/dev/null'
      W='curl -LsSf https://astral.sh/uv/install.sh | env UV_INSTALL_DIR="$HOME/.local/bin" UV_NO_MODIFY_PATH=1 sh && "$HOME/.local/bin/uv" python install 3.12 && "$HOME/.local/bin/uv" --version && echo USE_UV=1 >> "$OVERRIDES"' ;;
    agent-runtime:tool-cc:provision)     P='command -v cc || apt-get install -y build-essential pkg-config; cc --version && make --version | head -1' ;;
    agent-runtime:tool-rust:provision)
      P='command -v cargo || (curl --proto =https --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --no-modify-path); . "$HOME/.cargo/env" 2>/dev/null; rustc --version && cargo --version && echo ". \"\$HOME/.cargo/env\"" >> "$OVERRIDES"' ;;
    agent-runtime:tool-go:provision)
      P='v=$(curl -fsSL "https://raw.githubusercontent.com/cli/cli/'"$GH_SHA"'/go.mod" | awk "/^go /{print \$2}"); case "$v" in *.*.*) ;; *) v="$v.0" ;; esac; echo "go.mod wants $v"; command -v go || (curl -fsSL "https://go.dev/dl/go${v}.linux-arm64.tar.gz" -o "$HOME/.tmp/go.tgz" && mkdir -p "$HOME/.local" && tar -C "$HOME/.local" -xzf "$HOME/.tmp/go.tgz"); export PATH="$HOME/.local/go/bin:$PATH"; go version && echo "export PATH=\"\$HOME/.local/go/bin:\$PATH\"" >> "$OVERRIDES"' ;;
    agent-runtime:tool-bun:provision)
      P="curl -fsSL https://bun.sh/install | bash -s bun-v$BUN_VERSION && \"\$HOME/.bun/bin/bun\" --version && echo 'export PATH=\$HOME/.bun/bin:\$PATH' >> \"\$OVERRIDES\""
      W="BUN_JSC_useJIT=0 \"\$HOME/.bun/bin/bun\" --version && echo 'export PATH=\$HOME/.bun/bin:\$PATH' >> \"\$OVERRIDES\" && echo 'export BUN_JSC_useJIT=0' >> \"\$OVERRIDES\"" ;;

    # ---------------- DAX: Bun 1.4, Rust cargo workspace, turbo monorepo
    *:dax:clone)   P="$(clone_cmd "$DAX_URL" "$DAX_SHA" dax)"; W="$(clone_full_cmd "$DAX_URL" "$DAX_SHA" dax)"; R="tool-git/provision" ;;
    *:dax:install) P='cd dax && bun install --frozen-lockfile'; W='cd dax && bun install'; R="dax/clone tool-bun/provision" ;;
    *:dax:build)   P='cd dax && bun run --cwd packages/dax build --single'; R="dax/install tool-rust/provision tool-cc/provision" ;;
    *:dax:run)
      P='cd dax && b=$(find packages/dax/dist -path "*/bin/dax" -type f | head -1); echo "binary=$b"; [ -n "$b" ] && "$b" --version'
      W='cd dax && bun run --cwd packages/dax dev --version'
      R="dax/build" ;;

    # ---------------- simonw/llm: pip/venv, pydantic-core Rust wheel
    *:llm:clone)   P="$(clone_cmd "$LLM_URL" "$LLM_SHA" llm)"; W="$(clone_full_cmd "$LLM_URL" "$LLM_SHA" llm)"; R="tool-git/provision" ;;
    userland:llm:install)
      P='cd llm && python -m venv .venv && .venv/bin/pip install -e .'
      W='cd llm && ANDROID_API_LEVEL=$(getprop ro.build.version.sdk 2>/dev/null || echo 24) .venv/bin/pip install -e .'
      R="llm/clone tool-python/provision" ;;
    agent-runtime:llm:install)
      P='cd llm && if [ "${USE_UV:-0}" = 1 ]; then "$HOME/.local/bin/uv" venv --python 3.12 .venv && "$HOME/.local/bin/uv" pip install --python .venv/bin/python -e .; else python3 -m venv .venv && .venv/bin/pip install -e .; fi'
      R="llm/clone tool-python/provision" ;;
    userland:llm:build)      P='cd llm && .venv/bin/pip wheel --no-deps -w dist .'; R="llm/install" ;;
    agent-runtime:llm:build) P='cd llm && if [ "${USE_UV:-0}" = 1 ]; then "$HOME/.local/bin/uv" build --wheel -o dist .; else .venv/bin/pip wheel --no-deps -w dist .; fi'; R="llm/install" ;;
    *:llm:run)     P='cd llm && .venv/bin/llm --version && .venv/bin/llm models list | head -5 && .venv/bin/llm plugins'; R="llm/install" ;;

    # ---------------- create-next-app: npm, native SWC, localhost dev server
    *:next:scaffold)
      P="rm -rf next && npx --yes create-next-app@$NEXT_VERSION next --ts --eslint --app --src-dir --no-tailwind --import-alias '@/*' --use-npm --skip-install --disable-git --yes && grep '\"next\"' next/package.json"
      R="tool-node/provision" ;;
    *:next:install) P='cd next && npm install'; R="next/scaffold" ;;
    *:next:build)   P='cd next && npx next build'; W='cd next && npx next build --webpack'; R="next/install" ;;
    *:next:run)
      P="$(dev_server_cmd 'npx next dev -p 3123' 3123)"
      W="$(dev_server_cmd 'npx next dev --webpack -p 3124' 3124)"
      R="next/install" ;;

    # ---------------- cli/cli: Go toolchain, sustained compilation
    *:gh:clone)   P="$(clone_cmd "$GH_URL" "$GH_SHA" gh)"; W="$(clone_full_cmd "$GH_URL" "$GH_SHA" gh)"; R="tool-git/provision" ;;
    *:gh:install) P='cd gh && go mod download && echo downloaded'; W='cd gh && GOTOOLCHAIN=auto go mod download && echo downloaded && echo "export GOTOOLCHAIN=auto" >> "$OVERRIDES"'; R="gh/clone tool-go/provision" ;;
    *:gh:build)   P='cd gh && go build -trimpath -o bin/gh ./cmd/gh && ls -l bin/gh'; R="gh/install" ;;
    *:gh:run)     P='cd gh && ./bin/gh --version && ./bin/gh help | head -5'; R="gh/build" ;;

    *) return 1 ;;
  esac
  STAGE_P="$P"; STAGE_W="$W"; STAGE_R="$R"
}

clone_cmd() {       # shallow fetch of the exact pinned commit
  printf 'rm -rf %s && git init -q %s && cd %s && git remote add origin %s && git fetch --depth 1 origin %s && git checkout -q FETCH_HEAD && git rev-parse HEAD' "$3" "$3" "$3" "$1" "$2"
}
clone_full_cmd() {  # full clone, then check out the pin
  printf 'rm -rf %s && git clone %s %s && cd %s && git checkout -q %s && git rev-parse HEAD' "$3" "$1" "$3" "$3" "$2"
}

# Starts a dev server, polls localhost until it answers or ~28 minutes pass, then kills the whole
# process tree. npx -> node -> next-server is several processes deep, so killing the first pid would
# leave the server holding the port; the tree is walked through /proc, which both runtimes have.
dev_server_cmd() {
  cat <<EOF
cd next || exit 1
$1 > ../next-dev-$2.out 2>&1 &
root=\$!
kill_tree() { for s in \$(grep -l "^PPid:[[:space:]]*\$1\$" /proc/[0-9]*/status 2>/dev/null); do c=\${s#/proc/}; kill_tree "\${c%/status}"; done; kill "\$1" 2>/dev/null; }
ok=1
for i in \$(seq 1 336); do
  code=\$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:$2/ 2>/dev/null)
  if [ "\$code" = 200 ]; then echo "HTTP 200 after \$((i * 5))s"; ok=0; break; fi
  kill -0 "\$root" 2>/dev/null || { echo "server exited before answering"; break; }
  sleep 5
done
echo "--- server output"; cat ../next-dev-$2.out
kill_tree "\$root"
exit \$ok
EOF
}

# ---------------------------------------------------------------- execution and recording

stage_dir()    { echo "$EVID_ROOT/$1"; }
attempt_count() { # number of attempts ever started for unit/stage
  local d n=0; d="$(stage_dir "$1")"
  while [ -e "$d/$2.attempt$((n + 1)).started" ]; do n=$((n + 1)); done
  echo "$n"
}
final_status() { # status of the latest attempt of unit/stage, or nothing
  local n; n="$(attempt_count "$1" "$2")"
  [ "$n" -gt 0 ] || return 0
  grep -h '^status=' "$(stage_dir "$1")/$2.attempt$n.meta" 2>/dev/null | tail -1 | cut -d= -f2
}

write_meta() { # file key=value...
  local f="$1"; shift
  : > "$f.tmp"
  for kv in "$@"; do printf '%s\n' "$kv" >> "$f.tmp"; done
  mv "$f.tmp" "$f"
}

summary_row() { printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$LABEL" "$@" >> "$SUMMARY"; }

# A `.started` with no `.meta` beside it means the previous run never came back: the stage was
# interrupted, most likely because Verb itself was killed. That is a lifecycle fact, recorded here
# and never inferred into a compatibility verdict.
reconcile_interrupted() {
  local unit="$1" stage="$2" d; d="$(stage_dir "$unit")"
  for s in "$d"/"$stage".attempt*.started; do
    [ -e "$s" ] || continue
    local m="${s%.started}.meta"
    [ -e "$m" ] && continue
    local start_utc; start_utc="$(grep '^start_utc=' "$s" | cut -d= -f2)"
    local att; att="$(grep '^attempt=' "$s" | cut -d= -f2)"
    local kind; kind="$(grep '^kind=' "$s" | cut -d= -f2)"
    write_meta "$m" "$(cat "$s")" "status=INTERRUPTED" "exit=UNKNOWN" "end_utc=UNKNOWN" \
      "note=started but never finished; correlate start_utc with dumpsys activity exit-info (LIFECYCLE, Track 2)"
    summary_row "$unit" "$stage" "$att" "$kind" "INTERRUPTED" "UNKNOWN" "$start_utc" "UNKNOWN" "UNKNOWN" "-"
  done
}

next_attempt() { echo $(( $(attempt_count "$1" "$2") + 1 )); }

# run_attempt unit stage kind command -> returns exit code; 124 means the budget ran out
run_attempt() {
  local unit="$1" stage="$2" kind="$3" cmd="$4"
  local d; d="$(stage_dir "$unit")"; mkdir -p "$d"
  local att; att="$(next_attempt "$unit" "$stage")"
  local base="$d/$stage.attempt$att"
  local start_utc start_ep disk_before avail_before
  start_utc="$(now_utc)"; start_ep="$(now_epoch)"
  disk_before="$(disk_kb "$WORK_ROOT/$unit")"; avail_before="$(avail_kb)"
  write_meta "$base.started" "runtime=$LABEL" "unit=$unit" "stage=$stage" "attempt=$att" "kind=$kind" \
    "start_utc=$start_utc" "start_epoch=$start_ep" "budget_s=$STAGE_BUDGET_S" "command=$cmd"

  echo "[$start_utc] $LABEL $unit/$stage attempt $att ($kind)"
  (
    cd "$WORK_ROOT" || exit 97
    # Overrides are what earlier successful workarounds established; they are part of the record.
    set -a; . "$OVERRIDES"; set +a
    echo "### $LABEL $unit/$stage attempt $att ($kind)"
    echo "### command: $cmd"
    echo "### overrides:"; sed 's/^/###   /' "$OVERRIDES"
    echo "### start: $start_utc"
    exec timeout -k 30 "$STAGE_BUDGET_S" bash -c "$cmd"
  ) > "$base.log" 2>&1 < /dev/null
  local rc=$?
  local end_utc end_ep; end_utc="$(now_utc)"; end_ep="$(now_epoch)"
  local dur=$((end_ep - start_ep))
  local disk_after avail_after; disk_after="$(disk_kb "$WORK_ROOT/$unit")"; avail_after="$(avail_kb)"
  local status="OK"
  if [ "$rc" -eq 124 ] || [ "$rc" -eq 137 ]; then
    status="TIMEOUT"
  elif [ "$rc" -ne 0 ]; then
    status="FAILED"
  elif [ "$kind" != "primary" ]; then
    status="OK-WORKAROUND"
  fi
  write_meta "$base.meta" "$(cat "$base.started")" "end_utc=$end_utc" "end_epoch=$end_ep" "duration_s=$dur" \
    "exit=$rc" "status=$status" "unit_disk_kb_before=$disk_before" "unit_disk_kb_after=$disk_after" \
    "fs_avail_kb_before=$avail_before" "fs_avail_kb_after=$avail_after" \
    "peak_rss=SEE_HOST_SAMPLER (adb ps of the app uid between start_epoch and end_epoch); UNKNOWN if no sampler ran" \
    "log=$(basename "$base.log")"
  summary_row "$unit" "$stage" "$att" "$kind" "$status" "$rc" "$start_utc" "$dur" "$((disk_after - disk_before))" "-"
  echo "  -> $status exit=$rc ${dur}s"
  return "$rc"
}

run_stage() { # unit stage [explicit-workaround]
  local unit="$1" stage="$2" explicit="${3:-}"
  stage_def "$unit" "$stage" || { echo "no such stage: $unit/$stage for $RUNTIME" >&2; return 2; }
  local primary="$STAGE_P" workaround="$STAGE_W" requires="$STAGE_R"

  reconcile_interrupted "$unit" "$stage"

  if [ -n "$explicit" ]; then
    run_attempt "$unit" "$stage" "workaround-explicit" "$explicit"
    return $?
  fi

  # BLOCKED and INTERRUPTED are not verdicts, so they are re-evaluated rather than skipped.
  local prior; prior="$(final_status "$unit" "$stage")"
  case "$prior" in
    OK|OK-WORKAROUND) echo "skip $unit/$stage: already $prior"; return 0 ;;
    FAILED|TIMEOUT) echo "skip $unit/$stage: already $prior (use 'stage' to retry explicitly)"; return 1 ;;
  esac
  # A BLOCKED record whose requirements are still unmet would only be written again; say so once.
  if [ "$prior" = "BLOCKED" ]; then
    local still=""
    for req in $requires; do
      case "$(final_status "${req%/*}" "${req#*/}")" in OK|OK-WORKAROUND) ;; *) still="$req" ;; esac
    done
    [ -n "$still" ] && { echo "skip $unit/$stage: still BLOCKED BY $still"; return 3; }
  fi

  for req in $requires; do
    local rs; rs="$(final_status "${req%/*}" "${req#*/}")"
    case "$rs" in
      OK|OK-WORKAROUND) ;;
      *)
        # Name the finding that actually stopped the journey, not the nearest blocked stage.
        local cause="$req (${rs:-not run})"
        if [ "$rs" = "BLOCKED" ]; then
          local rn; rn="$(attempt_count "${req%/*}" "${req#*/}")"
          cause="$(grep -h '^blocked_by=' "$(stage_dir "${req%/*}")/${req#*/}.attempt$rn.meta" | cut -d= -f2-)"
        fi
        local d; d="$(stage_dir "$unit")"; mkdir -p "$d"
        local att; att="$(next_attempt "$unit" "$stage")"
        write_meta "$d/$stage.attempt$att.started" "runtime=$LABEL" "unit=$unit" "stage=$stage" "attempt=$att" "kind=none" "start_utc=$(now_utc)"
        write_meta "$d/$stage.attempt$att.meta" "$(cat "$d/$stage.attempt$att.started")" "status=BLOCKED" "blocked_by=$cause"
        summary_row "$unit" "$stage" "$att" "none" "BLOCKED" "-" "$(now_utc)" "-" "-" "$cause"
        echo "  -> BLOCKED BY $cause"
        return 3 ;;
    esac
  done

  run_attempt "$unit" "$stage" "primary" "$primary"
  local rc=$?
  [ "$rc" -eq 0 ] && return 0
  if [ "$rc" -eq 124 ] || [ "$rc" -eq 137 ]; then
    echo "  budget exhausted: not attempting a workaround (timeouts are cost evidence)"
    return "$rc"
  fi
  if [ -n "$workaround" ]; then
    run_attempt "$unit" "$stage" "workaround" "$workaround"
    rc=$?
  fi
  return "$rc"
}

run_unit() {
  local unit="$1"
  for s in $(stages_of "$unit"); do run_stage "$unit" "$s"; done
}

case "${1:-}" in
  env)     record_env ;;
  all)     record_env; for u in $UNITS_TOOLS $UNITS_WORKLOADS; do run_unit "$u"; done; echo "done: $SUMMARY" ;;
  tools)   record_env; for u in $UNITS_TOOLS; do run_unit "$u"; done ;;
  unit)    run_unit "${2:?unit}" ;;
  stage)   run_stage "${2:?unit}" "${3:?stage}" "${4:-}" ;;
  summary) column -t -s "$(printf '\t')" "$SUMMARY" 2>/dev/null || cat "$SUMMARY" ;;
  sync)    # userland only: mirror the Agent Runtime's evidence, which cannot reach shared storage itself
    src="${VERB_AUDIT_WORK:-$PWD/verb-audit-work}/evidence/agent-runtime"
    [ -d "$src" ] || { echo "no Agent Runtime evidence under $src" >&2; exit 1; }
    mkdir -p /sdcard/Download/verb-audit/evidence && cp -r "$src" /sdcard/Download/verb-audit/evidence/ && echo "synced $src" ;;
  *) sed -n '2,20p' "$SCRIPT_PATH"; exit 2 ;;
esac
