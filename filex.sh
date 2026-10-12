#!/usr/bin/env bash
set -euo pipefail
filex_root="$(dirname -- "$(readlink -f -- "${BASH_SOURCE[0]}")")"
export FILEX_ROOT="$filex_root"
# Native Rust TUI.
if [[ ! -x "$filex_root/target/release/filex" ]]; then
    cargo build --release --locked --manifest-path "$filex_root/Cargo.toml"
fi
exec "$filex_root/target/release/filex" "$@"
