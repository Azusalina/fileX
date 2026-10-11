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
            import json
            config = root / ".settings.conf"
            editor_code = 'import pathlib,sys; pathlib.Path(sys.argv[1]).write_text("edited"); print("EDITOR-RETURNED")'
            config.write_text('[openers.fixture]\nlabel="Fixture editor"\nterminal=true\ncommand=' + json.dumps([sys.executable, "-c", editor_code, "{path}"]) + '\n[open_defaults]\n".txt"="fixture"\n')
            master, slave = pty.openpty()
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 26, 110, 0, 0))
            process = subprocess.Popen([str(Path(__file__).resolve().parents[1] / "filex.sh"), str(root), "--config", str(config)],
                                       stdin=slave, stdout=slave, stderr=slave,
                                       cwd=Path(__file__).resolve().parents[1], env={**{k: v for k, v in os.environ.items() if k not in {"TMUX", "TMUX_PANE"}}, "TERM": "xterm-256color", "XDG_DATA_HOME": str(data)})
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
            try:
                drain(.3)
                send("a中文.txt\r")
                exists("中文.txt")
                send("R\x15renamed.txt\r")
                exists("renamed.txt")
                self.assertFalse((root / "中文.txt").exists())
                send("ccopied.txt\r")
                exists("copied.txt")
                send("mmoved.txt\r")
                exists("moved.txt")
                self.assertFalse((root / "copied.txt").exists())
                send("Anew-folder\r")
                exists("new-folder")
                send("?q")
                send("p\x1b")
                send("q")
                self.assertIn(b"Created", output)
                self.assertIn(b"Modified", output)
                self.assertIsNone(process.poll())
                send("q")
                send("\x06sample\r")
                self.assertIn(b"sample.txt", output)
                send("\r\r")
                self.assertEqual((root / "nested/sample.txt").read_text(), "edited")
                self.assertIn(b"EDITOR-RETURNED", output)
                self.assertIsNone(process.poll())
                send("\x1b[D")
                send("v")
                self.assertIn("纵向树".encode(), output)
                send("\r\x1b")
                self.assertIn("打开方式".encode(), output)
                send("\x06\x15delete-me\r")
                send("\x1b[3~")
                self.assertIn("输入 YES".encode(), output)
                send("\x1b")
                self.assertTrue(victim.exists())
                send("\x1b[3~NO\r")
                self.assertTrue(victim.exists())
                send("\x1b[3~YES\r")
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
                send("LHr")
                send("Q")
                self.assertEqual(process.wait(timeout=3), 0)
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
