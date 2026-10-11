#!/usr/bin/env python3
"""Install a scoped Bash alias without changing the caller's working directory."""
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
    alias = shlex.quote(shlex.quote(str(root / "filex.sh")))
    block = f"{start}\nalias filex={alias}\n{end}\n"
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
print(f"{'移除' if args.remove else '安装'} filex alias: {path}")
