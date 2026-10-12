import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from filex.config import Config, load_config
from filex.core import Browser, create_entry, preview, rename, transfer, cell_width, shorten, safe_text, trash
from filex.layout import Layout, plain_tree
from filex.actions import perform


class ExplorerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "a1").mkdir()
        (self.root / "a2").mkdir()
        (self.root / "a1" / "file1").write_text("hello 世界\n")
        (self.root / "a2" / "file2").write_text("two")
        (self.root / "a2" / "file3").write_text("three")
        (self.root / ".hidden").touch()
        self.browser = Browser(self.root, Config())

    def find(self, name):
        return next(n for n in self.browser.visible() if n.path.name == name)

    def test_lazy_multiple_branches_and_lines(self):
        self.assertEqual(len(self.browser.visible()), 3)
        self.browser.expand(self.find("a1"))
        self.browser.expand(self.find("a2"))
        layout = Layout(self.browser)
        self.assertEqual(layout.by_node[self.find("a1")].y, 0)
        self.assertEqual(layout.by_node[self.find("file1")].y, 0)
        self.assertGreater(layout.by_node[self.find("a2")].y, 0)
        self.assertIn("┬", plain_tree(self.browser))
        self.assertIn("└", plain_tree(self.browser))
        self.assertIn("file3", plain_tree(self.browser))

    def test_navigation_refresh_and_hidden(self):
        self.browser.right()
        self.assertEqual(self.browser.selected.path.name, "a1")
        self.browser.right()
        self.browser.right()
        self.assertEqual(self.browser.selected.path.name, "file1")
        self.browser.refresh()
        self.assertEqual(self.browser.selected.path.name, "file1")
        self.browser.back()
        self.browser.left()
        self.assertFalse(self.browser.selected.expanded)
        self.browser.toggle_hidden()
        self.assertIn(".hidden", [n.path.name for n in self.browser.visible()])

    def test_links_not_followed_and_copied_as_links(self):
        link = self.root / "loop"
        link.symlink_to(self.root, target_is_directory=True)
        broken = self.root / "broken"
        broken.symlink_to(self.root / "missing")
        self.browser.refresh()
        node = self.find("loop")
        self.browser.expand(node)
        self.assertFalse(node.expanded)
        transfer(link, self.root / "copy-link")
        self.assertTrue((self.root / "copy-link").is_symlink())
        transfer(broken, self.root / "broken-copy")
        self.assertTrue((self.root / "broken-copy").is_symlink())
        self.assertIn("符号链接", preview(broken))

    def test_atomic_rename_move_and_no_overwrite(self):
        source = self.root / "a1" / "file1"
        original_inode = source.stat().st_ino
        renamed = rename(source, "中文 新文件")
        self.assertEqual(renamed.stat().st_ino, original_inode)
        moved = transfer(renamed, self.root / "a2" / "moved", move=True)
        self.assertEqual(moved.stat().st_ino, original_inode)
        self.assertFalse(renamed.exists())
        target = self.root / "existing"
        target.write_text("keep")
        for fn in (lambda: transfer(moved, target), lambda: transfer(moved, target, True)):
            with self.assertRaises(FileExistsError):
                fn()
        self.assertEqual(target.read_text(), "keep")
        self.assertTrue(moved.exists())

    def test_recursive_copy_and_self_transfer_rejected(self):
        transfer(self.root / "a2", self.root / "a2-copy")
        self.assertEqual((self.root / "a2-copy" / "file3").read_text(), "three")
        for move in (False, True):
            with self.assertRaises(ValueError):
                transfer(self.root / "a2", self.root / "a2" / "nested", move)
        with self.assertRaises(FileExistsError):
            rename(self.root / "a1", "a2")

    def test_creation_operations_and_root_protection(self):
        perform(self.browser, "new_folder", "new")
        self.assertEqual(self.browser.selected.path.name, "new")
        perform(self.browser, "new_file", "中文.txt")
        self.assertTrue((self.root / "new" / "中文.txt").exists())
        self.browser.selected = self.browser.root
        for action in ("trash", "move", "rename"):
            with self.assertRaises(ValueError):
                perform(self.browser, action, "unused")
        for name in ("", "../bad", ".", "..", "a/b", "\0"):
            with self.assertRaises(ValueError):
                create_entry(self.root, name)
        with self.assertRaises(FileExistsError):
            create_entry(self.root, "a1", True)

    def test_config_validation_and_customization(self):
        path = self.root / "config.conf"
        path.write_text('[browser]\nshow_hidden = true\nsort = "modified"\n[layout]\ncolumn_gap = 10\n[keys]\nquit = "x"\n')
        config = load_config(path)
        self.assertTrue(config.show_hidden)
        self.assertEqual(config.keys["quit"], "x")
        self.assertNotEqual(config.keys["exit_here"], "x")
        for content in ('[layout]\ncolumn_gap = true', '[keys]\nquit = "j"', '[layout]\ncolumn_gap = 0', '[browser]\nsort = "bad"', '[what]\nx=1', '[colors]\nbackground="red"', '[keys]\nquit="H"', 'invalid'):
            path.write_text(content)
            with self.assertRaises(ValueError, msg=content):
                load_config(path)
        load_config(Path(__file__).resolve().parents[1] / "fileX.example.conf")

    def test_preview_bounded_binary_and_fifo(self):
        path = self.root / "text"
        path.write_text("世界\n" * 1000)
        self.assertIn("截断", preview(path, 1024))
        path.write_bytes(b"\x00\x01")
        self.assertIn("二进制", preview(path))
        fifo = self.root / "pipe"
        os.mkfifo(fifo)
        self.assertIn("特殊文件", preview(fifo))
        path.write_bytes(b"\xff")
        self.assertIn("非 UTF-8", preview(path))

    def test_labels_controls_and_wide_characters(self):
        self.assertEqual(cell_width("中文a"), 5)
        self.assertEqual(cell_width("e\u0301"), 1)
        self.assertLessEqual(cell_width(shorten("中文目录很长", 7)), 7)
        self.assertNotIn("\x1b", safe_text("bad\x1b[31m\nname"))
        weird = self.root / "文件\n\x1b"
        weird.touch()
        self.browser.refresh()
        self.assertNotIn("\x1b", plain_tree(self.browser))

    def test_limit_sort_and_permission_error(self):
        browser = Browser(self.root, Config(max_entries=1))
        self.assertEqual(len(browser.root.children), 1)
        self.assertEqual(browser.root.omitted, 1)
        with patch("filex.core.os.scandir", side_effect=PermissionError("denied")):
            browser.expand(browser.root)
        self.assertIn("denied", browser.root.error)
        self.assertFalse(browser.root.expanded)

    def test_cross_device_move_and_failed_copy_keep_source(self):
        import errno
        source = self.root / "a1" / "file1"
        destination = self.root / "cross-device"
        with patch("filex.core.rename_path", side_effect=OSError(errno.EXDEV, "different filesystem")):
            transfer(source, destination, True)
        self.assertEqual(destination.read_text(), "hello 世界\n")
        self.assertFalse(source.exists())
        failed = self.root / "partial-copy"
        with patch("filex.core.shutil.copyfileobj", side_effect=OSError("copy failed")):
            with self.assertRaises(OSError):
                transfer(destination, failed)
        self.assertTrue(destination.exists())

    def test_actual_gio_trash_in_isolated_data_directory(self):
        import shutil
        if not shutil.which("gio"):
            self.skipTest("gio not installed")
        # /tmp is an internal tmpfs on this host; gio deliberately refuses trash there.
        with tempfile.TemporaryDirectory(prefix=".trash-fixture-", dir=Path(__file__).resolve().parents[1]) as directory:
            root = Path(directory)
            path = root / "real-trash-test.txt"
            path.write_text("fileX trash fixture")
            data = root / "isolated-data"
            data.mkdir()
            with patch.dict(os.environ, {"XDG_DATA_HOME": str(data)}):
                trash(path)
            self.assertFalse(path.exists())
            trashed = data / "Trash/files/real-trash-test.txt"
            self.assertEqual(trashed.read_text(), "fileX trash fixture")
            self.assertTrue((data / "Trash/info/real-trash-test.txt.trashinfo").exists())

    def test_trash_only_external_trash_never_unlink(self):
        path = self.root / "trash-me"
        path.touch()
        with patch("filex.core.shutil.which", return_value=None):
            with self.assertRaises(RuntimeError):
                trash(path)
        self.assertTrue(path.exists())
        with patch("filex.core.shutil.which", return_value="/usr/bin/gio"), patch("filex.core.subprocess.run") as run:
            trash(path)
            self.assertEqual(run.call_args.args[0], ["/usr/bin/gio", "trash", "--", str(path)])


if __name__ == "__main__":
    unittest.main()
