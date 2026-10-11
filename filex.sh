#!/usr/bin/env bash
set -euo pipefail
filex_root="$(dirname -- "$(readlink -f -- "${BASH_SOURCE[0]}")")"
# GUI remains the existing Qt implementation; default TUI is a native Rust process.
for filex_arg in "$@"; do
    if [[ "$filex_arg" == "--gui" ]]; then
        export PYTHONPATH="$filex_root${PYTHONPATH:+:$PYTHONPATH}"
        exec /usr/bin/python -m filex "$@"
    fi
done
if [[ ! -x "$filex_root/target/release/filex" ]]; then
    cargo build --release --locked --manifest-path "$filex_root/Cargo.toml"
fi
exec "$filex_root/target/release/filex" "$@"
