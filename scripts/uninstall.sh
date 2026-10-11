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
args = parser.parse_args()
root = Path(os.environ['FILEX_INSTALL_ROOT'])
prefix = args.prefix.expanduser().absolute() if args.prefix else Path.home() / '.local'
launcher = prefix / 'bin/filex'
desktop = (prefix / 'share' if args.prefix else Path(os.environ.get('XDG_DATA_HOME', str(prefix / 'share')))) / 'applications/filex.desktop'
if launcher.is_symlink() and launcher.resolve() == root / 'filex.sh':
    launcher.unlink()
if desktop.exists() and '# Managed by fileX installer' in desktop.read_text() and str(root / 'filex.sh').replace('%', '%%') in desktop.read_text():
    desktop.unlink()
print('已移除此项目的启动入口；保留源码、配置和用户文件。')
PY
