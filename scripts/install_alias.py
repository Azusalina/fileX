#!/usr/bin/env python3
"""Maintain filex's Bash function and interactive login startup, preserving other settings."""
import argparse
import os
import pwd
from pathlib import Path
import re
import shlex
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument("--shell", choices=("auto", "bash", "zsh"), default="auto", help="默认自动识别 Bash / Zsh；其他 shell 用 --command-only")
parser.add_argument('--rc', type=Path, help='自定义 bashrc；指定后不自动修改个人登录配置')
parser.add_argument('--profile', type=Path, help='自定义 Bash 登录配置；默认选择已有的第一个登录配置')
parser.add_argument('--remove', action='store_true')
args = parser.parse_args()
shell = args.shell
if shell == "auto":
    shell = "bash" if args.rc else Path(os.environ.get("SHELL") or pwd.getpwuid(os.getuid()).pw_shell).name
if shell not in {"bash", "zsh"}:
    raise SystemExit("Shell integration supports Bash/Zsh; use install.sh --command-only for other shells")
base_dir = Path(os.environ.get("ZDOTDIR", str(Path.home()))).expanduser() if shell == "zsh" else Path.home()
path = (args.rc or base_dir / ('.zshrc' if shell == 'zsh' else '.bashrc')).expanduser().resolve()
root = Path(__file__).resolve().parents[1]
start, end = '# >>> fileX alias >>>', '# <<< fileX alias <<<'
login_start, login_end = '# >>> fileX login >>>', '# <<< fileX login <<<'

def strip(text, first, last):
    return re.sub(re.escape(first) + r'\n.*?' + re.escape(last) + r'\n?', '', text, flags=re.S)

def write(path, changed):
    original = path.read_text() if path.exists() else ''
    if original == changed:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    mode = path.stat().st_mode & 0o777 if path.exists() else 0o644
    handle, temporary = tempfile.mkstemp(prefix='.filex-shell-', dir=path.parent)
    try:
        with os.fdopen(handle, 'w') as stream:
            stream.write(changed)
        os.chmod(temporary, mode)
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary): os.unlink(temporary)

original = path.read_text() if path.exists() else ''
base = strip(original, start, end)
if not args.remove:
    if re.search(r'^\s*(?:alias\s+filex=|(?:function\s+)?filex\s*\(\))', base, re.M):
        raise SystemExit('发现既有 filex alias / function，拒绝覆盖')
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
'''.replace('@FILEX_LAUNCHER@', shlex.quote(str(root / 'filex.sh')))
    changed = base.rstrip('\n') + '\n\n' + start + '\n' + function + end + '\n'
else:
    changed = base
write(path, changed)
print(f'{"移除" if args.remove else "安装"} filex shell function: {path}')

candidates = [base_dir/'.zprofile'] if shell == 'zsh' else [Path.home()/name for name in ('.bash_profile', '.bash_login', '.profile')]
if args.profile:
    profiles = [args.profile.expanduser().resolve()]
elif args.rc:
    profiles = []  # Custom fixtures/installations must not modify the real home.
elif args.remove:
    profiles = [p.resolve() for p in candidates if p.exists() and login_start in p.read_text()]
else:
    profiles = [next((p for p in candidates if p.exists()), candidates[0]).resolve()]
for profile in profiles:
    original = profile.read_text() if profile.exists() else ''
    base = strip(original, login_start, login_end)
    if args.remove:
        if '# Created by fileX installer' in original and not base.strip():
            profile.unlink()
        else:
            write(profile, base)
    else:
        # POSIX-compatible outer block: .profile may also be used by other shells.
        login = '''case ":${PATH-}:" in
    *":$HOME/.local/bin:"*) ;;
    *) export PATH="$HOME/.local/bin${PATH:+:$PATH}" ;;
esac
if [ -n "${BASH_VERSION-}" ]; then
    case $- in
        *i*)
            if ! declare -F filex >/dev/null && [ -r @FILEX_RC@ ]; then
                . @FILEX_RC@
            fi
            ;;
    esac
fi
'''.replace('@FILEX_RC@', shlex.quote(str(path)))
        if shell == 'zsh':
            # Zsh loads .zshrc for interactive login itself; never source it twice.
            login = login[:login.index('if [ -n')]

        created = '# Created by fileX installer\n' if not profile.exists() or '# Created by fileX installer' in original else ''
        write(profile, base.rstrip('\n')+'\n\n'+login_start+'\n'+created+login+login_end+'\n')
    print(f'{"移除" if args.remove else "安装"} filex login startup: {profile}')
