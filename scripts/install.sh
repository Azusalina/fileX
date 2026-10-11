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
parser.add_argument('--command-only', action='store_true', help='只安装命令，供 tmux 等非交互 shell 调用')
args = parser.parse_args()
root = Path(os.environ['FILEX_INSTALL_ROOT'])
prefix = args.prefix.expanduser().absolute() if args.prefix else Path.home() / '.local'
launcher = prefix / 'bin/filex'
apps = (prefix / 'share' if args.prefix else Path(os.environ.get('XDG_DATA_HOME', str(prefix / 'share')))) / 'applications'
desktop = apps / 'filex.desktop'
if not args.command_only and desktop.exists() and '# Managed by fileX installer' not in desktop.read_text():
    raise SystemExit(f'拒绝覆盖已有桌面入口: {desktop}')
launcher.parent.mkdir(parents=True, exist_ok=True)
if not args.command_only:
    apps.mkdir(parents=True, exist_ok=True)
if launcher.is_symlink() and launcher.resolve() == root / 'filex.sh':
    pass
elif launcher.exists() or launcher.is_symlink():
    raise SystemExit(f'拒绝覆盖已有命令: {launcher}')
else:
    launcher.symlink_to(root / 'filex.sh')
if args.command_only:
    print(f'已安装命令: {launcher}')
    raise SystemExit(0)
# Desktop Entry Exec quoting has its own grammar (not shell quoting).
command = str(root / 'filex.sh').replace('\\', '\\\\').replace('"', '\\"').replace('`', '\\`').replace('$', '\\$').replace('%', '%%')
desktop.write_text(f'''# Managed by fileX installer
[Desktop Entry]
Type=Application
Name=filex
Comment=Tree file explorer
Exec="{command}" --gui %f
Icon=system-file-manager
Terminal=false
Categories=System;FileTools;FileManager;
MimeType=inode/directory;
StartupWMClass=filex
''')
print(f'已安装命令: {launcher}\nKDE 应用菜单入口: {desktop}\n不修改默认文件管理器。')
PY
