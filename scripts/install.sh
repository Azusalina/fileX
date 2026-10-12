#!/usr/bin/env bash
set -euo pipefail
filex_root="$(dirname -- "$(dirname -- "$(readlink -f -- "${BASH_SOURCE[0]}")")")"
export FILEX_INSTALL_ROOT="$filex_root"
/usr/bin/python - "$@" <<'PY'
import argparse
import os
from pathlib import Path
parser = argparse.ArgumentParser()
parser.add_argument('--prefix', type=Path, help='自定义安装根目录，默认 ~/.local')
parser.add_argument('--command-only', action='store_true', help='兼容旧参数；现在始终只安装终端命令')
args = parser.parse_args()
root = Path(os.environ['FILEX_INSTALL_ROOT'])
prefix = args.prefix.expanduser().absolute() if args.prefix else Path.home() / '.local'
launcher = prefix / 'bin/filex'
apps = (prefix / 'share' if args.prefix else Path(os.environ.get('XDG_DATA_HOME', str(prefix / 'share')))) / 'applications'
desktop = apps / 'filex.desktop'
launcher.parent.mkdir(parents=True, exist_ok=True)
if launcher.is_symlink() and launcher.resolve() == root / 'filex.sh':
    pass
elif launcher.exists() or launcher.is_symlink():
    raise SystemExit(f'拒绝覆盖已有命令: {launcher}')
else:
    launcher.symlink_to(root / 'filex.sh')
# Remove only this checkout's obsolete graphical launcher.
if desktop.exists() and '# Managed by fileX installer' in desktop.read_text() and str(root / 'filex.sh').replace('%', '%%') in desktop.read_text():
    desktop.unlink()
print(f'已安装终端命令: {launcher}')
PY
