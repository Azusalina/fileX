"""Filesystem metadata, including true birth time when supported."""
from datetime import datetime
from pathlib import Path
import shutil
import stat
import subprocess
from .core import safe_text


def human_size(size):
    value = float(size)
    for unit in ("B", "KiB", "MiB", "GiB", "TiB", "PiB"):
        if value < 1024 or unit == "PiB":
            return f"{value:.1f} {unit}" if unit != "B" else f"{size} B"
        value /= 1024


def birth_time(path, info):
    value = getattr(info, "st_birthtime", None)
    if value is not None:
        return value
    executable = shutil.which("stat")
    if not executable:
        return None
    try:
        result = subprocess.run([executable, "--printf=%W", "--", str(path)], capture_output=True, text=True, timeout=3)
        value = int(result.stdout) if result.returncode == 0 else 0
        return value if value else None
    except (OSError, ValueError, subprocess.TimeoutExpired):
        return None


def metadata(path):
    path = Path(path)
    try:
        info = path.lstat()
        created = birth_time(path, info)
        directory = stat.S_ISDIR(info.st_mode)
        kind = "目录" if directory else "符号链接" if stat.S_ISLNK(info.st_mode) else "普通文件" if stat.S_ISREG(info.st_mode) else "特殊文件"
        return {
            "name": safe_text(path.name or str(path)), "path": safe_text(path), "kind": kind,
            "created": datetime.fromtimestamp(created).astimezone().strftime("%Y-%m-%d %H:%M:%S %Z") if created is not None else "不可用（文件系统未提供）",
            "modified": datetime.fromtimestamp(info.st_mtime).astimezone().strftime("%Y-%m-%d %H:%M:%S %Z"),
            "size": f"{human_size(info.st_size)} ({info.st_size:,} bytes)", "size_bytes": info.st_size,
            "size_note": "目录自身大小，不包含子文件总量" if directory else "符号链接自身大小" if stat.S_ISLNK(info.st_mode) else "",
        }
    except OSError as exc:
        return {"name": safe_text(path.name), "path": safe_text(path), "error": safe_text(exc)}


def metadata_text(data):
    if "error" in data:
        return f"无法读取信息: {data['error']}"
    return (f"{data['name']}\n{data['kind']}\n\nCreated / 创建时间\n{data['created']}\n\n"
            f"Modified / 修改时间\n{data['modified']}\n\nSize / 大小\n{data['size']}\n{data['size_note']}\n\nPath / 路径\n{data['path']}")
