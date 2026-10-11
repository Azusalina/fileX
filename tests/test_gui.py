import os
os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
from pathlib import Path
import tempfile
import time
import unittest
try:
    from PySide6.QtWidgets import QApplication
    from filex.gui import Window
except ImportError:
    QApplication = None
from filex.core import Browser
from filex.config import Config


@unittest.skipIf(QApplication is None, "GUI requires PySide6")
class GuiTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        (self.root / "中文目录").mkdir()
        (self.root / "中文目录" / "file.txt").write_text("hello 世界")
        self.browser = Browser(self.root, Config())
        self.window = Window(self.browser)
        self.window.show()
        self.app.processEvents()

    def tearDown(self):
        self.wait()
        self.window.close()
        self.app.processEvents()
        self.temp.cleanup()

    def wait(self):
        deadline = time.monotonic() + 5
        while self.window.busy and time.monotonic() < deadline:
            self.app.processEvents()
            time.sleep(.005)
        self.app.processEvents()
        self.assertFalse(self.window.busy, "background task did not complete")

    def test_expand_search_preview_and_hidden(self):
        self.window.navigate("expand")
        self.wait()
        self.assertEqual(self.browser.selected.path.name, "中文目录")
        self.window.navigate("expand")
        self.wait()
        self.window.search.setText("file")
        self.window.next_match()
        self.wait()
        self.assertEqual(self.browser.selected.path.name, "file.txt")
        self.assertIn("hello 世界", self.window.preview.toPlainText())
        self.assertEqual(len(self.window.rects), 3)
        self.window.navigate("hidden")
        self.wait()
        self.assertTrue(self.browser.config.show_hidden)
        image = self.window.grab().toImage()
        self.assertGreater(image.width(), 1000)
        self.assertFalse(image.isNull())

    def test_background_operation(self):
        from filex.actions import perform
        self.window.execute(lambda: perform(self.browser, "new_file", "new.txt"))
        self.wait()
        self.assertTrue((self.root / "new.txt").exists())
        self.assertTrue(self.window.centralWidget().isEnabled())
        self.assertEqual(self.browser.selected.path.name, "new.txt")

    def test_dashboard_full_branch_and_parent_preservation(self):
        self.window.navigate("dashboard")
        self.assertIn("Created", self.window.preview.toPlainText())
        self.assertIn("Size", self.window.preview.toPlainText())
        original = self.browser.root
        self.window.navigate("expand_all")
        self.wait()
        from PySide6.QtTest import QTest
        from PySide6.QtCore import Qt
        self.window.view.setFocus()
        QTest.keyClick(self.window.view, Qt.Key.Key_F, Qt.KeyboardModifier.ControlModifier)
        self.app.processEvents()
        self.assertTrue(self.window.search.hasFocus())
        self.window.search.setText("file.txt")
        QTest.keyClick(self.window.search, Qt.Key.Key_Return)
        self.wait()
        selected = self.browser.selected
        self.window.navigate("parent")
        self.wait()
        self.assertIs(self.browser.selected, selected.parent)
        self.assertIs(self.browser.root, original)
        self.window.navigate("parent")
        self.wait()
        self.assertIs(self.browser.selected, original)
        self.window.navigate("parent")
        self.wait()
        self.assertIn(original, self.browser.root.children)
        self.assertIs(self.browser.selected, self.browser.root)
        self.assertTrue(original.expanded)
        self.browser.selected = original
        self.window.navigate("collapse_all")
        self.wait()
        self.assertFalse(original.expanded)

    def test_arrow_siblings_and_vertical_toggle_preserve_open_branches(self):
        from PySide6.QtTest import QTest
        from PySide6.QtCore import Qt
        (self.root / "末尾目录").mkdir()
        self.browser.refresh()
        self.window.navigate("expand_all")
        self.wait()
        self.window.navigate("expand")
        self.wait()
        folder = self.browser.selected
        self.window.view.setFocus()
        QTest.keyClick(self.window.view, Qt.Key.Key_Down)
        self.assertEqual(self.browser.selected.path.name, "末尾目录")
        QTest.keyClick(self.window.view, Qt.Key.Key_Up)
        self.assertIs(self.browser.selected, folder)
        self.assertTrue(folder.expanded)
        QTest.keyClick(self.window.view, Qt.Key.Key_Right)
        self.wait()
        self.assertEqual(self.browser.selected.path.name, "file.txt")
        QTest.keyClick(self.window.view, Qt.Key.Key_Left)
        self.wait()
        self.assertIs(self.browser.selected, folder)
        self.assertTrue(folder.expanded)
        QTest.keyClick(self.window.view, Qt.Key.Key_V)
        self.assertEqual(self.browser.config.orientation, "vertical")
        self.assertTrue(folder.expanded)
        self.assertEqual(self.window.windowTitle(), "filex")
        self.assertTrue(self.window.orientation_action.isChecked())
        QTest.keyClick(self.window.view, Qt.Key.Key_Right)
        self.assertEqual(self.browser.selected.path.name, "末尾目录")
        QTest.keyClick(self.window.view, Qt.Key.Key_Left)
        self.assertIs(self.browser.selected, folder)
        QTest.keyClick(self.window.view, Qt.Key.Key_Down)
        self.wait()
        self.assertEqual(self.browser.selected.path.name, "file.txt")
        QTest.keyClick(self.window.view, Qt.Key.Key_Right)
        self.assertEqual(self.browser.selected.path.name, "file.txt")
        QTest.keyClick(self.window.view, Qt.Key.Key_Up)
        self.wait()
        self.assertIs(self.browser.selected, folder)
        self.assertTrue(folder.expanded)
        QTest.keyClick(self.window.view, Qt.Key.Key_V)
        self.assertEqual(self.browser.config.orientation, "horizontal")
        other = next(child for child in self.browser.root.children if child is not folder)
        QTest.keyClick(self.window.view, Qt.Key.Key_H)
        self.assertFalse(folder.expanded)
        self.assertIs(self.browser.selected, folder)
        self.assertTrue(other.expanded)
        QTest.keyClick(self.window.view, Qt.Key.Key_F)
        self.wait()
        self.assertIs(self.browser.root, folder)
        self.assertIs(self.browser.selected, folder)
        self.assertTrue(folder.expanded)
        self.assertEqual(len(self.window.rects), 2)
        self.assertIsNone(folder.parent)

    def test_enter_chooser_and_extension_color(self):
        from unittest.mock import patch
        self.window.search.setText("file.txt")
        self.window.next_match()
        self.wait()
        self.browser.config.extension_colors[".txt"] = "#abcdef"
        self.window.render()
        rect = self.window.rects[self.browser.selected]
        self.assertEqual(rect.childItems()[0].brush().color().name(), "#abcdef")
        with patch("filex.gui.launch_open") as launch:
            # Use the actual configured menu label, including the default marker if any.
            from filex.opening import opener_choices
            label = next(label for name, label in opener_choices(self.browser.selected, self.browser.config) if name == "vim")
            with patch("filex.gui.QInputDialog.getItem", return_value=(label, True)):
                self.window.navigate("open_with")
            self.assertEqual(launch.call_args.args[2], "vim")
            self.assertTrue(launch.call_args.kwargs["gui"])

    def test_invalid_reload_preserves_config(self):
        path = self.root / "invalid.conf"
        path.write_text('[layout]\ncolumn_gap = 0')
        self.window.config_file = path
        # Avoid opening a modal error in the automated test.
        from unittest.mock import patch
        old = self.browser.config
        with patch("filex.gui.QMessageBox.warning") as warning:
            self.window.navigate("config")
            self.wait()
            warning.assert_called_once()
        self.assertIs(self.browser.config, old)


if __name__ == "__main__":
    unittest.main()
