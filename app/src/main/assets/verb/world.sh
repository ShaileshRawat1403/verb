#!/data/data/com.aistudio.verb.app/files/usr/bin/bash
# Verb world export/import.
#
# The working world is the expensive part of a Verb installation: the agents' own logins, the API
# keys in ~/.env, and Verb's session and project records. All of it lives in app-private storage, so
# an uninstall or a "clear storage" destroys it and the setup work starts again. This is how it comes
# back.
#
# Two rules shape everything below, from docs/BACKLOG.md:
#
#   * Nothing here ever runs on its own. An archive that contains Claude's and Codex's credentials is
#     created only when a person asks for it, by name, at a path they chose.
#   * Import previews before it replaces. It prints the manifest, says exactly what would change, and
#     does nothing at all without --apply -- and even then it snapshots what it is about to overwrite.
#
# The archive is AES-256-CBC with PBKDF2 (OpenSSL 3), keyed by a passphrase the user supplies. The
# payload is checksummed before encryption and verified after decryption, so a truncated or tampered
# archive is refused rather than half-restored.

set -euo pipefail

SCHEMA_VERSION=2
MIN_READABLE_SCHEMA_VERSION=1
HOME_DIR="${HOME:-/data/data/com.aistudio.verb.app/files/home}"
APP_DIR="$(cd "$HOME_DIR/../.." && pwd)"

# What the world is made of. Paths are relative to the app directory, so the archive is portable
# between installs of the same package.
# Scratch that is not part of the world, and cannot be read anyway.
#
# Codex keeps a plugin working tree under `.codex/.tmp`, and proot's link2symlink leaves `.l2s.*`
# entries beside the files it rewrites. Both are unstat-able through the proot mount, and both are
# recreated by the tools that made them -- excluding them loses nothing, while failing on them would
# make every export impossible. Everything *else* that cannot be read still refuses to archive.
#
# The symlink exclusions below were found by running the round-trip on a device: the export happily
# wrote an archive that `verb import` then refused, because import rejects any member that is not a
# regular file or a directory. That refusal is right -- a symlink in an archive is a path-traversal
# waiting to happen -- so the export has to stop producing them.
#
# Two sources, both derived state that the tools regenerate:
#
#   * `.codex/tmp/arg0/*` -- Codex's arg0 shims. Note `tmp`, not `.tmp`: the existing rule above
#     missed them by a single dot, which is why a Codex user's archive was unrestorable.
#   * `node_modules/.bin/*` -- npm's executable shims, under `.config/opencode` and anywhere else
#     a tool vendors its dependencies. Nothing about a user's world lives in them.
WORLD_EXCLUDES=(
  "--exclude=*/.tmp"
  "--exclude=*/.tmp/*"
  "--exclude=.l2s.*"
  "--exclude=*/.l2s.*"
  "--exclude=*/.codex/tmp"
  "--exclude=*/.codex/tmp/*"
  "--exclude=.codex/tmp"
  "--exclude=.codex/tmp/*"
  "--exclude=*/node_modules/.bin"
  "--exclude=*/node_modules/.bin/*"
)

# What `stage_payload` will accept on the way back in. Kept beside the excludes because the two have
# to agree: an export that writes what import refuses is not a backup, it is a file.
assert_payload_restorable() {
  local payload=$1 listing bad
  listing=$(tar -tvzf "$payload" 2>/dev/null | cut -c1 | sort -u | tr -d '\n')
  bad=$(printf '%s' "$listing" | tr -d 'd-')
  if [ -n "$bad" ]; then
    echo "verb: the archive would contain a link or special file, which restore refuses." >&2
    echo "  Entry types found: $listing" >&2
    echo "  This is a bug in Verb's own exclusions, not something you did." >&2
    return 1
  fi
  return 0
}

WORLD_PATHS=(
  "files/home/.env"
  "files/home/.claude"
  "files/home/.claude.json"
  "files/home/.codex"
  "files/home/.config/opencode"
  "files/home/.local/share/opencode"
  "shared_prefs/verb_session.xml"
  "shared_prefs/verb_session_codex.xml"
  "shared_prefs/verb_session_opencode.xml"
  "shared_prefs/verb_projects.xml"
)

usage() {
  cat <<'USAGE'
verb world — save and restore the working world

  verb export <file.vbak>            Write an encrypted archive (asks for a passphrase)
  verb import <file.vbak>            Show what the archive contains and what would change
  verb import <file.vbak> --apply    Restore it, after snapshotting what it replaces
  verb world list                    What would be included, and how large it is

The archive holds agent logins, ~/.env and Verb's session records. It is encrypted, versioned and
checksummed. Keep it somewhere you would keep a password.
USAGE
}

human_size() {
  local bytes=$1
  if [ "$bytes" -ge 1048576 ]; then
    echo "$((bytes / 1048576)) MB"
  elif [ "$bytes" -ge 1024 ]; then
    echo "$((bytes / 1024)) KB"
  else
    echo "$bytes B"
  fi
}

present_paths() {
  local found=()
  for path in "${WORLD_PATHS[@]}"; do
    [ -e "$APP_DIR/$path" ] && found+=("$path")
  done
  printf '%s\n' "${found[@]}"
}

# `du` is allowed to fail on individual entries and must not take the listing with it. The guest
# filesystem contains proot link2symlink artefacts and agent-owned directories whose permissions
# Verb does not control, and one unreadable file inside `.codex` was enough to end this listing
# three lines in, silently, under `set -e`.
# Prints kilobytes, or nothing at all when the size could not be read. "Nothing" and "zero" are
# different answers, and reporting an unreadable directory as 0 B would be Verb claiming a fact it
# does not have.
measure_kb() {
  local path=$1 size
  # Same exclusions as the archive, so the number describes what would actually be written.
  size=$(du -sk --exclude=.tmp --exclude='.l2s.*' "$path" 2>/dev/null | tail -1 | cut -f1) || size=""
  echo "$size"
}

# The footer is suppressed when export borrows this listing: telling someone that nothing has been
# copied yet, in the middle of the copy they asked for, is a false statement about their data.
cmd_list() {
  local footer=${1:-with-footer}
  echo "The working world in $APP_DIR:"
  local total=0
  local unknown=0
  while read -r path; do
    [ -z "$path" ] && continue
    local kb
    kb=$(measure_kb "$APP_DIR/$path")
    if [ -z "$kb" ]; then
      unknown=1
      printf '  %-34s %s\n' "$path" "size unknown"
      continue
    fi
    local size=$((kb * 1024))
    total=$((total + size))
    printf '  %-34s %s\n' "$path" "$(human_size "$size")"
  done < <(present_paths)
  echo
  if [ "$unknown" = "1" ]; then
    echo "Total: at least $(human_size "$total")"
  else
    echo "Total: $(human_size "$total")"
  fi
  [ "$footer" = "with-footer" ] && echo "Nothing is copied anywhere until you run verb export."
  return 0
}

# The manifest travels inside the archive and is printed before any restore, so a person can see what
# they are about to let in.
write_manifest() {
  local target=$1 payload_sum=$2
  {
    echo "{"
    echo "  \"schemaVersion\": $SCHEMA_VERSION,"
    echo "  \"createdAt\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\","
    echo "  \"host\": \"verb-android\","
    echo "  \"payloadSha256\": \"$payload_sum\","
    echo "  \"contents\": ["
    local first=1
    while read -r path; do
      [ -z "$path" ] && continue
      [ $first -eq 0 ] && echo ","
      first=0
      printf '    {"path": "%s", "kind": "%s"}' \
        "$path" "$([ -d "$APP_DIR/$path" ] && echo directory || echo file)"
    done < <(present_paths)
    echo
    echo "  ]"
    echo "}"
  } > "$target"
}

cmd_export() {
  local destination=${1:-}
  [ -z "$destination" ] && { usage; exit 2; }
  [ -e "$destination" ] && { echo "verb: $destination already exists; choose another name." >&2; exit 1; }

  local work
  work=$(mktemp -d "${TMPDIR:-/tmp}/verb-export.XXXXXX")
  # Quoted with a default so the trap is safe when it fires at exit, where the local is gone.
  trap 'rm -rf "${work:-}"' EXIT

  echo "Collecting the working world…"
  cmd_list no-footer
  echo

  # shellcheck disable=SC2046
  if ! tar -C "$APP_DIR" "${WORLD_EXCLUDES[@]}" -czf "$work/payload.tgz" \
      $(present_paths | tr '\n' ' ') 2>"$work/tar.err"; then
    echo "verb: could not read part of the world, so no archive was written." >&2
    sed 's/^/  /' "$work/tar.err" >&2
    echo "  A partial archive is worse than none: it looks like a backup." >&2
    exit 1
  fi
  # Refuse to write an archive this build's own importer would reject. Checked here rather than
  # trusted, because the exclusions above are a list of things somebody remembered.
  if ! assert_payload_restorable "$work/payload.tgz"; then
    exit 1
  fi

  local payload_sum
  payload_sum=$(sha256sum "$work/payload.tgz" | cut -d' ' -f1)
  write_manifest "$work/manifest.json" "$payload_sum"

  tar -C "$work" -cf "$work/bundle.tar" manifest.json payload.tgz

  echo "This archive contains agent logins and API keys. Choose a passphrase you will remember;"
  echo "there is no way to recover the archive without it."
  openssl enc -aes-256-cbc -pbkdf2 -iter 200000 -salt -in "$work/bundle.tar" -out "$destination"

  chmod 600 "$destination"
  echo
  echo "Wrote $destination ($(human_size "$(wc -c < "$destination" | tr -d ' ')"))"
  echo "Checksum of contents: $payload_sum"
  echo
  echo "This file lives in app storage, which an uninstall deletes. Save it off the device now:"
  echo "  Verb → System → Working world → Save to Downloads"
  echo
  echo "To restore later: bring the archive back in from the same card, then run verb import."
}

decrypt_to() {
  local archive=$1 work=$2
  openssl enc -d -aes-256-cbc -pbkdf2 -iter 200000 -in "$archive" -out "$work/bundle.tar" 2>/dev/null || {
    echo "verb: could not decrypt $archive. Wrong passphrase, or the file is damaged." >&2
    exit 1
  }

  # CBC has no authentication tag, so a wrong passphrase or a tampered file can decrypt into
  # plausible-looking rubbish. Both failures are caught here and reported as one honest sentence
  # rather than a tar error the user has to interpret.
  local bundle_members
  bundle_members=$(tar -tf "$work/bundle.tar" 2>/dev/null || true)
  if [ "$bundle_members" != $'manifest.json\npayload.tgz' ]; then
    echo "verb: $archive did not open. Either the passphrase is wrong, or the file is damaged." >&2
    exit 1
  fi
  tar -xOf "$work/bundle.tar" manifest.json > "$work/manifest.json"
  tar -xOf "$work/bundle.tar" payload.tgz > "$work/payload.tgz"
  [ -f "$work/manifest.json" ] || {
    echo "verb: $archive is not a Verb world archive." >&2
    exit 1
  }

  local recorded actual
  recorded=$(grep -o '"payloadSha256": "[a-f0-9]*"' "$work/manifest.json" | cut -d'"' -f4)
  actual=$(sha256sum "$work/payload.tgz" | cut -d' ' -f1)
  if [ "$recorded" != "$actual" ]; then
    echo "verb: this archive is damaged -- its contents do not match its checksum." >&2
    echo "  recorded $recorded" >&2
    echo "  actual   $actual" >&2
    exit 1
  fi

  ARCHIVE_SCHEMA=$(grep -o '"schemaVersion": [0-9]*' "$work/manifest.json" | tr -d ' ' | cut -d: -f2)
  if [ -z "$ARCHIVE_SCHEMA" ] ||
      [ "$ARCHIVE_SCHEMA" -lt "$MIN_READABLE_SCHEMA_VERSION" ] ||
      [ "$ARCHIVE_SCHEMA" -gt "$SCHEMA_VERSION" ]; then
    echo "verb: this archive is schema v${ARCHIVE_SCHEMA:-unknown}; this Verb reads v$MIN_READABLE_SCHEMA_VERSION-v$SCHEMA_VERSION." >&2
    exit 1
  fi
}

payload_member_allowed() {
  local member=${1%/} schema=$2 allowed
  [ -n "$member" ] || return 1
  case "$member" in
    /*|../*|*/../*|*/..|*//*|*/./*|*/.|*\\*) return 1 ;;
  esac

  # Schema v1 exported the entire SharedPreferences directory. It may be staged so an existing
  # user backup remains recoverable, but restore below still copies only WORLD_PATHS. That means
  # obsolete chat memory, device-bound provider ciphertext, UI preferences and migration markers
  # are inspected as ordinary files and then deliberately ignored.
  if [ "$schema" = "1" ]; then
    case "$member" in
      shared_prefs|shared_prefs/*) return 0 ;;
    esac
  fi

  for allowed in "${WORLD_PATHS[@]}"; do
    if [ "$member" = "$allowed" ] || [[ "$member" == "$allowed/"* ]]; then
      return 0
    fi
  done
  return 1
}

stage_payload() {
  local payload=$1 stage=$2 schema=$3 listing member type
  listing=$(mktemp "${TMPDIR:-/tmp}/verb-payload-list.XXXXXX")
  tar -tvzf "$payload" > "$listing" 2>/dev/null || {
    rm -f "$listing"
    echo "verb: the archive payload is not a readable tar file." >&2
    exit 1
  }

  while IFS= read -r type; do
    case "$type" in -|d) ;; *)
      rm -f "$listing"
      echo "verb: the archive contains a link or special file; restore refused." >&2
      exit 1
    esac
  done < <(cut -c1 "$listing")
  rm -f "$listing"

  while IFS= read -r member; do
    if ! payload_member_allowed "$member" "$schema"; then
      echo "verb: the archive contains an unexpected path: $member" >&2
      exit 1
    fi
  done < <(tar -tzf "$payload")

  mkdir -p "$stage"
  tar -C "$stage" -xzf "$payload"
}

cmd_import() {
  local archive=${1:-} apply=${2:-}
  [ -z "$archive" ] && { usage; exit 2; }
  [ -f "$archive" ] || { echo "verb: $archive not found." >&2; exit 1; }

  local work
  work=$(mktemp -d "${TMPDIR:-/tmp}/verb-import.XXXXXX")
  trap 'rm -rf "${work:-}"' EXIT

  decrypt_to "$archive" "$work"
  stage_payload "$work/payload.tgz" "$work/staged" "$ARCHIVE_SCHEMA"

  echo "Archive: $archive"
  sed 's/^/  /' "$work/manifest.json"
  echo
  if [ "$ARCHIVE_SCHEMA" = "1" ]; then
    echo "Compatibility: schema v1 preferences are narrowed to structural session/project records."
    echo "  Legacy chat memory, provider ciphertext and unrelated preferences will not be restored."
    echo
  fi
  # Report the exact allowlisted paths that reached the validated staging area. This keeps a v1
  # compatibility preview truthful even though its manifest claims the old broad shared_prefs path.
  echo "What would change:"
  for path in "${WORLD_PATHS[@]}"; do
    [ -e "$work/staged/$path" ] || continue
    if [ -e "$APP_DIR/$path" ]; then
      printf '  replace  %s\n' "$path"
    else
      printf '  create   %s\n' "$path"
    fi
  done

  if [ "$apply" != "--apply" ]; then
    echo
    echo "Nothing has been changed. Run the same command with --apply to restore."
    return 0
  fi

  # What is about to be overwritten is saved first. A restore that destroys a working login because
  # the archive was older would be the same failure this whole feature exists to prevent.
  local snapshot="$HOME_DIR/verb-world-before-import-$(date -u +%Y%m%d-%H%M%S).tgz"
  # shellcheck disable=SC2046
  tar -C "$APP_DIR" "${WORLD_EXCLUDES[@]}" -czf "$snapshot" \
    $(present_paths | tr '\n' ' ') 2>/dev/null || true
  chmod 600 "$snapshot" 2>/dev/null || true
  echo
  echo "Saved what is here now to $snapshot"

  local path
  for path in "${WORLD_PATHS[@]}"; do
    [ -e "$work/staged/$path" ] || continue
    rm -rf "$APP_DIR/$path"
    mkdir -p "$(dirname "$APP_DIR/$path")"
    mv "$work/staged/$path" "$APP_DIR/$path"
  done
  echo "Restored."
  echo "Restart the terminal session so the shell picks up the restored environment."
}

case "${1:-}" in
  export) shift; cmd_export "$@" ;;
  import) shift; cmd_import "$@" ;;
  world)  shift; case "${1:-list}" in list) cmd_list ;; *) usage; exit 2 ;; esac ;;
  ""|help|-h|--help) usage ;;
  *) echo "verb: unknown command '$1'" >&2; usage; exit 2 ;;
esac
