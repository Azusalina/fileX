"""Wayland text clipboard, shared with the optional Python TUI and launch helpers."""
from pathlib import Path
import subprocess


def copy_path(path):
    result = subprocess.run(["wl-copy", "--type", "text/plain;charset=utf-8"],
                            input=str(Path(path).absolute()).encode(), capture_output=True)
    if result.returncode:
        raise RuntimeError("复制路径失败: " + result.stderr.decode(errors="replace").strip())
