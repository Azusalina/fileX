#!/usr/bin/env python3
"""Install the filex Bash function; cd only after an explicit exit-here action."""
import argparse
import os
from pathlib import Path
import re
import shlex
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument("--rc", type=Path, default=Path.home() / ".bashrc")
parser.add_argument("--remove", action="store_true")
args = parser.parse_args()
path = args.rc.expanduser().resolve()
root = Path(__file__).resolve().parents[1]
start, end = "# >>> fileX alias >>>", "# <<< fileX alias <<<"
original = path.read_text() if path.exists() else ""
pattern = re.compile(re.escape(start) + r"\n.*?" + re.escape(end) + r"\n?", re.S)
base = pattern.sub("", original)
if not args.remove:
    if re.search(r"^\s*(?:alias\s+filex=|(?:function\s+)?filex\s*\(\))", base, re.M):
        raise SystemExit("发现既有 filex alias / function，拒绝覆盖")
    launcher = shlex.quote(str(root / "filex.sh"))
    function = r'''unalias filex 2>/dev/null || true
filex() {
    local filex_cwd_file filex_destination filex_exit_status
    filex_cwd_file="$(command mktemp "${TMPDIR:-/tmp}/filex-cwd.XXXXXX")" || return
    if FILEX_CWD_FILE="$filex_cwd_file" @FILEX_LAUNCHER@ "$@"; then
        filex_exit_status=0
    else
        filex_exit_status=$?
    fi
    if [[ -s "$filex_cwd_file" ]] && IFS= read -r -d '' filex_destination < "$filex_cwd_file"; then
        builtin cd -- "$filex_destination" || filex_exit_status=$?
    fi
    command rm -f -- "$filex_cwd_file"
    return "$filex_exit_status"
}
'''.replace("@FILEX_LAUNCHER@", launcher)
    block = start + "\n" + function + end + "\n"
    changed = base.rstrip("\n") + "\n\n" + block
else:
    changed = base
if changed != original:
    path.parent.mkdir(parents=True, exist_ok=True)
    mode = path.stat().st_mode & 0o777 if path.exists() else 0o644
    handle, temporary = tempfile.mkstemp(prefix=".filex-alias-", dir=path.parent)
    try:
        with os.fdopen(handle, "w") as stream:
            stream.write(changed)
        os.chmod(temporary, mode)
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)
print(f"{'移除' if args.remove else '安装'} filex shell function: {path}")
