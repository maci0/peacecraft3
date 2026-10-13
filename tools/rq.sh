#!/usr/bin/env bash
# Compact REA query helper for Storm.dll / game.dll.
#   tools/rq.sh procs <binary> <regex>     procedure names matching regex
#   tools/rq.sh strs  <binary> <regex>     strings matching regex
#   tools/rq.sh fn    <binary> <symbol>    full function dossier
#   tools/rq.sh decomp <binary> <symbol>   pseudocode only
#   tools/rq.sh raw   <binary> <address> <len>   hexdump provider bytes
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export GHIDRA_INSTALL_DIR="$ROOT/.tools/ghidra"
export REA_GHIDRA_STARTUP_TIMEOUT_MS="${REA_GHIDRA_STARTUP_TIMEOUT_MS:-900000}"
REA=(bunx rea-agents)

snap_for() { printf '%s/.rea/snapshots/%s.snap' "$ROOT" "$(sha256sum "$1" | cut -c1-64)"; }
tgt_for() {
  case "$1" in
    storm) printf '%s/wc3/game/app/Storm.dll' "$ROOT" ;;
    game)  printf '%s/wc3/game/app/game.dll'  "$ROOT" ;;
    war3)  printf '%s/wc3/game/app/war3.exe'  "$ROOT" ;;
    *)     printf '%s' "$1" ;;
  esac
}

cmd="$1"; key="$2"; pat="${3:-}"; extra="${4:-}"
TGT="$(tgt_for "$key")"
SNAP="$(snap_for "$TGT")"

if [ ! -f "$SNAP" ]; then
  echo "cold analysis: $TGT" >&2
  "${REA[@]}" inspect "$TGT" --snapshot "$SNAP" --json >/dev/null || exit 1
fi

case "$cmd" in
  procs)
    "${REA[@]}" search "$TGT" --pattern "$pat" --kind procedures --mode regex \
        --snapshot "$SNAP" --json 2>/dev/null | python3 -c '
import sys,json
for it in json.load(sys.stdin)["normalized_result"]:
    print(it.get("address"), it.get("value"))'
    ;;
  strs)
    "${REA[@]}" search "$TGT" --pattern "$pat" --kind strings --mode regex \
        --snapshot "$SNAP" --json 2>/dev/null | python3 -c '
import sys,json
for it in json.load(sys.stdin)["normalized_result"]:
    v=it.get("value") or ""
    print(it.get("address"), repr(v)[:200])'
    ;;
  fn|decomp)
    "${REA[@]}" function "$TGT" "$pat" --snapshot "$SNAP" --json 2>/dev/null
    ;;
  raw)
    addr="$pat"; len="${extra:-256}"
    "${REA[@]}" read-bytes "$TGT" --address "$addr" --length "$len" --snapshot "$SNAP" --json 2>/dev/null
    ;;
  *)
    echo "unknown: $cmd" >&2; exit 2 ;;
esac