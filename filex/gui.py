"""Native Qt GUI, using the same model, operations and tree layout as the TUI."""
from pathlib import Path
from PySide6.QtCore import Qt, Signal, QObject, QRunnable, QThreadPool, QRectF
from PySide6.QtGui import QColor, QFont, QFontMetricsF, QPainter, QPen, QAction, QKeySequence
from PySide6.QtWidgets import (QApplication, QMainWindow, QWidget, QVBoxLayout, QHBoxLayout,
    QLineEdit, QPushButton, QSplitter, QTextEdit, QGraphicsScene, QGraphicsView,
    QGraphicsRectItem, QGraphicsSimpleTextItem, QMenu, QInputDialog, QMessageBox,
    QFileDialog, QToolBar, QLabel)
from .actions import ARG_LABELS, perform
from .config import config_path, load_config, key_map, display_color, key_code, navigation_keys, navigation_hint
from .core import preview, safe_text
from .details import metadata, metadata_text
from .opening import default_opener, opener_choices, launch_open
from .layout import Layout


class WorkerSignals(QObject):
    done = Signal(object)
    failed = Signal(str)


class Worker(QRunnable):
    def __init__(self, operation):
        super().__init__()
        self.operation = operation
        self.signals = WorkerSignals()

    def run(self):
        try:
            self.signals.done.emit(self.operation())
        except Exception as exc:
            self.signals.failed.emit(str(exc))


class TreeView(QGraphicsView):
    selected = Signal(object)
    activated = Signal(object)
    context = Signal(object, object)
    navigation = Signal(str)

    def __init__(self):
        super().__init__()
        self.arrow_actions = navigation_keys("horizontal")
        self.setScene(QGraphicsScene(self))
        self.setRenderHint(QPainter.RenderHint.Antialiasing)
        self.setDragMode(QGraphicsView.DragMode.ScrollHandDrag)
        self.setTransformationAnchor(QGraphicsView.ViewportAnchor.AnchorUnderMouse)
        self.setAlignment(Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignTop)

    def node_at(self, position):
        item = self.itemAt(position)
        while item:
            node = item.data(0)
            if node is not None:
                return node
            item = item.parentItem()
        return None

    def mousePressEvent(self, event):
        node = self.node_at(event.position().toPoint())
        if node and event.button() == Qt.MouseButton.LeftButton:
            self.selected.emit(node)
            event.accept()
        else:
            super().mousePressEvent(event)

    def mouseDoubleClickEvent(self, event):
        node = self.node_at(event.position().toPoint())
        if node:
            self.activated.emit(node)
        else:
            super().mouseDoubleClickEvent(event)

    def contextMenuEvent(self, event):
        node = self.node_at(event.pos())
        if node:
            self.context.emit(node, event.globalPos())

    def wheelEvent(self, event):
        if event.modifiers() & Qt.KeyboardModifier.ControlModifier:
            factor = 1.15 if event.angleDelta().y() > 0 else 1 / 1.15
            scale = self.transform().m11() * factor
            if .3 <= scale <= 3:
                self.scale(factor, factor)
            event.accept()
        else:
            super().wheelEvent(event)

    def keyPressEvent(self, event):
        fixed = {Qt.Key.Key_Down: self.arrow_actions["down"], Qt.Key.Key_Up: self.arrow_actions["up"],
                 Qt.Key.Key_Left: self.arrow_actions["left"], Qt.Key.Key_Right: self.arrow_actions["right"],
                 Qt.Key.Key_Return: "open_with", Qt.Key.Key_Enter: "open_with",
                 Qt.Key.Key_Space: "expand", Qt.Key.Key_Escape: "clear"}
        action = fixed.get(event.key())
        if action:
            self.navigation.emit(action)
            event.accept()
        elif event.text() in self.keys:
            self.navigation.emit(self.keys[event.text()])
            event.accept()
        else:
            super().keyPressEvent(event)


class Window(QMainWindow):
    def __init__(self, browser, config_file=None):
        super().__init__()
        self.browser = browser
        self.config_file = config_file
        self.query = ""
        self.dashboard = False
        self.metadata_path = None
        self.metadata_value = ""
        self.binding_actions = []
        self.rects = {}
        self.workers = set()
        self.busy = False
        self.setWindowTitle("filex")
        self.resize(1200, 780)
        container = QWidget()
        self.setCentralWidget(container)
        layout = QVBoxLayout(container)
        layout.setContentsMargins(16, 12, 16, 8)
        row = QHBoxLayout()
        self.parent_button = QPushButton("← 父级")
        self.parent_button.clicked.connect(lambda: self.navigate("parent"))
        self.path = QLineEdit(str(browser.root.path))
        self.path.setPlaceholderText("输入目录路径后按 Enter")
        self.path.returnPressed.connect(lambda: self.go(self.path.text()))
        chooser = QPushButton("选择目录")
        chooser.clicked.connect(self.choose_directory)
        focus_root = QPushButton("设为根 f")
        focus_root.clicked.connect(lambda: self.navigate("focus_root"))
        row.addWidget(self.parent_button)
        row.addWidget(self.path, 1)
        row.addWidget(focus_root)
        row.addWidget(chooser)
        layout.addLayout(row)
        toolbar = QToolBar()
        toolbar.setMovable(False)
        self.toolbar = toolbar
        for label, action in [("展开", "expand"), ("折叠", "collapse"), ("全部展开", "expand_all"), ("全部收起", "collapse_all"), ("信息", "dashboard"), ("新文件", "new_file"),
                              ("新目录", "new_folder"), ("重命名", "rename"), ("复制", "copy"),
                              ("移动", "move"), ("回收站", "trash"), ("刷新", "refresh")]:
            item = toolbar.addAction(label)
            item.triggered.connect(lambda checked=False, a=action: self.navigate(a))
        self.hidden_action = toolbar.addAction("隐藏文件")
        self.hidden_action.setCheckable(True)
        self.hidden_action.triggered.connect(lambda: self.navigate("hidden"))
        self.orientation_action = toolbar.addAction("纵向树")
        self.orientation_action.setCheckable(True)
        self.orientation_action.triggered.connect(lambda: self.navigate("toggle_tree"))
        toolbar.addSeparator()
        toolbar.addAction("读取配置", lambda: self.navigate("config"))
        toolbar.addAction("适应画布", lambda: self.view.fitInView(self.view.sceneRect(), Qt.AspectRatioMode.KeepAspectRatio))
        toolbar.addAction("100%", lambda: self.view.resetTransform())
        toolbar.addAction("帮助", lambda: self.navigate("help"))
        layout.addWidget(toolbar)
        self.search = QLineEdit()
        self.search.setPlaceholderText("Ctrl+F · 搜索文件 / 目录名（含未展开目录） · Enter 定位 / 下一个 · Esc 清除")
        self.search.textChanged.connect(self.search_changed)
        self.search.returnPressed.connect(self.next_match)
        layout.addWidget(self.search)
        splitter = QSplitter()
        self.view = TreeView()
        self.view.selected.connect(self.select)
        self.view.activated.connect(self.activate)
        self.view.context.connect(self.context_menu)
        self.view.navigation.connect(self.navigate)
        self.preview = QTextEdit()
        self.preview.setReadOnly(True)
        self.preview.setMinimumWidth(180)
        splitter.addWidget(self.view)
        splitter.addWidget(self.preview)
        splitter.setSizes([850, 280])
        layout.addWidget(splitter, 1)
        self.hint = QLabel()
        layout.addWidget(self.hint)
        for seq, action in [("Ctrl+L", "goto"), ("Ctrl+F", "search"), ("F5", "refresh"), ("F2", "rename"), ("Delete", "trash")]:
            shortcut = QAction(self)
            shortcut.setShortcut(QKeySequence(seq))
            shortcut.triggered.connect(lambda checked=False, a=action: self.navigate(a))
            self.addAction(shortcut)
        clear = QAction(self)
        clear.setShortcut(QKeySequence("Esc"))
        clear.triggered.connect(lambda: self.navigate("clear"))
        self.addAction(clear)
        self.apply_theme()
        self.render()
        self.view.setFocus()

    def apply_theme(self):
        c = self.browser.config
        self.setStyleSheet(f"""
            QWidget {{ background: {c.background}; color: {c.foreground}; font-size: {c.gui_font_size}pt; }}
            QLineEdit, QTextEdit {{ border: 1px solid {c.line_color}; border-radius: 5px; padding: 6px; }}
            QPushButton, QToolButton {{ padding: 6px 9px; border: 1px solid transparent; border-radius: 5px; }}
            QPushButton:hover, QToolButton:hover, QToolButton:checked {{ background: {c.selected_color}; }}
            QSplitter::handle {{ background: {c.line_color}; width: 2px; }}
        """)
        self.view.setBackgroundBrush(QColor(c.background))
        self.view.keys = key_map(c)
        self.view.keys["/"] = "search"
        for action in self.binding_actions:
            self.removeAction(action)
            action.deleteLater()
        self.binding_actions.clear()
        for name, key in c.keys.items():
            if len(key_code(key)) == 1 and not key_code(key).isprintable() and key_code(key) not in {"\x06", "\x0c"}:
                action = QAction(self)
                action.setShortcut(QKeySequence(key))
                action.triggered.connect(lambda checked=False, n=name: self.navigate(n))
                self.addAction(action)
                self.binding_actions.append(action)
        self.hidden_action.setChecked(c.show_hidden)
        self.orientation_action.setChecked(c.orientation == "vertical")

    def execute(self, operation, on_done=None):
        if self.busy:
            return
        self.busy = True
        self.centralWidget().setEnabled(False)
        self.statusBar().showMessage("正在读取 / 操作文件…")
        worker = Worker(operation)
        self.workers.add(worker)
        def finish(result=None, error=None):
            self.workers.discard(worker)
            self.busy = False
            self.centralWidget().setEnabled(True)
            self.metadata_path = None
            self.apply_theme()
            self.render()
            if error:
                QMessageBox.warning(self, "操作失败", safe_text(error))
            elif result:
                self.statusBar().showMessage(f"完成: {safe_text(result)}", 8000)
            if not error and on_done:
                on_done(result)
        worker.signals.done.connect(lambda result: finish(result=result))
        worker.signals.failed.connect(lambda message: finish(error=message))
        QThreadPool.globalInstance().start(worker)

    def render(self, center=True):
        c = self.browser.config
        self.view.arrow_actions = navigation_keys(c.orientation)
        self.parent_button.setText("↑ 父级" if c.orientation == "vertical" else "← 父级")
        self.hint.setText(f"{navigation_hint(c.orientation)} · {c.keys['collapse']} 收起当前目录 · {c.keys['toggle_tree']} 横向/纵向 · Enter 打开方式 · {c.keys['dashboard']} 信息")
        layout = Layout(self.browser)
        scene = self.view.scene()
        scene.clear()
        self.rects.clear()
        font = QFont("monospace", c.gui_font_size)
        font.setStyleHint(QFont.StyleHint.Monospace)
        metrics = QFontMetricsF(font)
        unit = metrics.horizontalAdvance("M")
        row = metrics.height() + 22
        padding = 8
        for x1, y1, x2, y2 in layout.lines:
            # Model line ends are in cells; cards have symmetric padding.
            scene.addLine(24 + x1 * unit + padding, 24 + y1 * row + row / 2,
                          24 + x2 * unit + padding, 24 + y2 * row + row / 2,
                          QPen(QColor(c.line_color), 1.5))
        for p in layout.placements:
            rect = QGraphicsRectItem(24 + p.x * unit, 24 + p.y * row + 5,
                                     metrics.horizontalAdvance(p.label) + 16, row - 10)
            selected = p.node is self.browser.selected
            matched = self.query and self.query.casefold() in p.node.path.name.casefold()
            rect.setBrush(QColor(c.selected_color if selected else c.background))
            rect.setPen(QPen(QColor(c.folder_color if matched or selected else c.line_color), 1.5 if selected else .8))
            rect.setData(0, p.node)
            rect.setToolTip(safe_text(str(p.node.path)))
            scene.addItem(rect)
            text = QGraphicsSimpleTextItem(p.label, rect)
            text.setFont(font)
            text.setBrush(QColor(display_color(p.node, c)))
            text.setPos(rect.rect().x() + 8, rect.rect().y() + (rect.rect().height() - metrics.height()) / 2)
            self.rects[p.node] = rect
        scene.setSceneRect(scene.itemsBoundingRect().adjusted(-24, -24, 48, 48))
        self.path.setText(str(self.browser.root.path))
        self.hidden_action.setChecked(c.show_hidden)
        self.update_selection(center)

    def update_selection(self, center=True):
        c = self.browser.config
        for node, rect in self.rects.items():
            selected = node is self.browser.selected
            matched = self.query and self.query.casefold() in node.path.name.casefold()
            rect.setBrush(QColor(c.selected_color if selected else c.background))
            rect.setPen(QPen(QColor(c.folder_color if matched or selected else c.line_color), 1.5 if selected else .8))
        node = self.browser.selected
        if self.dashboard:
            if self.metadata_path != node.path:
                self.metadata_value = metadata_text(metadata(node.path))
                self.metadata_path = node.path
            self.preview.setPlainText(self.metadata_value)
        else:
            self.preview.setPlainText(safe_text(node.path.name) + "\n\n" + preview(node.path, c.preview_bytes))
        status = node.error or (f"还有 {node.omitted} 项未显示；调高 max_entries 后刷新" if node.omitted else str(node.path))
        self.statusBar().showMessage(safe_text(status))
        if center and node in self.rects:
            self.view.ensureVisible(self.rects[node], 30, 30)

    def select(self, node):
        if self.busy:
            return
        self.browser.selected = node
        self.update_selection(False)

    def activate(self, node):
        self.select(node)
        if node.is_dir:
            if node.expanded:
                self.navigate("collapse")
            else:
                self.navigate("expand")
        else:
            self.navigate("open_with")

    def go(self, path):
        self.execute(lambda: self.browser.set_root(path))

    def choose_directory(self):
        path = QFileDialog.getExistingDirectory(self, "选择根目录", str(self.browser.root.path))
        if path:
            self.go(path)

    def search_changed(self, text):
        if self.busy:
            return
        self.query = text
        self.render(False)

    def next_match(self, direction=1):
        if self.query:
            self.execute(lambda: self.browser.find_next(self.query, direction), lambda result: self.view.setFocus())

    def choose_open(self):
        node = self.browser.selected
        options = opener_choices(node, self.browser.config)
        if node.is_dir:
            options.insert(0, (None, "fileX 展开 / 收起当前目录"))
        labels = [label for _, label in options]
        label, accepted = QInputDialog.getItem(self, "选择打开方式", safe_text(node.path), labels, 0, False)
        if accepted:
            method = options[labels.index(label)][0]
            if method is None:
                if node.expanded:
                    node.expanded = False
                    self.render()
                else:
                    self.execute(lambda: self.browser.expand(node))
            else:
                self.open_method(method)

    def open_method(self, method):
        try:
            launch_open(self.browser.selected.path, self.browser.config, method, gui=True)
        except (OSError, RuntimeError) as exc:
            QMessageBox.warning(self, "打开失败", str(exc))

    def context_menu(self, node, point):
        self.select(node)
        menu = QMenu(self)
        for label, action in [("选择打开方式", "open_with"), ("默认程序打开", "open"), ("全部展开", "expand_all"), ("全部收起", "collapse_all"), ("文件信息", "dashboard"), ("新建文件", "new_file"), ("新建目录", "new_folder"),
                              ("重命名", "rename"), ("复制", "copy"), ("移动", "move"), ("送入回收站", "trash")]:
            menu.addAction(label, lambda a=action: self.navigate(a))
        menu.exec(point)

    def navigate(self, action):
        if self.busy:
            return
        if action == "quit":
            self.close()
        elif action == "dashboard":
            self.dashboard = not self.dashboard
            self.metadata_path = None
            self.update_selection(False)
        elif action == "down":
            self.browser.step(1)
            self.render()
        elif action == "up":
            self.browser.step(-1)
            self.render()
        elif action == "collapse":
            self.browser.left()
            self.render()
        elif action == "expand":
            self.execute(self.browser.right)
        elif action == "expand_all":
            self.execute(self.browser.expand_all)
        elif action == "collapse_all":
            self.execute(self.browser.collapse_all)
        elif action == "open":
            self.open_method(default_opener(self.browser.selected, self.browser.config))
        elif action == "open_with":
            self.choose_open()
        elif action == "parent":
            self.execute(self.browser.back)
        elif action == "focus_root":
            self.execute(self.browser.focus_root, lambda result: self.search.clear())
        elif action == "toggle_tree":
            self.browser.config.orientation = "vertical" if self.browser.config.orientation == "horizontal" else "horizontal"
            self.orientation_action.setChecked(self.browser.config.orientation == "vertical")
            self.render()
        elif action == "goto":
            self.path.setFocus()
            self.path.selectAll()
        elif action == "search":
            self.search.setFocus()
            self.search.selectAll()
        elif action == "clear":
            self.search.clear()
            self.view.setFocus()
        elif action in {"next_match", "previous_match"}:
            self.next_match(1 if action == "next_match" else -1)
        elif action == "hidden":
            self.execute(self.browser.toggle_hidden)
        elif action == "refresh":
            self.execute(self.browser.refresh)
        elif action in ARG_LABELS:
            initial = self.browser.selected.path.name if action == "rename" else ""
            value, accepted = QInputDialog.getText(self, "文件操作", ARG_LABELS[action] + "\n源: " + safe_text(self.browser.selected.path), text=initial)
            if accepted and value:
                self.execute(lambda: perform(self.browser, action, value))
        elif action == "trash":
            node = self.browser.selected
            answer = QMessageBox.question(self, "送入回收站", f"将以下项目送入回收站？\n{safe_text(node.path)}\n可在 Dolphin 回收站恢复。", QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No, QMessageBox.StandardButton.No)
            if answer == QMessageBox.StandardButton.Yes:
                self.execute(lambda: perform(self.browser, "trash"))
        elif action == "preview":
            self.dashboard = False
            self.update_selection(False)
            self.preview.setFocus()
        elif action == "config":
            def reload():
                self.browser.config = load_config(self.config_file or config_path())
                self.browser.refresh()
            self.execute(reload)
        elif action == "help":
            lines = "\n".join(f"{'Up' if name == 'parent' and key_code(key) == 'left' and self.browser.config.orientation == 'vertical' else key}  {name}" for name, key in self.browser.config.keys.items())
            QMessageBox.information(self, "fileX 使用帮助", "键盘焦点在画布时，可使用与 TUI 相同的快捷键。\n\n" + lines + "\n\nEnter 选择打开方式 · o 扩展名默认程序\nCtrl+L 路径 · Ctrl+F 递归搜寻 · F5 刷新\nF2 重命名 · Delete 回收站\n复制 / 移动填写完整目标路径；不会覆盖。\n扩展名配色同时用于 TUI / GUI；逗号重新读取配置。")

    def closeEvent(self, event):
        if self.busy:
            event.ignore()
            self.statusBar().showMessage("请等待当前文件操作完成后关闭窗口")
        else:
            event.accept()


def run(browser, config_file=None):
    app = QApplication.instance() or QApplication([])
    app.setApplicationName("filex")
    app.setDesktopFileName("filex")
    window = Window(browser, config_file)
    window.show()
    return app.exec()
