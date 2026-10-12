"""Return a selected directory to the calling filex shell function."""
from pathlib import Path
import os


def exit_directory(path):
    path = Path(path)
    path.lstat()
    directory = path if path.is_dir() else path.parent
    if not directory.is_dir():
        raise ValueError("目标目录已移除或不可访问")
    destination = os.environ.get("FILEX_CWD_FILE")
    if not destination:
        raise RuntimeError("请 source ~/.bashrc 后使用 filex shell function；子进程不能更改父 shell 目录")
    Path(destination).write_bytes(os.fsencode(directory.absolute()) + b"\0")
