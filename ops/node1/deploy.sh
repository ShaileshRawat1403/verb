#!/usr/bin/env bash
# Builds Verb Desktop for ARM64 and installs it on Node 1, keeping the previous binary for rollback.
#
#   ops/node1/deploy.sh --dry-run     build and show what would happen; touch nothing on the node
#   ops/node1/deploy.sh               install, but do not restart (the running Verb keeps serving)
#   ops/node1/deploy.sh --restart     install and restart Verb -- this ENDS every live terminal
#
# Refuses a dirty working tree: what runs on the node must be a commit anyone can rebuild.
. "$(dirname "$0")/lib.sh"
DRY=0; RESTART=0
for a in "$@"; do case "$a" in --dry-run) DRY=1;; --restart) RESTART=1;; *) die "unknown option $a";; esac; done

cd "$REPO"
[ -z "$(git status --porcelain)" ] || die "working tree is dirty; commit first so the deployment is reproducible"
GIT_SHA="$(git rev-parse --short HEAD)"
command -v cargo-zigbuild >/dev/null || die "cargo-zigbuild not installed (cargo install cargo-zigbuild; brew install zig)"
RUSTUP="$(command -v rustup || echo "$HOME/.cargo/bin/rustup")"
[ -x "$RUSTUP" ] || die "rustup not found; install it from https://rustup.rs"
"$RUSTUP" toolchain install "$RUST_TOOLCHAIN" --profile minimal --target "$TARGET" >/dev/null

say "building $GIT_SHA for $TARGET"
PATH="$(dirname "$RUSTUP"):$PATH" cargo "+$RUST_TOOLCHAIN" zigbuild --release --target "$TARGET" --manifest-path desktop/Cargo.toml
SHA256="$(shasum -a 256 "$BIN" | cut -c1-64)"
say "built $BIN ($SHA256)"

if [ "$DRY" = 1 ]; then
  say "dry run: would copy to the node, back up /usr/local/bin/verb, install, and $( [ $RESTART = 1 ] && echo restart || echo leave the running process alone)"
  exit 0
fi

STAGE="verb-deploy-$GIT_SHA"
say "copying to the node"
scp -o BatchMode=yes -P "$NODE_PORT" "$BIN" "$NODE_USER@$NODE_HOST:$STAGE"
debian "set -e
  D=/root/verb-deployments/$GIT_SHA; mkdir -p \$D
  NEW=/data/data/com.termux/files/home/$STAGE
  [ \"\$(sha256sum \$NEW | cut -c1-64)\" = $SHA256 ] || { echo 'checksum mismatch after copy'; exit 1; }
  PREV=\$(sha256sum /usr/local/bin/verb | cut -c1-64)
  cp -p /usr/local/bin/verb \$D/verb.previous
  install -m 0755 \$NEW /usr/local/bin/verb.new && mv /usr/local/bin/verb.new /usr/local/bin/verb
  rm -f \$NEW
  printf '{\"git\":\"%s\",\"sha256\":\"%s\",\"previous_sha256\":\"%s\",\"installed\":\"%s\"}\n' $GIT_SHA $SHA256 \$PREV \"\$(date -Iseconds)\" > \$D/manifest.json
  echo installed; cat \$D/manifest.json"

if [ "$RESTART" = 1 ]; then
  say "restarting Verb (live terminals end)"
  restart_verb
fi

printf -- '- %s `%s` sha256 `%s`%s\n' "$(date +%F)" "$GIT_SHA" "$SHA256" "$( [ $RESTART = 1 ] && echo ', restarted' || echo ', installed, not restarted')" >> "$HERE/DEPLOYMENTS.md"
say "recorded in ops/node1/DEPLOYMENTS.md -- commit that file"
"$HERE/status.sh"
