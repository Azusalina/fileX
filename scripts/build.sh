#!/usr/bin/env bash
set -euo pipefail
filex_root="$(dirname -- "$(dirname -- "$(readlink -f -- "${BASH_SOURCE[0]}")")")"
if ! command -v cargo >/dev/null || ! command -v rustc >/dev/null; then
    printf 'filex: build needs Rust/Cargo 1.88+. See README or run python3 scripts/check-system.py\n' >&2
    exit 1
fi
filex_rust_version="$(rustc --version)"
if [[ ! "$filex_rust_version" =~ ^rustc\ ([0-9]+)\.([0-9]+) ]] ||
   (( BASH_REMATCH[1] < 1 || BASH_REMATCH[1] == 1 && BASH_REMATCH[2] < 88 )); then
    printf 'filex: Rust 1.88+ required, found %s. Update Rust via rustup or your distro.\n' "$filex_rust_version" >&2
    exit 1
fi
exec cargo build --release --locked --manifest-path "$filex_root/Cargo.toml" "$@"
