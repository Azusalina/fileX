"""Exercise filex's adapter with the adjacent real toolkit and a real browser."""
from pathlib import Path
import shutil
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]


class MarkdownTests(unittest.TestCase):
    @unittest.skipUnless(shutil.which("node") and (ROOT.parent / "toolkit/node_modules").is_dir(),
                         "requires adjacent toolkit dependencies")
    def test_rust_tui_opens_markdown_and_copies_exact_path(self):
        import fcntl
        import json
        import os
        import pty
        import select
        import signal
        import struct
        import sys
        import tempfile
        import termios
        import time
        import urllib.request
        import http.cookiejar

        with tempfile.TemporaryDirectory(prefix="filex-md-open-") as directory:
            root = Path(directory)
            target = root / "notes 空格 ' ; {toolkit}.md"
            target.write_text("# Rust TUI Markdown\n\n**Rendered**\n")
            helpers = root / ".bin"
            helpers.mkdir()
            clipboard = root / ".clipboard"
            browser_record = root / ".browser.json"
            clipboard_daemon = root / ".clipboard-daemon"
            for name, code in {
                "wl-copy": f"from pathlib import Path; import sys,os,time; Path({str(clipboard)!r}).write_bytes(sys.stdin.buffer.read()); child=os.fork(); Path({str(clipboard_daemon)!r}).write_text(str(os.getpid())) if child==0 else None; time.sleep(12) if child==0 else None; os._exit(0)",
                "firefox": f"from pathlib import Path; import json,os,sys; Path({str(browser_record)!r}).write_text(json.dumps([os.getppid(),sys.argv[1:]]))",
            }.items():
                executable = helpers / name
                executable.write_text(f"#!{sys.executable}\n{code}\n")
                executable.chmod(0o700)
            config = root / ".settings.conf"
            config.write_text("[input_method]\nauto_english=false\n")
            master, slave = pty.openpty()
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 26, 120, 0, 0))
            env = {**{k: v for k, v in os.environ.items() if k not in {"TMUX", "TMUX_PANE"} and not k.startswith("KONSOLE_DBUS_")},
                   "TERM": "xterm-256color", "PATH": str(helpers) + os.pathsep + os.environ["PATH"]}
            process = subprocess.Popen([str(ROOT / "filex.sh"), str(root), "--config", str(config)],
                                       stdin=slave, stdout=slave, stderr=slave, env=env)
            os.close(slave)
            server_pid = None
            output = bytearray()
            def drain(seconds):
                end = time.monotonic() + seconds
                while time.monotonic() < end:
                    if select.select([master], [], [], .02)[0]:
                        try:
                            output.extend(os.read(master, 65536))
                        except OSError:
                            break
            try:
                drain(.3)
                os.write(master, b"\x1b[Co")
                end = time.monotonic() + 8
                while not browser_record.exists() and time.monotonic() < end:
                    drain(.05)
                self.assertTrue(browser_record.exists(), output.decode(errors="replace")[-2000:])
                server_pid, args = json.loads(browser_record.read_text())
                self.assertEqual(clipboard.read_text(), str(target))
                self.assertEqual(args[0], "--new-tab")
                self.assertRegex(args[1], r"^http://127\.0\.0\.1:\d+/$")
                client = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))
                client.open(args[1], timeout=5).close()
                request = urllib.request.Request(args[1] + "api/views", data=b"{}",
                                                 headers={"Content-Type": "application/json"})
                with client.open(request, timeout=5) as response:
                    state = json.load(response)["state"]
                self.assertEqual(state["filename"], str(target))
                self.assertIn("<strong>Rendered</strong>", state["html"])
                self.assertEqual(target.read_text(), "# Rust TUI Markdown\n\n**Rendered**\n")
                os.write(master, b"Q")
                drain(.1)
                self.assertEqual(process.wait(timeout=3), 0)
            finally:
                if process.poll() is None:
                    process.terminate()
                    process.wait(timeout=3)
                os.close(master)
                if clipboard_daemon.exists():
                    try: os.kill(int(clipboard_daemon.read_text()), signal.SIGTERM)
                    except ProcessLookupError: pass
                if server_pid and Path(f"/proc/{server_pid}/cmdline").exists():
                    if b"open_markdown.mjs" in Path(f"/proc/{server_pid}/cmdline").read_bytes():
                        os.kill(server_pid, signal.SIGTERM)

    @unittest.skipUnless(shutil.which("node") and shutil.which("chromium") and
                         (ROOT.parent / "toolkit/node_modules/playwright-core").is_dir(),
                         "requires adjacent toolkit dependencies and Chromium")
    def test_actual_toolkit_render_watch_and_independent_pages(self):
        result = subprocess.run(["node", str(ROOT / "tests/markdown_browser.mjs")],
                                capture_output=True, text=True, timeout=35)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
