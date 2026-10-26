#!/usr/bin/env bash
set -euo pipefail
filex_root="$(dirname -- "$(readlink -f -- "${BASH_SOURCE[0]}")")"
export FILEX_ROOT="$filex_root"
# --gui opens the graphical filex (../filex-linux-gui) detached from this terminal.
filex_args=()
filex_gui=0
for filex_arg in "$@"; do
    if [[ "$filex_arg" == "--gui" && "$filex_gui" == 0 ]]; then
        filex_gui=1
    else
        filex_args+=("$filex_arg")
    fi
done
if [[ "$filex_gui" == 1 ]]; then
    filex_gui_bin="${FILEX_GUI:-$HOME/.local/bin/filex-gui}"
    if [[ ! -x "$filex_gui_bin" ]]; then
        filex_gui_bin="$filex_root/../filex-linux-gui/target/release/filex-gui"
        if [[ ! -x "$filex_gui_bin" ]]; then
            cargo build --release --locked --manifest-path "$filex_root/../filex-linux-gui/Cargo.toml"
        fi
    fi
    # The GUI has no shell to hand a directory back to; x opens a terminal instead.
    unset FILEX_CWD_FILE
    setsid -f "$filex_gui_bin" "${filex_args[@]}" </dev/null >/dev/null 2>&1
    exit 0
fi
# Native Rust TUI.
if [[ ! -x "$filex_root/target/release/filex" ]]; then
    "$filex_root/scripts/build.sh"
fi
exec "$filex_root/target/release/filex" "$@"
