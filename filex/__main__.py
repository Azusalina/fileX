import argparse
from pathlib import Path
import sys
from . import __version__
from .config import config_path, load_config
from .core import Browser


def main():
    parser = argparse.ArgumentParser(description="fileX — 横向树文件浏览器（默认 TUI）")
    parser.add_argument("path", nargs="?", default=".", help="根目录，默认当前目录")
    parser.add_argument("--gui", action="store_true", help="启动 Qt GUI")
    parser.add_argument("--config", type=Path, help="自定义 TOML 配置路径")
    parser.add_argument("--init-config", action="store_true", help="创建配置样本，拒绝覆盖已有文件")
    parser.add_argument("--print-tree", action="store_true", help="输出第一层横向树，无需交互终端")
    parser.add_argument("--version", action="version", version=f"fileX {__version__}")
    args = parser.parse_args()
    try:
        if args.init_config:
            destination = args.config or config_path()
            destination.parent.mkdir(parents=True, exist_ok=True)
            example = Path(__file__).resolve().parent.parent / "fileX.conf"
            with destination.open("xb") as output:
                output.write(example.read_bytes())
            print(f"已创建配置: {destination}")
            return 0
        if args.config is not None and not args.config.exists():
            raise FileNotFoundError(f"配置不存在: {args.config}（可用 --init-config 创建）")
        conf = load_config(args.config)
        browser = Browser(args.path, conf)
        if args.print_tree:
            from .layout import plain_tree
            print(plain_tree(browser))
            return 0
        if args.gui:
            try:
                from .gui import run
            except ImportError as exc:
                raise RuntimeError("GUI 需要 PySide6；Arch: sudo pacman -S pyside6") from exc
        else:
            if not sys.stdin.isatty() or not sys.stdout.isatty():
                raise RuntimeError("TUI 需要交互终端；可改用 --print-tree 或 --gui")
            from .tui import run
        return run(browser, args.config) or 0
    except (OSError, ValueError, RuntimeError) as exc:
        print(f"fileX: {exc}", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        return 130


if __name__ == "__main__":
    raise SystemExit(main())
