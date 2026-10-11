#!/usr/bin/env bash
set -euo pipefail
filex_root="$(dirname -- "$(dirname -- "$(readlink -f -- "${BASH_SOURCE[0]}")")")"
exec cargo build --release --locked --manifest-path "$filex_root/Cargo.toml"
