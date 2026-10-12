import os
from pathlib import Path
import tempfile
import unittest
from filex.config import Config, load_config, key_map, extension_rule
from filex.core import Browser
from filex.details import metadata, metadata_text, dashboard_text


class LinkTests(unittest.TestCase):
    def test_symlinks_hard_links_bounds_and_directory_counts(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root = base / "root"
            root.mkdir()
            path = root / "very_long_中文_filename_that_is_clipped.txt"
            path.write_text("first\nsecond\nthird\n")
            hidden = root / ".hard-peer"
            os.link(path, hidden)
            os.link(path, base / "outside-root")
            (root / "soft").symlink_to(path.name)
            (root / "broken").symlink_to("missing")
            (root / "loop").symlink_to(root, target_is_directory=True)
            data = metadata(path, root, Config())
            self.assertEqual(data["link_count"], 3)
            self.assertEqual(data["hard_links"], [str(hidden)])
            self.assertIn("状态: 有效", metadata_text(metadata(root / "soft", root)))
            self.assertIn("状态: 断开", metadata_text(metadata(root / "broken", root)))
            self.assertFalse(metadata(root, root)["hard_linked"])
            config = Config(scan_limit=1)
            bounded = metadata(path, root, config)
            self.assertLessEqual(bounded["link_scanned"], 1)
            self.assertTrue(bounded["link_limited"])
            browser = Browser(root, Config())
            node = next(node for node in browser.root.children if node.path == path)
            self.assertTrue(node.display_label(16).endswith("H×3"))
            self.assertNotIn("H×", browser.root.label)

    def test_configurable_head_never_previews_a_symlink_or_binary(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "source.txt"
            path.write_text("HEAD_ONE\nHEAD_TWO\nOMITTED_THREE\n")
            conf = Config(preview_lines=2)
            text = dashboard_text(path, root, conf)
            self.assertIn("HEAD_ONE\nHEAD_TWO", text)
            self.assertNotIn("OMITTED_THREE", text)
            self.assertIn("Modified", text)
            self.assertIn("Size", text)
            conf.preview_lines = 0
            self.assertNotIn("HEAD_ONE", dashboard_text(path, root, conf))
            (root / "soft").symlink_to(path.name)
            conf.preview_lines = 2
            self.assertNotIn("HEAD_ONE", dashboard_text(root / "soft", root, conf))
            path.write_bytes(b"\x00BINARY_CONTENT")
            self.assertNotIn("BINARY_CONTENT", dashboard_text(path, root, conf))

    def test_new_configuration_keys_and_runners(self):
        with tempfile.TemporaryDirectory() as directory:
            config = Path(directory) / "settings.conf"
            config.write_text('[keys]\nnew_file="Ctrl+n"\nnew_folder="Ctrl+Shift+n"\n[preview]\npreview_lines=2\n[links]\nscan_limit=40\nmax_results=3\n')
            conf = load_config(config)
            self.assertEqual(key_map(conf)["\x0e"], "new_file")
            self.assertEqual(key_map(conf)["ctrl+shift+n"], "new_folder")
            self.assertEqual(conf.preview_lines, 2)
            self.assertEqual(key_map(conf)["ctrl+f5"], "refresh")
            self.assertEqual(conf.openers["rscript"]["command"], ["Rscript", "{path}"])
            self.assertEqual(conf.openers["bash"]["command"], ["bash", "--", "{path}"])
            for name in ("source.js", "source.MJS"):
                self.assertEqual(extension_rule(Path(name), conf.extension_colors), "#FFD700")
                self.assertEqual(extension_rule(Path(name), conf.open_defaults), "system")
            config.write_text('[keys]\nrefresh="Ctrl+F5"\n')
            self.assertEqual(key_map(load_config(config))["ctrl+f5"], "refresh")
            config.write_text('[extension_colors]\n".js"="#ffd700"\n')
            self.assertEqual(load_config(config).extension_colors[".js"], "#ffd700")
            for invalid in ['[preview]\npreview_lines=-1', '[links]\nmax_results=0', '[keys]\ncopy="Ctrl+Shift+n"', '[keys]\ncopy="Ctrl+F5"', '[extension_colors]\n".js"="FFD700"']:
                config.write_text(invalid)
                with self.assertRaises(ValueError):
                    load_config(config)

    def test_symlink_segment_and_native_indigo_bold(self):
        import fcntl, pty, select, struct, subprocess, termios, time
        from filex.layout import Layout
        root_repo = Path(__file__).resolve().parents[1]
        with tempfile.TemporaryDirectory(prefix="filex-link-style-") as directory:
            root = Path(directory)
            (root / "b.txt").write_text("target")
            (root / "a.js").symlink_to("b.txt")
            (root / "broken").symlink_to("missing")
            browser = Browser(root, Config())
            item = next(item for item in Layout(browser).placements if item.node.path.name == "a.js")
            self.assertEqual(item.label, "· a.js------b.txt")
            self.assertEqual(item.label[item.link_start:], "------b.txt")
            config = root / ".settings.conf"
            config.write_text('[input_method]\nauto_english=false\n[folder_colors]\nnode_modules="#112233"\n".git"="#aabbcc"\n')
            master, slave = pty.openpty()
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 120, 0, 0))
            env = {k:v for k,v in os.environ.items() if not k.startswith("KONSOLE_DBUS_") and k not in {"TMUX", "TMUX_PANE", "NO_COLOR"}}
            env["TERM"] = "xterm-256color"
            process = subprocess.Popen([str(root_repo / "filex.sh"), str(root), "--config", str(config)], stdin=slave, stdout=slave, stderr=slave, env=env)
            os.close(slave)
            def capture():
                output = bytearray()
                end = time.monotonic() + 3
                while time.monotonic() < end:
                    if select.select([master], [], [], .02)[0]:
                        try: output.extend(os.read(master, 65536))
                        except OSError: break
                    if "↑↓".encode() in output: break
                return bytes(output)
            try:
                output = capture()
                self.assertIn(b"\x1b[38;2;75;0;130m\x1b[1m------b.txt", output)
                self.assertIn(b"\x1b[38;2;255;215;0m", output)
                os.write(master, b"\x1b[C")
                selected = capture()
                self.assertIn(b"\x1b[38;2;75;0;130m\x1b[1m------b.txt", selected)
                (root / "node_modules").mkdir()
                os.write(master, b"\x1b[15;5~")
                refreshed = capture()
                self.assertIn(b"\x1b[38;2;17;34;51m" + "▸ node_modules".encode(), refreshed)
                (root / ".git").mkdir()
                os.write(master, b".")
                hidden = capture()
                self.assertIn(b"\x1b[38;2;170;187;204m" + "▸ .git".encode(), hidden)
                os.write(master, b"Q")
                self.assertEqual(process.wait(timeout=3), 0)
            finally:
                if process.poll() is None: process.terminate(); process.wait(timeout=3)
                os.close(master)
