"""Configured application launchers: argument arrays, never a shell command."""
from pathlib import Path
import os
import shutil
import subprocess
from .config import extension_rule


def default_opener(node, config):
    return extension_rule(node.path, config.open_defaults, node.is_dir) or "system"


def opener_choices(node, config):
    default = default_opener(node, config)
    ordered = [default] + [name for name in config.openers if name != default]
    return [(name, config.openers[name]["label"] + (" [默认]" if name == default else "") +
             (" [未安装]" if not shutil.which(config.openers[name]["command"][0]) else "")) for name in ordered]


def prepare_open(path, config, method):
    opener = config.openers[method]
    root = Path(os.environ.get("FILEX_ROOT", Path(__file__).resolve().parents[1]))
    command = [arg.replace("{filex}", str(root)).replace("{toolkit}", str(root.parent / "toolkit")).replace("{path}", str(Path(path).absolute())) for arg in opener["command"]]
    if not shutil.which(command[0]):
        raise RuntimeError(f"打开程序未安装或不可执行: {command[0]}；请安装或编辑 openers 配置")
    return command, opener["terminal"]


def launch_open(path, config, method):
    command, terminal = prepare_open(path, config, method)
    if config.openers[method].get("copy_path", False):
        from .clipboard import copy_path
        copy_path(path)
    return subprocess.Popen(command, env={k:v for k,v in os.environ.items() if k != "FILEX_CWD_FILE"}, cwd=(Path(path) if Path(path).is_dir() else Path(path).absolute().parent) if terminal else None, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
