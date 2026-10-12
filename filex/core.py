from dataclasses import dataclass, field
from pathlib import Path
import mimetypes
import os
import shutil
import stat
import subprocess
import unicodedata


def safe_text(value):
    return "".join(c if not unicodedata.category(c).startswith("C") else "�" for c in str(value))


def cell_width(text):
    return sum(0 if unicodedata.combining(c) else 2 if unicodedata.east_asian_width(c) in "WF" else 1 for c in text)


def shorten(text, width):
    text = safe_text(text)
    if cell_width(text) <= width:
        return text
    result = ""
    for char in text:
        if cell_width(result + char) > width - 1:
            break
        result += char
    return result + "…"


@dataclass(eq=False)
class Node:
    path: Path
    parent: "Node | None" = None
    is_dir: bool = False
    symlink: bool = False
    links: int = 1
    expanded: bool = False
    children: list = field(default_factory=list)
    error: str = ""
    omitted: int = 0

    @property
    def label(self):
        prefix = "▾ " if self.is_dir and self.expanded else "▸ " if self.is_dir else "· "
        suffix = (" ↗" if self.symlink else "") + (f" H×{self.links}" if not self.is_dir and self.links > 1 else "")
        return prefix + (self.path.name or str(self.path)) + suffix

    def display_label(self, width):
        return self.label_parts(width)[0]

    def label_parts(self, width):
        prefix = "▾ " if self.is_dir and self.expanded else "▸ " if self.is_dir else "· "
        name = safe_text(self.path.name or str(self.path))
        if self.symlink:
            try:
                target = Path(os.readlink(self.path))
                target_name = safe_text(target.name or str(target))
                available = max(0, width - cell_width(prefix) - 6)
                target_width = min(cell_width(target_name), available // 2)
                head = prefix + shorten(name, max(0, available - target_width))
                tail = shorten(target_name, max(0, width - cell_width(head) - 6))
                return head + "------" + tail, len(head)
            except OSError:
                pass
        suffix = (" ↗" if self.symlink else "") + (f" H×{self.links}" if not self.is_dir and self.links > 1 else "")
        name = shorten(name, max(0, width - cell_width(prefix + suffix)))
        return shorten(prefix + name + suffix, width), None



class Browser:
    def __init__(self, root, config):
        self.config = config
        self._nodes = {}
        self._pinned = set()
        self._ancestry_pins = set()
        self.search_results = []
        self.search_signature = None
        self.search_summary = ""
        self.set_root(root)

    def set_root(self, root):
        # Keep child symlink paths intact; resolve only the browsing root.
        path = Path(root).expanduser().absolute().resolve(strict=True)
        if not path.is_dir():
            raise ValueError(f"不是目录: {path}")
        self.root = self._nodes.setdefault(path, Node(path, is_dir=True))
        self.root.parent = None
        self.selected = self.root
        self.root.expanded = True
        self.refresh()

    def ascend(self):
        """Wrap the existing tree in its parent without losing nodes or selection."""
        previous, selected = self.root, self.selected
        path = previous.path.parent
        if path == previous.path:
            return "已在文件系统根目录"
        parent = self._nodes.setdefault(path, Node(path, is_dir=True))
        parent.parent = None
        self._pinned.add(previous.path)
        self._ancestry_pins.add(previous.path)
        self.expand(parent)
        if not parent.expanded:
            raise OSError(parent.error)
        self.root = parent
        self.selected = selected
        self.search_signature = None
        return f"引入上级目录: {path}"

    def expand(self, node):
        if not node.is_dir or node.symlink:
            return
        node.error = ""
        try:
            entries = []
            with os.scandir(node.path) as iterator:
                for entry in iterator:
                    if not self.config.show_hidden and entry.name.startswith(".") and node.path / entry.name not in self._ancestry_pins:
                        continue
                    try:
                        link = entry.is_symlink()
                        directory = entry.is_dir(follow_symlinks=False)
                        info = entry.stat(follow_symlinks=False)
                        entries.append((entry.name, directory, link, info.st_size, info.st_mtime, info.st_nlink))
                    except OSError:
                        entries.append((entry.name, False, entry.is_symlink(), 0, 0, 1))
            def key(entry):
                name, directory, _, size, modified, _ = entry
                value = name.casefold() if self.config.sort == "name" else -size if self.config.sort == "size" else -modified
                return (not directory if self.config.directories_first else False, value, name)
            entries.sort(key=key)
            old = {child.path: child for child in node.children}
            node.children = []
            chosen = entries[:self.config.max_entries]
            chosen += [e for e in entries[self.config.max_entries:] if node.path / e[0] in self._pinned or old.get(node.path / e[0], Node(node.path / e[0])).expanded]
            for name, directory, link, _, _, links in chosen:
                path = node.path / name
                child = self._nodes.setdefault(path, Node(path, node, directory, link))
                child.parent = node
                child.is_dir, child.symlink, child.links = directory, link, links
                if not directory or link:
                    child.expanded = False
                    child.children = []
                node.children.append(child)
            node.omitted = max(0, len(entries) - len(node.children))
            node.expanded = True
        except OSError as exc:
            node.error = str(exc)
            node.expanded = False

    def visible(self):
        result = []
        stack = [self.root]
        while stack:
            node = stack.pop()
            result.append(node)
            if node.expanded:
                stack.extend(reversed(node.children))
        return result

    def refresh(self):
        self.search_signature = None
        selected_path = self.selected.path
        stack = [self.root]
        while stack:
            node = stack.pop()
            if node.expanded:
                self.expand(node)
                stack.extend(node.children)
        visible = self.visible()
        self.selected = next((n for n in visible if n.path == selected_path), self.root)

    def toggle_hidden(self):
        self.config.show_hidden = not self.config.show_hidden
        self.refresh()

    def step(self, direction):
        depth, ancestor = 0, self.selected
        while ancestor.parent:
            ancestor, depth = ancestor.parent, depth + 1
        nodes, stack = [], [(self.root, 0)]
        while stack:
            node, level = stack.pop()
            if level == depth:
                nodes.append(node)
            elif level < depth and node.expanded:
                stack.extend((child, level + 1) for child in reversed(node.children))
        if nodes:
            self.selected = nodes[(nodes.index(self.selected) + direction) % len(nodes)]

    def right(self):
        node = self.selected
        if node.symlink:
            return
        if not node.expanded:
            self.expand(node)
        elif node.children:
            self.selected = node.children[0]

    def left(self):
        if self.selected.expanded:
            self.selected.expanded = False

    def back(self):
        if self.selected.parent:
            self.selected = self.selected.parent
            return ""
        message = self.ascend()
        self.selected = self.root
        return message

    def focus_root(self):
        target = self.selected if self.selected.is_dir and not self.selected.symlink else self.selected.parent or self.root
        self.set_root(target.path)
        self.search_results = []
        self.search_signature = None
        self.search_summary = ""
        return f"当前目录设为根: {self.root.path}"

    def matches(self, query):
        return [n for n in self.visible() if query.casefold() in n.path.name.casefold()]

    def expand_all(self):
        node = self.selected
        if not node.is_dir or node.symlink:
            raise ValueError("请选择一个普通目录")
        stack, count, errors = [node], 0, 0
        limited = False
        while stack:
            current = stack.pop()
            if count >= self.config.max_expand_nodes:
                limited = True
                break
            count += 1
            if current.is_dir and not current.symlink:
                self.expand(current)
                errors += bool(current.error)
                limited |= bool(current.omitted)
                stack.extend(reversed(current.children))
        return f"已展开 {count} 个节点" + ("；达到显示 / 展开上限" if limited else "") + (f"；{errors} 个目录无法读取" if errors else "")

    def collapse_all(self):
        node = self.selected
        if not node.is_dir or node.symlink:
            raise ValueError("请选择一个普通目录")
        stack = [node]
        while stack:
            current = stack.pop()
            current.expanded = False
            stack.extend(current.children)
        return "已收起选中目录的全部分支"

    def reveal(self, path):
        """Reveal only the ancestry of a search result; other branches stay open."""
        path = Path(path)
        relative = path.relative_to(self.root.path)
        node = self.root
        for part in relative.parts:
            target = node.path / part
            self._pinned.add(target)
            self.expand(node)
            node = next((child for child in node.children if child.path == target), None)
            if node is None:
                raise FileNotFoundError(f"无法定位，文件可能已变更: {target}")
        self.selected = node
        return node

    def search(self, query):
        """Bounded recursive filename search, including collapsed directories."""
        results, scanned, errors, limited = [], 0, 0, False
        stack = [self.root.path]
        needle = query.casefold()
        while stack and needle:
            directory = stack.pop()
            try:
                with os.scandir(directory) as iterator:
                    for entry in iterator:
                        if scanned >= self.config.search_scan_limit:
                            limited = True
                            break
                        scanned += 1
                        if not self.config.show_hidden and entry.name.startswith("."):
                            continue
                        path = directory / entry.name
                        if needle in entry.name.casefold():
                            results.append(path)
                            if len(results) >= self.config.search_max_results:
                                limited = True
                                break
                        try:
                            if entry.is_dir(follow_symlinks=False):
                                stack.append(path)
                        except OSError:
                            errors += 1
            except OSError:
                errors += 1
            if limited:
                break
        results.sort(key=lambda p: (len(p.parts), str(p).casefold()))
        self.search_results = results
        self.search_signature = (query, self.root.path, self.config.show_hidden)
        self.search_summary = f"{len(results)} 个结果 · 检查 {scanned} 项" + (" · 已达搜索上限" if limited else "") + (f" · 跳过 {errors} 个无法读取的目录 / 条目" if errors else "")
        return results

    def find_next(self, query, direction=1):
        signature = (query, self.root.path, self.config.show_hidden)
        new_search = self.search_signature != signature
        if new_search:
            self.search(query)
        if not self.search_results:
            return "没有匹配的文件 / 目录；" + self.search_summary
        current = self.selected.path
        index = (self.search_results.index(current) + direction) % len(self.search_results) if not new_search and current in self.search_results else 0
        self.reveal(self.search_results[index])
        return f"{index + 1}/{len(self.search_results)} · {self.search_summary}"

    def target_directory(self):
        node = self.selected
        return node.path if node.is_dir else node.path.parent


def resolve_destination(source, text):
    path = Path(text).expanduser()
    if not path.is_absolute():
        path = source.parent / path
    return path.absolute()


def validate_destination(source, destination):
    if destination.exists() or destination.is_symlink():
        raise FileExistsError(f"目标已存在: {destination}")
    if not destination.parent.is_dir():
        raise ValueError(f"父目录不存在: {destination.parent}")
    if source.is_dir() and not source.is_symlink():
        try:
            destination.resolve().relative_to(source.resolve())
        except ValueError:
            pass
        else:
            raise ValueError("不能将目录复制或移动到自身内部")


def create_entry(directory, name, folder=False):
    if not name or name in {".", ".."} or "/" in name or "\0" in name:
        raise ValueError("名称不能为空，且不能含 /、NUL 或为 . / ..")
    path = directory / name
    if folder:
        path.mkdir()
    else:
        with path.open("xb"):
            pass
    return path


def transfer(source, destination, move=False):
    source, destination = Path(source), Path(destination)
    validate_destination(source, destination)
    if move:
        try:
            return rename_path(source, destination)
        except OSError as exc:
            import errno
            if exc.errno != errno.EXDEV:
                raise
    # Cross-device moves copy first, deleting the source only after copying succeeds.
    # Reserve the destination first: collisions cannot silently replace user data.
    if source.is_symlink():
        destination.symlink_to(os.readlink(source))
        if move:
            try:
                source.unlink()
            except OSError:
                destination.unlink()
                raise
    elif source.is_dir():
        destination.mkdir()
        shutil.copytree(source, destination, dirs_exist_ok=True, symlinks=True)
        if move:
            shutil.rmtree(source)
    elif source.is_file():
        with source.open("rb") as reader, destination.open("xb") as writer:
            shutil.copyfileobj(reader, writer)
        shutil.copystat(source, destination)
        if move:
            source.unlink()
    else:
        raise ValueError("仅支持普通文件、目录和符号链接")
    return destination


def rename(source, name):
    if not name or name in {".", ".."} or "/" in name or "\0" in name:
        raise ValueError("请输入单个有效名称")
    destination = source.parent / name
    validate_destination(source, destination)
    return rename_path(source, destination)


def rename_path(source, destination):
    # Linux renameat2(RENAME_NOREPLACE) gives atomic no-overwrite rename.
    import ctypes
    library = ctypes.CDLL(None, use_errno=True)
    renameat2 = library.renameat2
    renameat2.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    renameat2.restype = ctypes.c_int
    if renameat2(-100, os.fsencode(source), -100, os.fsencode(destination), 1):
        error = ctypes.get_errno()
        raise OSError(error, os.strerror(error), str(destination))
    return destination


def trash(path):
    executable = shutil.which("gio")
    if not executable:
        raise RuntimeError("回收站需要 gio，请安装 glib2；不会退回永久删除")
    try:
        subprocess.run([executable, "trash", "--", str(path)], check=True, capture_output=True, text=True)
    except subprocess.CalledProcessError as exc:
        raise RuntimeError(f"回收站操作失败: {(exc.stderr or str(exc)).strip()}") from exc


def open_default(path):
    return subprocess.Popen(["xdg-open", str(path)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def preview(path, limit=65536):
    try:
        info = path.lstat()
        if stat.S_ISLNK(info.st_mode):
            return f"符号链接 → {safe_text(os.readlink(path))}\n用打开键访问目标；树不会自动跟随链接。"
        if stat.S_ISDIR(info.st_mode):
            return f"目录\n{safe_text(path)}\n展开以查看内容。"
        if not stat.S_ISREG(info.st_mode):
            return "特殊文件（设备、管道或套接字），不读取内容。"
        with path.open("rb") as stream:
            data = stream.read(limit + 1)
        mime = mimetypes.guess_type(path.name)[0] or "application/octet-stream"
        header = f"{mime} · {info.st_size:,} bytes\n\n"
        if b"\0" in data or mime.startswith(("image/", "audio/", "video/")):
            return header + "二进制文件；用默认程序打开。"
        try:
            import codecs
            content = codecs.getincrementaldecoder("utf-8")().decode(data[:limit], final=len(data) <= limit)
        except UnicodeDecodeError:
            return header + "非 UTF-8 文本或二进制文件；用默认程序打开。"
        content = "\n".join(safe_text(line.expandtabs(4)) for line in content.splitlines())
        return header + content + ("\n…预览已截断" if len(data) > limit else "")
    except OSError as exc:
        return f"无法预览: {safe_text(exc)}"
