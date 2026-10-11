from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from filex.config import Config, load_config, key_map, display_color
from filex.core import Browser
from filex.details import metadata, metadata_text
from filex.opening import default_opener, opener_choices, prepare_open, launch_open


class NavigationFeatures(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for name in ("left/deep", "right/deep", ".hidden/deep"):
            (self.root / name).mkdir(parents=True)
            (self.root / name / "needle 中文.PY").write_text("print('hello')")
        (self.root / "loop").symlink_to(self.root, target_is_directory=True)
        self.browser = Browser(self.root, Config())

    def test_recursive_search_reveals_without_collapsing_other_branches(self):
        left = next(n for n in self.browser.root.children if n.path.name == "left")
        self.browser.selected = left
        self.browser.expand_all()
        deep = left.children[0]
        self.browser.find_next("needle")
        self.browser.find_next("needle")
        self.assertIn("right", str(self.browser.selected.path.relative_to(self.root)))
        self.assertTrue(left.expanded)
        self.assertTrue(deep.expanded)
        self.assertEqual(len(self.browser.search_results), 2)
        self.browser.toggle_hidden()
        self.browser.search("needle")
        self.assertEqual(len(self.browser.search_results), 3)

    def test_ascend_keeps_tree_identity_selection_and_branches(self):
        self.browser.selected = self.browser.root
        self.browser.expand_all()
        self.browser.find_next("needle")
        tree, selected = self.browser.root, self.browser.selected
        expanded = [n for n in self.browser.visible() if n.expanded]
        self.browser.ascend()
        self.assertIs(tree.parent, self.browser.root)
        self.assertIn(tree, self.browser.root.children)
        self.assertIs(selected, self.browser.selected)
        self.assertTrue(all(n.expanded for n in expanded))
        self.assertIn(selected, self.browser.visible())
        self.browser.set_root(tree.path)
        self.assertIs(self.browser.root, tree)
        self.assertTrue(tree.children[0].expanded)

    def test_full_branch_collapse_and_expand(self):
        root = self.browser.root
        self.browser.expand_all()
        self.assertGreater(len(self.browser.visible()), 3)
        self.browser.collapse_all()
        self.assertEqual(self.browser.visible(), [root])
        self.assertTrue(all(not n.expanded for n in self.browser._nodes.values()))
        self.browser.expand_all()
        self.assertGreater(len(self.browser.visible()), 3)
        self.assertFalse(next(n for n in self.browser.visible() if n.path.name == "loop").expanded)

    def test_search_limits_and_reveal_past_display_limit(self):
        self.browser.config.max_entries = 1
        self.browser.refresh()
        self.browser.find_next("needle")
        self.browser.find_next("needle")
        self.assertEqual(self.browser.selected.path, self.root / "right/deep/needle 中文.PY")
        self.assertIn(self.browser.selected, self.browser.visible())
        self.browser.config.search_max_results = 1
        self.browser.search("needle")
        self.assertEqual(len(self.browser.search_results), 1)
        self.assertIn("上限", self.browser.search_summary)
        self.browser.config.max_expand_nodes = 1
        self.browser.selected = self.browser.root
        self.assertIn("上限", self.browser.expand_all())

    def test_metadata_birthtime_missing_never_uses_ctime(self):
        path = self.root / "left/deep/needle 中文.PY"
        with patch("filex.details.birth_time", return_value=None):
            info = metadata(path)
        self.assertIn("不可用", info["created"])
        self.assertEqual(info["size_bytes"], path.stat().st_size)
        self.assertIn("Modified", metadata_text(info))
        self.assertIn("目录自身", metadata(self.root)["size_note"])
        self.assertIn("error", metadata(self.root / "missing"))

    def test_extension_defaults_colors_and_safe_argument_arrays(self):
        self.browser.find_next("needle")
        node, conf = self.browser.selected, self.browser.config
        self.assertEqual(default_opener(node, conf), "vscodium")
        self.assertEqual(display_color(node, conf), "#e5c07b")
        conf.openers["test"] = {"label": "Test", "command": ["test-editor", "--file={path}"], "terminal": True}
        path = self.root / "weird ; $(command) file.py"
        with patch("filex.opening.shutil.which", return_value="/bin/true"):
            command, terminal = prepare_open(path, conf, "test")
            self.assertEqual(command, ["test-editor", "--file=" + str(path)])
            self.assertTrue(terminal)
            with patch("filex.opening.subprocess.Popen") as process:
                launch_open(path, conf, "test", gui=True)
                self.assertEqual(process.call_args.args[0], ["/bin/true", "-e", *command])
                self.assertNotIn("shell", process.call_args.kwargs)
        self.assertEqual(opener_choices(node, conf)[0][0], "vscodium")

    def test_custom_conf_ctrl_bindings_programs_and_compound_extensions(self):
        path = self.root / "settings.conf"
        path.write_text('''[keys]
dashboard = "Ctrl+d"
[openers.custom]
command = ["custom-app", "{path}"]
terminal = false
[open_defaults]
".tar.gz" = "custom"
[extension_colors]
".tar.gz" = "#123456"
''')
        conf = load_config(path)
        self.assertEqual(key_map(conf)["\x04"], "dashboard")
        from filex.core import Node
        node = Node(self.root / "PACKAGE.TAR.GZ")
        self.assertEqual(default_opener(node, conf), "custom")
        self.assertEqual(display_color(node, conf), "#123456")
        for content in ('[open_defaults]\n".py"="missing"', '[extension_colors]\n".py"="blue"', '[openers.bad]\ncommand="echo {path}"', '[openers.bad]\ncommand=["echo"]', '[keys]\ncopy="Ctrl+f"'):
            path.write_text(content)
            with self.assertRaises(ValueError):
                load_config(path)
        path.write_text('[keys]\nquit="q"')
        self.assertEqual(load_config(path).keys["quit"], "Q")

    def test_siblings_and_legacy_config_migration(self):
        self.browser.expand_all()
        self.browser.right()
        left = self.browser.selected
        self.browser.step(1)
        self.assertEqual(self.browser.selected.path.name, "right")
        self.browser.step(-1)
        self.assertIs(self.browser.selected, left)
        self.browser.right()
        deep = self.browser.selected
        self.browser.step(1)
        self.assertIs(self.browser.selected, deep)
        self.browser.back()
        self.assertIs(self.browser.selected, left)
        self.assertTrue(left.expanded)
        path = self.root / "settings.conf"
        path.write_text('[layout]\norientation="vertical"\n[keys]\nparent="u"\ntoggle_tree="v"\nrename="R"\nrefresh="r"\n[tmux]\nopen_in_right_pane=false\n')
        conf = load_config(path)
        self.assertEqual(key_map(conf)["left"], "parent")
        self.assertNotIn("u", key_map(conf))
        self.assertFalse(conf.open_in_right_pane)
        self.browser.config = conf
        from filex.layout import Layout, plain_tree
        layout = Layout(self.browser)
        parent = layout.by_node[self.browser.root]
        children = [layout.by_node[child] for child in self.browser.root.children]
        self.assertEqual(parent.y, children[0].y)
        self.assertGreater(children[0].x, parent.x)
        self.assertNotIn("v", key_map(conf))
        self.assertFalse(hasattr(conf, "orientation"))
        self.assertEqual(key_map(conf)["r"], "rename")
        self.assertEqual(key_map(conf)["R"], "rename")
        self.assertEqual(key_map(conf)["\x12"], "refresh")
        self.assertEqual(key_map(conf)["ctrl+alt+c"], "copy_path")



if __name__ == "__main__":
    unittest.main()
