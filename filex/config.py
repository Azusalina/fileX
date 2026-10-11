from dataclasses import dataclass, field
from pathlib import Path
import os
import re
import tomllib


DEFAULT_KEYS = {
    "quit": "Q", "dashboard": "q", "expand": "l", "collapse": "h", "down": "j", "up": "k",
    "expand_all": "E", "collapse_all": "C", "open": "o", "open_with": "w",
    "preview": "p", "hidden": ".", "refresh": "r", "search": "Ctrl+f",
    "next_match": "n", "previous_match": "N", "goto": "g", "parent": "Left", "toggle_tree": "v", "focus_root": "f",
    "new_file": "a", "new_folder": "A", "rename": "R", "copy": "c",
    "move": "m", "trash": "D", "help": "?", "config": ",",
}
DEFAULT_OPENERS = {
    "system": {"label": "系统默认", "command": ["xdg-open", "{path}"], "terminal": False},
    "vscodium": {"label": "VSCodium", "command": ["codium", "--", "{path}"], "terminal": False},
    "vim": {"label": "Vim", "command": ["vim", "--", "{path}"], "terminal": True},
    "nvim": {"label": "Neovim", "command": ["nvim", "--", "{path}"], "terminal": True},
    "nano": {"label": "Nano", "command": ["nano", "--", "{path}"], "terminal": True},
    "firefox": {"label": "Firefox", "command": ["firefox", "{path}"], "terminal": False},
}


def key_code(key):
    """Translate editable Ctrl+letter bindings to the character sent by terminals."""
    if isinstance(key, str) and re.fullmatch(r"(?i)ctrl\+[a-z]", key):
        return chr(ord(key[-1].lower()) - ord("a") + 1)
    if isinstance(key, str) and key.lower() == "left":
        return "left"
    return key


def key_map(config):
    return {key_code(key): action for action, key in config.keys.items()}


def navigation_keys(orientation):
    if orientation == "vertical":
        return {"up": "parent", "down": "expand", "left": "up", "right": "down"}
    return {"up": "up", "down": "down", "left": "parent", "right": "expand"}


def navigation_hint(orientation):
    return "←→ 同级 ↑ 父级 ↓ 展开" if orientation == "vertical" else "↑↓ 同级 ← 父级 → 展开"


def config_path():
    return Path(os.environ.get("XDG_CONFIG_HOME", str(Path.home() / ".config"))) / "fileX/fileX.conf"


@dataclass
class Config:
    show_hidden: bool = False
    sort: str = "name"
    directories_first: bool = True
    column_gap: int = 6
    max_label_width: int = 32
    max_entries: int = 1000
    max_loaded_nodes: int = 6000
    row_spacing: int = 1
    orientation: str = "horizontal"
    open_in_right_pane: bool = True
    preview_bytes: int = 65536
    dashboard_width: int = 36
    search_max_results: int = 500
    search_scan_limit: int = 100000
    max_expand_nodes: int = 5000
    gui_font_size: int = 11
    background: str = "#111820"
    foreground: str = "#d9e3ee"
    folder_color: str = "#7dcfff"
    file_color: str = "#c8d3df"
    selected_color: str = "#304962"
    line_color: str = "#526779"
    keys: dict = field(default_factory=lambda: DEFAULT_KEYS.copy())
    openers: dict = field(default_factory=lambda: {k: {**v, "command": v["command"].copy()} for k, v in DEFAULT_OPENERS.items()})
    open_defaults: dict = field(default_factory=lambda: {".py": "vscodium", ".html": "firefox", ".htm": "firefox", "*": "system"})
    extension_colors: dict = field(default_factory=lambda: {".py": "#e5c07b", ".html": "#e06c75", ".png": "#c792ea", ".jpg": "#c792ea", ".md": "#98c379"})


def extension_rule(path, rules, directory=False):
    if directory:
        return rules.get("<directory>", rules.get("*"))
    name = Path(path).name.lower()
    matches = [key for key in rules if key.startswith(".") and name.endswith(key)]
    if matches:
        return rules[max(matches, key=len)]
    return rules.get("", rules.get("*")) if not Path(name).suffix else rules.get("*")


def display_color(node, config):
    return config.folder_color if node.is_dir else extension_rule(node.path, config.extension_colors) or config.file_color


def load_config(path=None):
    path = Path(path).expanduser() if path else config_path()
    conf = Config()
    if not path.exists():
        return conf
    try:
        with path.open("rb") as stream:
            data = tomllib.load(stream)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise ValueError(f"无法读取配置 {path}: {exc}") from exc
    allowed = {"browser", "layout", "preview", "gui", "colors", "keys", "search", "dashboard", "openers", "open_defaults", "extension_colors", "tmux"}
    unknown = data.keys() - allowed
    if unknown:
        raise ValueError(f"未知配置区段: {', '.join(sorted(unknown))}")
    sections = {
        "browser": {"show_hidden", "sort", "directories_first", "max_entries", "max_expand_nodes", "max_loaded_nodes"},
        "layout": {"column_gap", "max_label_width", "row_spacing", "orientation"},
        "tmux": {"open_in_right_pane"},
        "preview": {"preview_bytes"}, "gui": {"gui_font_size"}, "dashboard": {"dashboard_width"},
        "search": {"search_max_results", "search_scan_limit"},
        "colors": {"background", "foreground", "folder_color", "file_color", "selected_color", "line_color"},
    }
    color_pattern = r"#[0-9a-fA-F]{6}"
    for section, names in sections.items():
        values = data.get(section, {})
        if not isinstance(values, dict) or values.keys() - names:
            raise ValueError(f"{section}: 存在未知选项或区段格式错误")
        for key, value in values.items():
            expected = type(getattr(conf, key))
            if type(value) is not expected:
                raise ValueError(f"{key}: 应为 {expected.__name__}")
            if key.endswith("color") or key in {"background", "foreground"}:
                if not re.fullmatch(color_pattern, value):
                    raise ValueError(f"{key}: 应为 #RRGGBB")
            setattr(conf, key, value)
    if conf.sort not in {"name", "size", "modified"}:
        raise ValueError("sort 应为 name、size 或 modified")
    if conf.orientation not in {"horizontal", "vertical"}:
        raise ValueError("orientation 应为 horizontal 或 vertical")
    for key, low, high in [("column_gap", 3, 20), ("max_label_width", 12, 120),
                            ("max_entries", 1, 100000), ("max_loaded_nodes", 100, 100000), ("row_spacing", 1, 4),
                            ("preview_bytes", 1024, 1048576), ("gui_font_size", 8, 32),
                            ("dashboard_width", 28, 80), ("search_max_results", 1, 10000),
                            ("search_scan_limit", 1, 10000000), ("max_expand_nodes", 1, 100000)]:
        if not low <= getattr(conf, key) <= high:
            raise ValueError(f"{key}: 应在 {low} 到 {high} 之间")
    keys = data.get("keys", {})
    if not isinstance(keys, dict) or keys.keys() - DEFAULT_KEYS.keys():
        raise ValueError("keys: 存在未知动作")
    keys = keys.copy()
    # Migrate the old default quit key, leaving custom bindings intact.
    if keys.get("quit") == "q" and "dashboard" not in keys:
        keys["quit"] = "Q"
    if keys.get("parent") == "u":
        keys["parent"] = "Left"
    reserved = {"\n", "\r", "\t", " ", "\x1b", "\x03", "\x15", "\x13", "\x11", "\x1a", "H", "L", ":"}
    for action, key in keys.items():
        code = key_code(key)
        ctrl = isinstance(key, str) and bool(re.fullmatch(r"(?i)ctrl\+[a-z]", key))
        if not isinstance(key, str) or not (ctrl or len(key) == 1 and key.isprintable() or action == "parent" and code == "left") or code in reserved:
            raise ValueError(f"keys.{action}: 应为一个非保留字符、Ctrl+字母或 parent=Left")
        if code in {"\x06", "/"} and action != "search":
            raise ValueError("Ctrl+F 和 / 保留给搜索")
        if code == "\x0c" and action != "goto":
            raise ValueError("Ctrl+L 保留给路径输入")
        conf.keys[action] = key
    if len({key_code(k) for k in conf.keys.values()}) != len(conf.keys):
        raise ValueError("快捷键重复，请为冲突动作同时指定不同字符")
    openers = data.get("openers", {})
    if not isinstance(openers, dict):
        raise ValueError("openers: 应为程序定义表")
    for name, opener in openers.items():
        if not isinstance(opener, dict) or opener.keys() - {"label", "command", "terminal"}:
            raise ValueError(f"openers.{name}: 未知字段")
        base = conf.openers.get(name, {"label": name, "terminal": False})
        value = {**base, **opener}
        command = value.get("command")
        if not isinstance(command, list) or not command or not all(isinstance(arg, str) and arg and "\0" not in arg for arg in command):
            raise ValueError(f"openers.{name}.command: 应为非空字符串参数列表")
        if not any("{path}" in arg for arg in command):
            raise ValueError(f"openers.{name}.command: 必须包含 {{path}}")
        if not isinstance(value["label"], str) or not value["label"] or type(value["terminal"]) is not bool:
            raise ValueError(f"openers.{name}: label 应为字符串，terminal 应为布尔值")
        conf.openers[name] = value
    for section in ("open_defaults", "extension_colors"):
        values = data.get(section, {})
        if not isinstance(values, dict):
            raise ValueError(f"{section}: 应为扩展名对应表")
        for extension, value in values.items():
            if extension not in {"*", "", "<directory>"} and (not extension.startswith(".") or "/" in extension):
                raise ValueError(f"{section}: 扩展名应以 . 开始")
            if not isinstance(value, str):
                raise ValueError(f"{section}.{extension}: 应为字符串")
            if section == "open_defaults" and value not in conf.openers:
                raise ValueError(f"open_defaults.{extension}: 未定义程序 {value}")
            if section == "extension_colors" and not re.fullmatch(color_pattern, value):
                raise ValueError(f"extension_colors.{extension}: 应为 #RRGGBB")
            getattr(conf, section)[extension.lower()] = value
    return conf
