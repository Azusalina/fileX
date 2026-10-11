"""Run explicitly in a KDE Wayland session: QT_QPA_PLATFORM=wayland python tests/wayland_smoke.py."""
from pathlib import Path
import sys
import tempfile
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from PySide6.QtCore import QTimer
from PySide6.QtGui import QImage, QColor
from PySide6.QtWidgets import QApplication
from filex.config import Config
from filex.core import Browser
from filex.gui import Window

with tempfile.TemporaryDirectory() as directory:
    root = Path(directory) / "folder(a)"
    (root / "folder(a1)").mkdir(parents=True)
    (root / "folder(a2)").mkdir()
    (root / "folder(a1)" / "file(a1a).txt").write_text("filex v1.4\nArch Linux · KDE Wayland\n\n←→ 父子导航\n↑↓ 同级选择\nr/R 重命名\nCtrl+Alt+C 复制路径\n")
    for name in ("file(a2a).txt", "file(a2b).txt", "中文文件.txt"):
        (root / "folder(a2)" / name).write_text("示例")
    image = QImage(120, 80, QImage.Format.Format_RGB32)
    image.fill(QColor("#7dcfff"))
    image.save(str(root / "folder(a2)" / "image(a2c).png"))
    app = QApplication([])
    assert app.platformName().startswith("wayland"), app.platformName()
    browser = Browser(root, Config())
    for node in browser.root.children:
        browser.expand(node)
    browser.selected = browser.root.children[0].children[0]
    window = Window(browser)
    window.show()
    window.navigate("dashboard")
    def capture():
        output = Path(__file__).resolve().parents[1] / "preview.png"
        import json
        values = dict(line.split(":", 1) for line in Path("/proc/self/status").read_text().splitlines() if ":" in line)
        rss = int(values["VmRSS"].split()[0])
        baseline = {"backend": "Python/PySide6 existing GUI", "platform": app.platformName(), "visible_nodes": len(window.rects), "rss_kib": rss, "rss_mb": round(rss * 1024 / 1_000_000, 3), "future_target_mb": 50, "target_achieved": rss * 1024 < 50_000_000}
        (output.parent / "gui-memory-baseline.json").write_text(json.dumps(baseline, ensure_ascii=False, indent=2) + "\n")
        assert window.grab().save(str(output))
        print(f"platform={app.platformName()} nodes={len(window.rects)} screenshot={output}")
        assert window.windowTitle() == "filex"
        window.close()
    QTimer.singleShot(750, capture)
    app.exec()
