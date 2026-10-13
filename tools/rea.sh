#!/usr/bin/env bash
# REA driver for PeaceCraft3.
#
#   tools/rea.sh inspect  <path>
#   tools/rea.sh search   <path> <pattern> [--kind strings|procedures] [--mode literal|regex]
#   tools/rea.sh function <path> <symbol|address>
#   tools/rea.sh trace    <path> <query>
#   tools/rea.sh xrefs    <path> <address>
#   tools/rea.sh snapshot <path>
#   tools/rea.sh doctor
#   tools/rea.sh export   <name> <op> <path> [args...]   # raw MCP op -> rea/<name>.json
#
# Analysis is cached per target in .rea/snapshots/<sha256>.snap so repeat
# queries cost seconds instead of a fresh ~4 minute Ghidra run.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GHIDRA_DIR="$ROOT/.tools/ghidra"
SNAP_DIR="$ROOT/.rea/snapshots"
REA_OUT="$ROOT/rea"
BUNX=(bunx rea-agents)

export GHIDRA_INSTALL_DIR="$GHIDRA_DIR"
export REA_GHIDRA_STARTUP_TIMEOUT_MS="${REA_GHIDRA_STARTUP_TIMEOUT_MS:-900000}"

mkdir -p "$SNAP_DIR" "$REA_OUT"

die() { printf '\033[31m%s\033[0m\n' "$*" >&2; exit 1; }

require_target() {
  [ -f "$1" ] || die "no such file: $1"
}

# snapshot path for a target, keyed by content digest
snap_for() {
  local f="$1"
  local sum
  sum="$(sha256sum "$f" | cut -c1-64)"
  printf '%s/%s.snap' "$SNAP_DIR" "$sum"
}

cmd="${1:-doctor}"; shift || true

case "$cmd" in
  doctor)
    "${BUNX[@]}" doctor --target "$ROOT/wc3/game/app/game.dll"
    ;;

  inspect)
    target="$1"; require_target "$target"; shift
    snap="$(snap_for "$target")"
    if [ ! -f "$snap" ]; then
      echo "→ cold analysis of $target (this takes minutes)" >&2
      "${BUNX[@]}" inspect "$target" --snapshot "$snap" --json >/dev/null
    fi
    "${BUNX[@]}" inspect "$target" --snapshot "$snap" "$@"
    ;;

  search)
    target="$1"; require_target "$target"; pattern="$2"; shift 2
    snap="$(snap_for "$target")"
    if [ ! -f "$snap" ]; then
      echo "→ cold analysis of $target" >&2
      "${BUNX[@]}" inspect "$target" --snapshot "$snap" --json >/dev/null
    fi
    "${BUNX[@]}" search "$target" "$pattern" --snapshot "$snap" "$@"
    ;;

  function)
    target="$1"; require_target "$target"; sym="$2"; shift 2
    snap="$(snap_for "$target")"
    if [ ! -f "$snap" ]; then
      echo "→ cold analysis of $target" >&2
      "${BUNX[@]}" inspect "$target" --snapshot "$snap" --json >/dev/null
    fi
    "${BUNX[@]}" function "$target" "$sym" --snapshot "$snap" "$@"
    ;;

  trace)
    target="$1"; require_target "$target"; query="$2"; shift 2
    snap="$(snap_for "$target")"
    if [ ! -f "$snap" ]; then
      "${BUNX[@]}" inspect "$target" --snapshot "$snap" --json >/dev/null
    fi
    "${BUNX[@]}" trace "$target" "$query" --snapshot "$snap" "$@"
    ;;

  xrefs)
    target="$1"; require_target "$target"; addr="$2"; shift 2
    snap="$(snap_for "$target")"
    if [ ! -f "$snap" ]; then
      "${BUNX[@]}" inspect "$target" --snapshot "$snap" --json >/dev/null
    fi
    "${BUNX[@]}" xrefs "$target" "$addr" --snapshot "$snap" "$@"
    ;;

  snapshot)
    require_target "$1"
    snap="$(snap_for "$1")"
    [ -f "$snap" ] || "${BUNX[@]}" inspect "$1" --snapshot "$snap" --json >/dev/null
    echo "$snap"
    ;;

  export)
    name="$1"; shift
    out="$REA_OUT/$name.json"
    mkdir -p "$REA_OUT"
    "${BUNX[@]}" "$@" --json > "$out"
    echo "$out"
    ;;

  *)
    die "unknown command: $cmd (try: doctor inspect search function trace xrefs snapshot export)"
    ;;
esac