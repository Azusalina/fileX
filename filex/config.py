from dataclasses import dataclass, field
from pathlib import Path
import os
import re
import tomllib


_THEME = tomllib.loads((Path(__file__).resolve().parents[1]/"themes/terminal-16.conf").read_text())

DEFAULT_KEYS = {
    "quit": "Q", "exit_here": "x", "dashboard": "q", "dashboard_narrow": "[", "dashboard_widen": "]", "link_info": "I", "expand": "l", "collapse": "h", "down": "j", "up": "k",
    "expand_all": "E", "collapse_all": "C", "open": "o", "open_with": "w",
    "preview": "p", "hidden": ".", "refresh": "Ctrl+r", "search": "Ctrl+f",
    "next_match": "n", "previous_match": "N", "goto": "g", "parent": "Left", "focus_root": "f",
    "new_file": "a", "new_folder": "A", "rename": "r", "copy": "c", "copy_path": "Ctrl+Alt+c",
    "move": "m", "trash": "D", "help": "?", "config": ",",
    "toggle_dual": "v", "switch_pane": "Tab",
}
DEFAULT_OPENERS = {
    "system": {"label": "系统默认", "command": ["xdg-open", "{path}"], "terminal": False},
    "vscodium": {"label": "VSCodium", "command": ["codium", "--", "{path}"], "terminal": False},
    "vim": {"label": "Vim", "command": ["vim", "--", "{path}"], "terminal": True},
    "nvim": {"label": "Neovim", "command": ["nvim", "--", "{path}"], "terminal": True},
    "nano": {"label": "Nano", "command": ["nano", "--", "{path}"], "terminal": True},
    "firefox": {"label": "Firefox", "command": ["firefox", "{path}"], "terminal": False},
    "python": {"label": "Python 执行", "command": ["python", "{path}"], "terminal": True, "pause_on_exit": True},
    "rscript": {"label": "Rscript 执行", "command": ["Rscript", "{path}"], "terminal": True, "pause_on_exit": True},
    "bash": {"label": "Bash 执行", "command": ["bash", "--", "{path}"], "terminal": True, "pause_on_exit": True},
    "markdown": {"label": "Toolkit Markdown / Firefox", "command": ["node", "{filex}/scripts/open_markdown.mjs", "--toolkit", "{toolkit}", "--browser", "firefox", "--", "{path}"], "terminal": False, "copy_path": True},
}


def key_code(key):
    """Translate editable Ctrl+letter bindings to the character sent by terminals."""
    if isinstance(key, str) and key.lower() == "tab":
        return "\t"
    if isinstance(key, str) and re.fullmatch(r"(?i)ctrl\+[a-z]", key):
        return chr(ord(key[-1].lower()) - ord("a") + 1)
    if isinstance(key, str) and re.fullmatch(r"(?i)ctrl\+(alt\+|shift\+|alt\+shift\+)[a-z]", key):
        return key.lower()
    if isinstance(key, str) and key.lower() == "ctrl+f5":
        return "ctrl+f5"
    if isinstance(key, str) and key.lower() == "left":
        return "left"
    return key


def key_map(config):
    return {**{key_code(key): action for action, key in config.keys.items()}, "R": "rename", "ctrl+alt+c": "copy_path", "\x0e": "new_file", "ctrl+shift+n": "new_folder", "ctrl+f5": "refresh"}


def navigation_keys():
    return {"up": "up", "down": "down", "left": "parent", "right": "expand"}


def navigation_hint():
    return "↑↓ 同层循环 ← 父级 → 展开"


def config_path():
    directory = Path(os.environ.get("XDG_CONFIG_HOME", str(Path.home() / ".config"))) / "fileX"
    path, legacy = directory / "filex.conf", directory / "fileX.conf"
    return legacy if not path.exists() and legacy.exists() else path


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
    highlight_path: bool = True
    dual_pane: bool = False
    open_in_right_pane: bool = True
    auto_english: bool = True
    konsole_shortcuts: bool = True
    tmux_extended_keys: bool = True
    mode: str = "auto"
    color_mode: str = "consistent"
    charset: str = "auto"
    cli_opener: str = "auto"
    scan_limit: int = 10000
    max_results: int = 32
    preview_bytes: int = 65536
    preview_lines: int = 5
    dashboard_width: int = 36
    search_max_results: int = 500
    search_scan_limit: int = 100000
    max_expand_nodes: int = 5000
    background: str = "#111820"
    foreground: str = "#d9e3ee"
    folder_color: str = _THEME["colors"]["folder_color"]
    folder_colors: dict = field(default_factory=lambda: _THEME["folder_colors"].copy())
    file_color: str = _THEME["colors"]["file_color"]
    selected_color: str = "#304962"
    mark_color: str = "#FF55FF"
    line_color: str = _THEME["colors"]["line_color"]
    link_color: str = _THEME["colors"]["link_color"]
    path_color: str = _THEME["colors"]["path_color"]
    order: dict = field(default_factory=dict)
    directory_order: dict = field(default_factory=dict)
    git: dict = field(default_factory=dict)
    keys: dict = field(default_factory=lambda: DEFAULT_KEYS.copy())
    openers: dict = field(default_factory=lambda: {k: {**v, "command": v["command"].copy()} for k, v in DEFAULT_OPENERS.items()})
    open_defaults: dict = field(default_factory=lambda: { ".py": "vscodium", ".html": "firefox", ".htm": "firefox", ".md": "markdown", ".markdown": "markdown", "*": "system"})
    extension_colors: dict = field(default_factory=lambda: _THEME["extension_colors"].copy())


def dashboard_columns(config, terminal_width):
    return min(config.dashboard_width, terminal_width - 38) if terminal_width >= 70 else 0


def extension_rule(path, rules, directory=False):
    if directory:
        return rules.get("<directory>", rules.get("*"))
    name = Path(path).name.lower()
    matches = [key for key in rules if key.startswith(".") and name.endswith(key)]
    if matches:
        return rules[max(matches, key=len)]
    return rules.get("", rules.get("*")) if not Path(name).suffix else rules.get("*")


def display_color(node, config):
    return config.folder_colors.get(node.path.name, config.folder_color) if node.is_dir else extension_rule(node.path, config.extension_colors) or config.file_color


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
    data.pop("gui", None)  # Retired configuration.
    for section in ("order", "directory_order", "git"):
        value = data.pop(section, {})
        if not isinstance(value, dict):
            raise ValueError(f"{section}: 应为表")
        if section != "git":
            rules = value.values() if section == "directory_order" else [value]
            for rule in rules:
                if not isinstance(rule, dict) or rule.keys() - {"top", "bottom"} or any(not isinstance(names,list) or not all(isinstance(n,str) and n and '/' not in n and '\\' not in n for n in names) for names in rule.values()):
                    raise ValueError(f"{section}: 应为 top / bottom 名称列表")
        setattr(conf, section, value)
    allowed = {"browser", "layout", "preview", "colors", "keys", "search", "dashboard", "openers", "open_defaults", "extension_colors", "folder_colors", "tmux", "input_method", "links", "terminal"}
    unknown = data.keys() - allowed
    if unknown:
        raise ValueError(f"未知配置区段: {', '.join(sorted(unknown))}")
    sections = {
        "browser": {"show_hidden", "sort", "directories_first", "max_entries", "max_expand_nodes", "max_loaded_nodes"},
        "layout": {"column_gap", "max_label_width", "row_spacing", "highlight_path", "dual_pane"},
        "tmux": {"open_in_right_pane"},
        "input_method": {"auto_english"},
        "links": {"scan_limit", "max_results"},
        "terminal": {"konsole_shortcuts", "tmux_extended_keys", "mode", "color_mode", "charset", "cli_opener"},
        "preview": {"preview_bytes", "preview_lines"}, "dashboard": {"dashboard_width"},
        "search": {"search_max_results", "search_scan_limit"},
        "colors": {"background", "foreground", "folder_color", "file_color", "selected_color", "line_color", "link_color", "path_color", "mark_color"},
    }
    # Drop retired options without changing the user's other settings.
    if isinstance(data.get("layout"), dict):
        data["layout"].pop("orientation", None)
    if isinstance(data.get("keys"), dict):
        data["keys"].pop("toggle_tree", None)
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
                value = value.strip()
                if not re.fullmatch(color_pattern, value):
                    raise ValueError(f"{key}: 应为 #RRGGBB")
            setattr(conf, key, value)
    for key, choices in {"mode": {"auto", "tty"}, "color_mode": {"consistent", "auto", "16", "256", "truecolor", "none"}, "charset": {"auto", "ascii", "unicode"}}.items():
        if getattr(conf, key) not in choices:
            raise ValueError(f"{key}: invalid terminal option")
    if conf.sort not in {"name", "size", "modified"}:
        raise ValueError("sort 应为 name、size 或 modified")
    for key, low, high in [("column_gap", 3, 20), ("max_label_width", 12, 120),
                            ("max_entries", 1, 100000), ("max_loaded_nodes", 100, 100000), ("row_spacing", 1, 4),
                            ("preview_bytes", 1024, 1048576), ("preview_lines", 0, 1000),
                            ("dashboard_width", 20, 240), ("search_max_results", 1, 10000),
                            ("search_scan_limit", 1, 10000000), ("max_expand_nodes", 1, 100000), ("scan_limit", 1, 1000000), ("max_results", 1, 1000)]:
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
    if keys.get("refresh") == "r":
        keys["refresh"] = "Ctrl+r"
    if keys.get("rename") == "R":
        keys["rename"] = "r"
    reserved = {"\n", "\r", " ", "\x1b", "\x03", "\x15", "\x13", "\x11", "\x1a", "H", "L", ":"}
    for action, key in keys.items():
        code = key_code(key)
        ctrl = isinstance(key, str) and bool(re.fullmatch(r"(?i)ctrl\+(alt\+)?(shift\+)?[a-z]", key))
        if not isinstance(key, str) or not (ctrl or len(key) == 1 and key.isprintable() or action == "parent" and code == "left" or action == "refresh" and code == "ctrl+f5" or action == "switch_pane" and code == "\t") or code in reserved or code == "\t" and action != "switch_pane":
            raise ValueError(f"keys.{action}: 应为一个非保留字符、Ctrl+字母或 parent=Left")
        if code in {"\x06", "/"} and action != "search":
            raise ValueError("Ctrl+F 和 / 保留给搜索")
        if code == "\x0c" and action != "goto":
            raise ValueError("Ctrl+L 保留给路径输入")
        if code == "R" and action != "rename" or code == "ctrl+alt+c" and action != "copy_path":
            raise ValueError("Shift+R / Ctrl+Alt+C 保留给重命名 / 复制路径")
        if code == "\x0e" and action != "new_file" or code == "ctrl+shift+n" and action != "new_folder":
            raise ValueError("Ctrl+N / Ctrl+Shift+N 保留给新建文件 / 目录")
        conf.keys[action] = key
    for action, alternatives in [("exit_here", ["z", "b", "t", "s", "Ctrl+Alt+x"]), ("link_info", ["i", "B", "T", "Ctrl+Alt+i"]), ("dashboard_narrow", ["{", "-", "Ctrl+Alt+b"]), ("dashboard_widen", ["}", "+", "Ctrl+Alt+w"]), ("toggle_dual", ["V", "Ctrl+Alt+v"])]:
        if action not in keys:
            occupied = {key_code(value) for name, value in conf.keys.items() if name != action}
            if key_code(conf.keys[action]) in occupied:
                conf.keys[action] = next((key for key in alternatives if key_code(key) not in occupied), conf.keys[action])
    if len({key_code(k) for k in conf.keys.values()}) != len(conf.keys):
        raise ValueError("快捷键重复，请为冲突动作同时指定不同字符")
    openers = data.get("openers", {})
    if not isinstance(openers, dict):
        raise ValueError("openers: 应为程序定义表")
    for name, opener in openers.items():
        if not isinstance(opener, dict) or opener.keys() - {"label", "command", "terminal", "copy_path", "pause_on_exit", "extensions"}:
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
        if type(value.get("copy_path", False)) is not bool:
            raise ValueError(f"openers.{name}.copy_path: 应为布尔值")
        if type(value.get("pause_on_exit", False)) is not bool:
            raise ValueError(f"openers.{name}.pause_on_exit: 应为布尔值")
        extensions = value.get("extensions", [])
        if not isinstance(extensions, list) or not all(isinstance(e, str) and e.startswith('.') and len(e) > 1 and not any(c in e for c in ('/', '\\', '\0')) for e in extensions):
            raise ValueError(f"openers.{name}.extensions: 应为扩展名列表")
        conf.openers[name] = value
    folder_colors = data.get("folder_colors", {})
    if not isinstance(folder_colors, dict):
        raise ValueError("folder_colors: 应为目录名对应颜色的表")
    for name, value in folder_colors.items():
        if not name or "/" in name or name in {".", ".."}:
            raise ValueError("folder_colors: 应为目录名，不含路径")
        if isinstance(value, str):
            value = value.strip()
        if not isinstance(value, str) or not re.fullmatch(color_pattern, value):
            raise ValueError(f"folder_colors.{name}: 应为 #RRGGBB")
        conf.folder_colors[name] = value
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
            if section == "extension_colors":
                value = value.strip()
                if not re.fullmatch(color_pattern, value):
                    raise ValueError(f"extension_colors.{extension}: 应为 #RRGGBB")
            getattr(conf, section)[extension.lower()] = value
    if conf.cli_opener != "auto" and not conf.openers.get(conf.cli_opener, {}).get("terminal", False):
        raise ValueError("terminal.cli_opener 必须为 auto 或 terminal=true 的 opener")
    return conf
