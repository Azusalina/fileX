#!/usr/bin/env bash
set -euo pipefail
filex_root="$(dirname -- "$(readlink -f -- "${BASH_SOURCE[0]}")")"
export FILEX_ROOT="$filex_root"
# Native Rust TUI.
if [[ ! -x "$filex_root/target/release/filex" ]]; then
    "$filex_root/scripts/build.sh"
fi
exec "$filex_root/target/release/filex" "$@"
