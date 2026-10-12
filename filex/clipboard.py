"""Wayland text clipboard, shared with the optional Python TUI and launch helpers."""
from pathlib import Path
import subprocess


def copy_path(path):
    import os
    child = subprocess.Popen(["wl-copy", "--type", "text/plain;charset=utf-8"], stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    try:
        child.stdin.write(str(Path(path).absolute()).encode())
        child.stdin.close()
        try:
            status = child.wait(timeout=3)
        except subprocess.TimeoutExpired:
            child.kill(); child.wait()
            raise RuntimeError("复制路径超时，请检查 Wayland 剪贴板服务")
        if status:
            os.set_blocking(child.stderr.fileno(), False)
            errors = child.stderr.read(16384) or b""
            raise RuntimeError("复制路径失败: " + errors.decode(errors="replace").strip())
    finally:
        child.stderr.close()
        if child.poll() is None: child.kill(); child.wait()
