#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
exec cargo run --quiet -p pcx-play --bin playrun -- "$@"
