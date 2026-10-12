"""Drive real curses through a pseudo-terminal; verify file operations on a fixture."""
import fcntl
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import sys
import tempfile
import termios
import time
import unittest


class TerminalTests(unittest.TestCase):
    def test_real_keyboard_file_operations_and_resize(self):
        # Keep the fixture on the normal home filesystem: gio refuses this host's /tmp mount.
        with tempfile.TemporaryDirectory(prefix=".trash-fixture-", dir=Path(__file__).resolve().parents[1]) as directory:
            root = Path(directory)
            data = root / "isolated-data"
            data.mkdir()
            victim = root / "delete-me.txt"
            victim.write_text("recoverable fixture")
            (root / "nested").mkdir()
            (root / "nested" / "sample.txt").write_text("sample")
            runner = root / "nested/script 空格.py"
            runner.write_text("from pathlib import Path\nPath('runner-created.txt').write_text('PYTHON-RAN')\nprint('PYTHON-RUNNER-RETURNED')\n")
            (root / "nested/runner.r").write_text('writeLines("RSCRIPT-RAN", "r-result.txt")\ncat("RSCRIPT-RETURNED\\n")\n')
            (root / "nested/runner.sh").write_text("printf '%s' BASH-RAN > shell-result.txt\nprintf '%s\\n' BASH-RETURNED\n")
            import json
            config = root / ".settings.conf"
            editor_code = 'import pathlib,sys; pathlib.Path(sys.argv[1]).write_text("edited"); print("EDITOR-RETURNED")'
            config.write_text('[openers.fixture]\nlabel="Fixture editor"\nterminal=true\ncommand=' + json.dumps([sys.executable, "-c", editor_code, "{path}"]) + '\n[open_defaults]\n".txt"="fixture"\n".py"="python"\n".r"="rscript"\n".sh"="bash"\n')
            helpers = root / ".bin"
            helpers.mkdir()
            clipboard = root / ".clipboard"
            im_state = root / ".im-state"
            im_state.write_text("pinyin")
            im_calls = root / ".im-calls"
            for name, code in {
                "wl-copy": f"import sys; from pathlib import Path; Path({str(clipboard)!r}).write_bytes(sys.stdin.buffer.read())",
                "fcitx5-remote": f"import sys; from pathlib import Path; state=Path({str(im_state)!r}); calls=Path({str(im_calls)!r}); calls.open('a').write(' '.join(sys.argv[1:])+'\\n'); print(state.read_text()) if '-n' in sys.argv else state.write_text(sys.argv[-1])",
            }.items():
                helper = helpers / name
                helper.write_text(f"#!{sys.executable}\n" + code + "\n")
                helper.chmod(0o700)
            master, slave = pty.openpty()
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 26, 110, 0, 0))
            process = subprocess.Popen([str(Path(__file__).resolve().parents[1] / "filex.sh"), str(root), "--config", str(config)],
                                       stdin=slave, stdout=slave, stderr=slave,
                                       cwd=Path(__file__).resolve().parents[1], env={**{k: v for k, v in os.environ.items() if k not in {"TMUX", "TMUX_PANE"} and not k.startswith("KONSOLE_DBUS_")}, "TERM": "xterm-256color", "XDG_DATA_HOME": str(data), "PATH": str(helpers) + os.pathsep + os.environ["PATH"]})
            os.close(slave)
            output = bytearray()
            def drain(timeout=.1):
                deadline = time.monotonic() + timeout
                while time.monotonic() < deadline:
                    readable, _, _ = select.select([master], [], [], max(0, deadline - time.monotonic()))
                    if readable:
                        try:
                            chunk = os.read(master, 65536)
                        except OSError:
                            break
                        if not chunk:
                            break
                        output.extend(chunk)
            def send(text):
                os.write(master, text.encode())
                drain(.12)
            def exists(name):
                deadline = time.monotonic() + 3
                while not (root / name).exists() and time.monotonic() < deadline:
                    drain(.05)
                self.assertTrue((root / name).exists(), output.decode(errors="replace")[-2000:])
            def copied_path(expected):
                deadline = time.monotonic() + 3
                while time.monotonic() < deadline:
                    if clipboard.exists() and clipboard.read_text() == str(expected):
                        return
                    drain(.05)
                self.fail("Clipboard did not contain expected path: " + output.decode(errors="replace")[-2000:])
            try:
                ready = time.monotonic() + 5
                while "↑↓".encode() not in output and time.monotonic() < ready:
                    drain(.05)
                self.assertIn("↑↓".encode(), output)
                (root / "ctrl-f5-probe.txt").write_text("externally created")
                self.assertNotIn(b"ctrl-f5-probe.txt", output)
                send("\x1b[15;5~")
                refreshed = time.monotonic() + 3
                while b"ctrl-f5-probe.txt" not in output and time.monotonic() < refreshed:
                    drain(.05)
                self.assertIn(b"ctrl-f5-probe.txt", output)
                send("\x0e中文.txt\r")
                exists("中文.txt")
                self.assertEqual(im_state.read_text(), "keyboard-us")
                send("r\x15first-name.txt\r")
                exists("first-name.txt")
                send("R\x15renamed.txt\r")
                exists("renamed.txt")
                self.assertFalse((root / "中文.txt").exists())
                send("ccopied.txt\r")
                exists("copied.txt")
                send("mmoved.txt\r")
                exists("moved.txt")
                self.assertFalse((root / "copied.txt").exists())
                send("\x1b[110;6unew-folder\r")
                exists("new-folder")
                send("\x1b[D")
                send("\x1b[78;6uanother-folder\r")
                exists("another-folder")
                exists("new-folder")
                send("?q")
                send("p\x1b")
                send("q")
                self.assertIn(b"Created", output)
                self.assertIn(b"Modified", output)
                send("]")
                self.assertIn("信息面板宽度: 40 列".encode(), output)
                send("[")
                self.assertIn("信息面板宽度: 36 列".encode(), output)
                self.assertIsNone(process.poll())
                send("q")
                send("\x06sample\r")
                self.assertIn(b"sample.txt", output)
                send("\r\r")
                self.assertEqual((root / "nested/sample.txt").read_text(), "edited")
                self.assertIn(b"EDITOR-RETURNED", output)
                self.assertIsNone(process.poll())
                deadline = time.monotonic() + 3
                while "↑↓".encode() not in output[output.rfind(b"EDITOR-RETURNED"):] and time.monotonic() < deadline:
                    drain(.05)
                self.assertIn("↑↓".encode(), output[output.rfind(b"EDITOR-RETURNED"):])
                send("\x1b\x03")
                self.assertIsNone(process.poll())
                copied_path(root / "nested/sample.txt")
                self.assertIn("已复制绝对路径".encode(), output)
                send("\x1b[D")
                send("\x1b[99;7u")  # enhanced Ctrl+Alt+C (CSI-u)
                copied_path(root / "nested")
                send("v")
                self.assertNotIn("纵向树".encode(), output)
                send("\x06\x15script 空格\r")
                send("o")
                self.assertEqual((root / "nested/runner-created.txt").read_text(), "PYTHON-RAN")
                self.assertIn(b"PYTHON-RUNNER-RETURNED", output)
                self.assertIn("按 Enter 返回 filex".encode(), output)
                send("\r")
                self.assertIsNone(process.poll())
                for query, result_name, content, marker in [("runner.r", "r-result.txt", "RSCRIPT-RAN\n", b"RSCRIPT-RETURNED"),
                                                            ("runner.sh", "shell-result.txt", "BASH-RAN", b"BASH-RETURNED")]:
                    send("\x06\x15" + query + "\r")
                    send("o")
                    end = time.monotonic() + 5
                    result = root / "nested" / result_name
                    while not result.exists() and time.monotonic() < end:
                        drain(.05)
                    self.assertEqual(result.read_text(), content)
                    drain(.2)
                    self.assertIn(marker, output)
                    send("\r")
                send("\r\x1b")
                self.assertIn("打开方式".encode(), output)
                send("\x06\x15delete-me\r")
                send("\x1b[3~")
                self.assertIn("再次按 Enter 确认".encode(), output)
                send("\x1b")
                self.assertTrue(victim.exists())
                send("\x1b[3~NO")
                self.assertTrue(victim.exists())
                send("\x1b")
                send("\x1b[3~\r")
                deadline = time.monotonic() + 3
                while victim.exists() and time.monotonic() < deadline:
                    drain(.05)
                self.assertFalse(victim.exists(), output.decode(errors="replace")[-2000:])
                self.assertEqual((data / "Trash/files/delete-me.txt").read_text(), "recoverable fixture")
                self.assertTrue((data / "Trash/info/delete-me.txt.trashinfo").exists())
                fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 10, 40, 0, 0))
                import signal
                process.send_signal(signal.SIGWINCH)
                drain(.15)
                send("LH\x12\x1b[15~")
                send("Q")
                self.assertEqual(process.wait(timeout=3), 0)
                self.assertEqual(im_state.read_text(), "pinyin")
                self.assertIn("--check -s keyboard-us", im_calls.read_text())
                self.assertIn("--check -s pinyin", im_calls.read_text())
                self.assertNotIn(b"Traceback", output)
                self.assertIn(b"filex", output)
                self.assertNotIn("Rust · 横向目录树".encode(), output)
            finally:
                if process.poll() is None:
                    process.terminate()
                    process.wait(timeout=3)
                os.close(master)


if __name__ == "__main__":
    unittest.main()
