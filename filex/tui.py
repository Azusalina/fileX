import curses
import os
import subprocess
from pathlib import Path
from .actions import ARG_LABELS, perform
from .core import cell_width, preview, safe_text, shorten
from .config import config_path, load_config, key_map, display_color, dashboard_columns, navigation_keys, navigation_hint
from .details import metadata, metadata_text, dashboard_text
from .opening import default_opener, opener_choices, prepare_open, launch_open
from .layout import Layout


class Terminal:
    def __init__(self, screen, browser, config_file=None):
        self.screen, self.browser = screen, browser
        self.config_file = config_file
        self.left = self.top = 0
        self.message = ""
        self.query = ""
        self.dashboard = False
        self.metadata_path = None
        self.metadata_value = ""
        self.color_pairs = {}
        self.layout = Layout(browser)
        self.focus = True
        curses.curs_set(0)
        screen.keypad(True)
        if curses.has_colors():
            curses.start_color()
            curses.use_default_colors()

    def color(self, value):
        if not curses.has_colors():
            return 0
        if value in self.color_pairs:
            return self.color_pairs[value]
        rgb = tuple(int(value[i:i + 2], 16) for i in (1, 3, 5))
        ansi = [(0, 0, 0), (128, 0, 0), (0, 128, 0), (128, 128, 0), (0, 0, 128), (128, 0, 128), (0, 128, 128), (192, 192, 192),
                (128, 128, 128), (255, 0, 0), (0, 255, 0), (255, 255, 0), (0, 0, 255), (255, 0, 255), (0, 255, 255), (255, 255, 255)]
        levels = [0, 95, 135, 175, 215, 255]
        palette = ansi + [(r, g, b) for r in levels for g in levels for b in levels] + [(v, v, v) for v in range(8, 239, 10)]
        index = min(range(min(curses.COLORS, 256)), key=lambda i: sum((a - b) ** 2 for a, b in zip(rgb, palette[i])))
        pair = len(self.color_pairs) + 1
        if pair >= curses.COLOR_PAIRS:
            return 0
        curses.init_pair(pair, index, -1)
        result = curses.color_pair(pair)
        self.color_pairs[value] = result
        return result

    def put(self, y, x, text, attr=0, limit=None):
        height, width = self.screen.getmaxyx()
        if y < 0 or y >= height or x >= width:
            return
        text = safe_text(text)
        if x < 0:
            consumed = 0
            while text and consumed < -x:
                consumed += cell_width(text[0])
                text = text[1:]
            x += consumed
        available = max(0, width - x - 1)
        if limit is not None:
            available = min(available, limit)
        text = shorten(text, available) if available else ""
        try:
            self.screen.addstr(y, x, text, attr)
        except curses.error:
            pass

    def draw(self):
        self.screen.erase()
        height, width = self.screen.getmaxyx()
        if height < 8 or width < 36:
            self.put(0, 0, f"请放大终端到至少 36 × 8；{self.browser.config.keys['quit']} 退出")
            self.screen.refresh()
            return
        body_height = height - 5
        panel_width = dashboard_columns(self.browser.config, width) if self.dashboard and width >= 70 else 0
        tree_width = width - panel_width - (2 if panel_width else 0)
        self.layout = Layout(self.browser)
        selected = self.layout.by_node[self.browser.selected]
        if self.focus:
            if selected.y < self.top:
                self.top = selected.y
            if selected.y >= self.top + body_height:
                self.top = selected.y - body_height + 1
            if selected.x < self.left:
                self.left = selected.x
            if selected.x + cell_width(selected.label) > self.left + tree_width - 2:
                self.left = max(0, selected.x + cell_width(selected.label) - tree_width + 2)
            self.focus = False
        self.left = max(0, min(self.left, max(0, self.layout.width - tree_width + 2)))
        self.top = max(0, min(self.top, max(0, self.layout.height - body_height)))
        self.put(0, 1, "filex", curses.A_BOLD)
        self.put(1, 1, str(self.browser.root.path), self.color(display_color(self.browser.root, self.browser.config)))
        for (x, y), char in self.layout.line_cells(self.left, self.top, tree_width - 2, body_height).items():
            self.put(y - self.top + 3, x - self.left + 1, char, self.color(self.browser.config.line_color))
        for item in self.layout.placements:
            if not self.top <= item.y < self.top + body_height:
                continue
            attr = self.color(display_color(item.node, self.browser.config))
            if self.query and self.query.casefold() in item.node.path.name.casefold():
                attr |= curses.A_BOLD
            if item.node is self.browser.selected:
                attr |= curses.A_REVERSE
            x = item.x - self.left + 1
            self.put(item.y - self.top + 3, x, item.label, attr, max(0, tree_width - max(0, x) - 1))
            if item.link_start is not None:
                start = item.link_start
                link_x = x + cell_width(item.label[:start])
                link_attr = self.color(self.browser.config.link_color) | curses.A_BOLD
                self.put(item.y - self.top + 3, link_x, item.label[start:], link_attr, max(0, tree_width - max(0, link_x) - 1))
        if panel_width:
            for y in range(3, height - 2):
                self.put(y, tree_width, "│", self.color(self.browser.config.line_color))
            path = self.browser.selected.path
            if path != self.metadata_path:
                self.metadata_value = dashboard_text(path, self.browser.root.path, self.browser.config)
                self.metadata_path = path
            self.put(3, tree_width + 2, "Dashboard · q 收起", curses.A_BOLD, panel_width - 2)
            for offset, line in enumerate(self.metadata_value.splitlines()[:max(0, body_height - 1)]):
                self.put(4 + offset, tree_width + 2, line, limit=panel_width - 2)
        node = self.browser.selected
        detail = node.error or (f"还有 {node.omitted} 项未显示；调高 max_entries 后刷新" if node.omitted else str(node.path))
        self.put(height - 2, 1, self.message or detail)
        keys = self.browser.config.keys
        self.put(height - 1, 1, f"{navigation_hint()}  {keys['collapse']} 收起  {keys['focus_root']} 设根  Enter 展开/文件打开  {keys['open_with']} 菜单  {keys['dashboard']} 信息  Ctrl+F 搜索  Delete 回收站  {keys['quit']} 退出", curses.A_DIM)
        self.screen.refresh()

    def prompt(self, label, initial=""):
        text = initial
        while True:
            self.draw()
            height, width = self.screen.getmaxyx()
            self.screen.move(height - 2, 0)
            self.screen.clrtoeol()
            prompt = f"{label}: "
            # Keep the editing tail visible for long paths.
            shown = prompt + text
            while cell_width(shown) >= width - 2:
                shown = shown[1:]
            self.put(height - 2, 0, shown)
            curses.curs_set(1)
            self.screen.move(height - 2, min(width - 2, cell_width(shown)))
            self.screen.refresh()
            key = self.screen.get_wch()
            if key in ("\n", "\r", curses.KEY_ENTER):
                curses.curs_set(0)
                return text
            if key == "\x1b":
                curses.curs_set(0)
                return None
            if key in (curses.KEY_BACKSPACE, "\x7f", "\b"):
                text = text[:-1]
            elif key == "\x15":
                text = ""
            elif isinstance(key, str) and key.isprintable():
                text += key

    def confirm(self, title):
        while True:
            self.draw()
            height, width = self.screen.getmaxyx()
            self.screen.move(height - 2, 0); self.screen.clrtoeol()
            self.put(height - 2, 0, title)
            self.put(height - 1, 0, "再次按 Enter 确认 · Esc 取消")
            self.screen.refresh()
            key = self.screen.get_wch()
            if key in ("\n", "\r", curses.KEY_ENTER): return True
            if key in ("\x1b", "\x03"): return False

    def pager(self, title, content):
        rows = content.splitlines()
        top, left = 0, 0
        while True:
            self.screen.erase()
            height, width = self.screen.getmaxyx()
            self.put(0, 1, title, curses.A_BOLD)
            for i, line in enumerate(rows[top:top + max(1, height - 3)]):
                self.put(i + 1, 1 - left, line)
            self.put(height - 1, 1, "↑↓ 滚动  ←→ 横移  Esc / q / Enter 返回", curses.A_DIM)
            self.screen.refresh()
            key = self.screen.get_wch()
            if key in ("\x1b", "q", "\n", "\r"):
                return
            if key in (curses.KEY_DOWN, "j"):
                top = min(max(0, len(rows) - max(1, height - 3)), top + 1)
            elif key in (curses.KEY_UP, "k"):
                top = max(0, top - 1)
            elif key == curses.KEY_NPAGE:
                top = min(max(0, len(rows) - max(1, height - 3)), top + max(1, height - 3))
            elif key == curses.KEY_PPAGE:
                top = max(0, top - max(1, height - 3))
            elif key == curses.KEY_RIGHT:
                left += 8
            elif key == curses.KEY_LEFT:
                left = max(0, left - 8)

    def match(self, direction=1):
        if self.query:
            self.message = "正在搜索当前根目录…"
            self.draw()
            self.message = self.browser.find_next(self.query, direction)

    def choose(self, title, options):
        selected = 0
        while True:
            self.screen.erase()
            height, width = self.screen.getmaxyx()
            self.put(0, 1, title, curses.A_BOLD)
            first = max(0, selected - max(1, height - 4) + 1)
            for i in range(first, min(len(options), first + max(1, height - 4))):
                self.put(i - first + 2, 2, f"{i + 1}. {options[i][1]}", curses.A_REVERSE if i == selected else 0)
            self.put(height - 1, 1, "↑↓ 选择 · Enter 确认 · Esc 取消", curses.A_DIM)
            self.screen.refresh()
            key = self.screen.get_wch()
            if key == "\x1b":
                return None
            if key in ("\n", "\r", curses.KEY_ENTER):
                return options[selected]
            if key in (curses.KEY_UP, "k"):
                selected = (selected - 1) % len(options)
            elif key in (curses.KEY_DOWN, "j"):
                selected = (selected + 1) % len(options)

    def open_method(self, method):
        path = self.browser.selected.path
        command, terminal = prepare_open(path, self.browser.config, method)
        if terminal:
            curses.def_prog_mode()
            curses.endwin()
            try:
                result = subprocess.run(command, check=False, env={k:v for k,v in os.environ.items() if k != "FILEX_CWD_FILE"}, cwd=path if path.is_dir() else path.parent)
                if self.browser.config.openers[method].get("pause_on_exit", False):
                    input(f"程序结束 ({result.returncode}) · 按 Enter 返回 filex")
            finally:
                curses.reset_prog_mode()
                curses.curs_set(0)
                self.screen.refresh()
        else:
            launch_open(path, self.browser.config, method)
        self.metadata_path = None
        self.message = f"已用 {self.browser.config.openers[method]['label']} 打开"

    def choose_open(self):
        node = self.browser.selected
        choices = opener_choices(node, self.browser.config)
        if node.is_dir:
            choices.insert(0, (None, "fileX 展开 / 收起当前目录"))
        selected = self.choose(f"打开方式: {node.path}", choices)
        if selected is not None:
            method, _ = selected
            if method is None:
                if node.expanded:
                    node.expanded = False
                else:
                    self.browser.expand(node)
            else:
                self.open_method(method)

    def run(self):
        while True:
            self.draw()
            key = self.screen.get_wch()
            if key == curses.KEY_RESIZE:
                self.focus = True
                continue
            if key == "\x1b":
                self.screen.timeout(35)
                try:
                    following = self.screen.get_wch()
                    if isinstance(following, str) and len(following) == 1 and "\x01" <= following <= "\x1a":
                        key = "ctrl+alt+" + chr(ord(following) + ord("a") - 1)
                    else:
                        curses.unget_wch(following)
                except curses.error:
                    pass
                finally:
                    self.screen.timeout(-1)
            keys = key_map(self.browser.config)
            action = keys.get(key, "") if isinstance(key, str) else ""
            if key in ("\x06", "/"):
                action = "search"
            elif key == "\x0c":
                action = "goto"
            elif key in (curses.KEY_F5, curses.KEY_F29):
                action = "refresh"
            elif key == curses.KEY_DC:
                action = "trash"
            elif key in (curses.KEY_UP, curses.KEY_DOWN, curses.KEY_LEFT, curses.KEY_RIGHT):
                names = {curses.KEY_UP: "up", curses.KEY_DOWN: "down", curses.KEY_LEFT: "left", curses.KEY_RIGHT: "right"}
                action = navigation_keys()[names[key]]
            if key == "\x1b":
                self.query = self.message = ""
                continue
            if key == "H":
                self.left = max(0, self.left - 12)
                continue
            if key == "L":
                self.left += 12
                continue
            self.message = ""
            self.focus = True
            try:
                if action == "quit":
                    return
                if action == "exit_here":
                    from .shell import exit_directory
                    exit_directory(self.browser.selected.path)
                    return
                if action == "dashboard":
                    self.dashboard = not self.dashboard
                    self.metadata_path = None
                    if self.dashboard and self.screen.getmaxyx()[1] < 70:
                        self.message = "右侧 Dashboard 需要至少 70 列的终端"
                elif action in {"dashboard_narrow", "dashboard_widen"}:
                    width = self.screen.getmaxyx()[1]
                    if width < 70:
                        self.message = "Dashboard 需要至少 70 列"
                    else:
                        current = dashboard_columns(self.browser.config, width)
                        self.browser.config.dashboard_width = min(min(width - 38, 240), max(20, current + (-4 if action == "dashboard_narrow" else 4)))
                        self.dashboard = True
                        self.message = f"信息面板宽度: {self.browser.config.dashboard_width} 列"
                elif action == "down":
                    self.browser.step(1)
                elif action == "up":
                    self.browser.step(-1)
                elif action == "expand" or key == " ":
                    self.browser.right()
                elif action == "collapse":
                    self.browser.left()
                elif key == curses.KEY_NPAGE:
                    self.browser.step(max(1, self.screen.getmaxyx()[0] - 5))
                elif key == curses.KEY_PPAGE:
                    self.browser.step(-max(1, self.screen.getmaxyx()[0] - 5))
                elif key == curses.KEY_HOME:
                    self.browser.selected = self.browser.root
                elif key == curses.KEY_END:
                    self.browser.selected = self.browser.visible()[-1]
                elif action == "expand_all":
                    self.message = "正在展开选中目录的全部分支…"
                    self.draw()
                    self.message = self.browser.expand_all()
                elif action == "collapse_all":
                    self.message = self.browser.collapse_all()
                elif action == "open":
                    self.open_method(default_opener(self.browser.selected, self.browser.config))
                elif action == "toolkit":
                    path = self.browser.selected.path
                    if not path.is_file(): raise ValueError("Toolkit：请先选中本地文件")
                    launcher = os.environ.get("FILEX_TOOLKIT_DESKTOP", str(Path(__file__).resolve().parents[2] / "integrate-toolkit-arch/toolkit-desktop"))
                    subprocess.Popen([launcher, "--list", "--", str(path.absolute())], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
                    self.message = "Toolkit：选择工具，再选择结果路径与文件名"
                elif key in ("\n", "\r", curses.KEY_ENTER) and self.browser.selected.is_dir:
                    if not self.browser.selected.expanded:
                        self.browser.expand(self.browser.selected)
                elif action == "open_with" or key in ("\n", "\r", curses.KEY_ENTER):
                    self.choose_open()
                elif action == "link_info":
                    self.pager("文件 / 链接信息", dashboard_text(self.browser.selected.path, self.browser.root.path, self.browser.config))
                elif action == "preview":
                    self.pager(str(self.browser.selected.path), preview(self.browser.selected.path, self.browser.config.preview_bytes))
                elif action == "parent":
                    self.message = self.browser.back()
                    self.left = self.top = 0
                elif action == "focus_root":
                    self.message = self.browser.focus_root()
                    self.query = ""
                    self.metadata_path = None
                    self.left = self.top = 0
                elif action == "goto":
                    value = self.prompt("进入目录（Esc 取消）", str(self.browser.root.path))
                    if value:
                        self.browser.set_root(value)
                        self.left = self.top = 0
                elif action == "hidden":
                    self.browser.toggle_hidden()
                elif action == "copy_path":
                    from .clipboard import copy_path
                    copy_path(self.browser.selected.path)
                    self.message = "已复制绝对路径"
                elif action == "refresh":
                    self.browser.refresh()
                    self.metadata_path = None
                elif action == "search":
                    query = self.prompt("搜索当前根目录（含未展开目录）；留空清除", self.query)
                    if query is not None:
                        self.query = query
                        if query:
                            self.match()
                elif action in {"next_match", "previous_match"}:
                    self.match(1 if action == "next_match" else -1)
                elif action in ARG_LABELS:
                    value = self.prompt(ARG_LABELS[action], self.browser.selected.path.name if action == "rename" else "")
                    if value:
                        result = perform(self.browser, action, value)
                        self.message = f"完成: {result}"
                elif action == "trash":
                    if self.confirm(f"送入回收站 {self.browser.selected.path}？"):
                        perform(self.browser, "trash")
                        self.message = "已送入回收站；可在 Dolphin 中恢复"
                elif action == "config":
                    path = Path(self.config_file) if self.config_file else config_path()
                    self.browser.config = load_config(path)
                    self.browser.refresh()
                    self.metadata_path = None
                    self.message = f"配置已重新读取: {path}"
                elif action == "help":
                    entries = {"quit": "退出", "exit_here": "退出并将 terminal 切到选中目录", "dashboard": "右侧创建 / 修改时间 / 大小 / 链接", "link_info": "完整文件 / 链接信息", "expand": "展开 / 进入子节点", "collapse": "只收起选中目录", "expand_all": "展开选中目录全部分支", "collapse_all": "收起选中目录全部分支", "down": "下一个同级节点", "up": "上一个同级节点", "open": "按扩展名默认程序打开", "open_with": "选择打开程序（文件也可 Enter）", "preview": "文本预览", "hidden": "隐藏文件开关", "refresh": "刷新", "search": "递归搜索文件 / 目录名", "next_match": "下一个匹配", "previous_match": "上一个匹配", "goto": "更换根目录", "parent": "返回父级 / 引入上级根目录", "focus_root": "当前目录设为根并刷新", "new_file": "新建文件", "new_folder": "新建目录", "rename": "重命名", "copy": "复制", "copy_path": "复制绝对路径", "move": "移动", "trash": "送入回收站（确认）", "help": "帮助", "config": "重新读取配置"}
                    self.pager("fileX 快捷键", "\n".join(f"{self.browser.config.keys[a]}    {label}" for a, label in entries.items()) + "\n\nCtrl+N / Ctrl+Shift+N    新建文件 / 目录\nCtrl+F / /    搜索\nEnter    展开目录（光标保留）/ 文件选择打开程序\nw    打开方式菜单\nH / L    横向平移\nPgUp / PgDn    翻页\nHome / End    首末节点\nEsc    取消输入 / 清除搜索\nCtrl+U    清空输入\n\n复制 / 移动填写完整目标路径，不会覆盖已有目标。\n扩展名配色用于 TUI，终端会近似为可用颜色。")
            except (OSError, ValueError, RuntimeError) as exc:
                self.message = f"操作失败: {safe_text(exc)}"
            except Exception as exc:
                self.message = f"操作失败: {type(exc).__name__}: {safe_text(exc)}"


def run(browser, config_file=None):
    os.environ.setdefault("ESCDELAY", "25")
    from .input_method import english
    with english(browser.config.auto_english):
        curses.wrapper(lambda screen: Terminal(screen, browser, config_file).run())
