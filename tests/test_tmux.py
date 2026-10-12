"""Real routing and navigation in an isolated tmux server; never uses live sessions."""
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
import uuid

ROOT = Path(__file__).resolve().parents[1]


@unittest.skipUnless(shutil.which("tmux"), "requires tmux")
class TmuxTests(unittest.TestCase):
    def test_alias_cwd_arrows_right_idle_busy_draft_and_resize(self):
        with tempfile.TemporaryDirectory(prefix="filex-tmux-") as folder:
            root = Path(folder)
            (root / "a-folder").mkdir()
            (root / "a-folder/child.txt").write_text("child")
            (root / "b-folder").mkdir()
            (root / "b-folder/b-child.txt").write_text("other parent")
            right_cwd = root / ".right-work"
            right_cwd.mkdir()
            path = root / "literal ' ; $(touch INJECTED).txt"
            path.write_text("hello")
            records = root / "records.jsonl"
            editor = ("import json,os,sys,time; "
                      f"f=open({str(records)!r},'a'); "
                      "f.write(json.dumps([os.environ.get('TMUX_PANE'),sys.argv[1],os.getcwd()])+'\\n'); "
                      "f.close(); print('FILEX-EDITOR'); time.sleep(1.2)")
            config = root / ".settings.conf"
            config.write_text('[openers.fixture]\nlabel="Fixture"\nterminal=true\ncommand=' +
                              json.dumps([sys.executable, "-c", editor, "{path}"]) +
                              '\n[input_method]\nauto_english=false\n[open_defaults]\n".txt"="fixture"\n')
            rc = root / ".bashrc"
            helpers = root / ".bin"
            helpers.mkdir()
            clipboard = root / ".clipboard"
            helper = helpers / "wl-copy"
            helper.write_text(f"#!{sys.executable}\nimport sys; from pathlib import Path; Path({str(clipboard)!r}).write_bytes(sys.stdin.buffer.read())\n")
            helper.chmod(0o700)
            rc.write_text("PS1='filex-test$ '\nexport PATH=" + shlex.quote(str(helpers)) + ":$PATH\n")
            subprocess.run([sys.executable, str(ROOT / "scripts/install_alias.py"), "--rc", str(rc)],
                           check=True, capture_output=True)
            socket = "filex-test-" + uuid.uuid4().hex
            env = {k: v for k, v in os.environ.items() if not k.startswith("KONSOLE_DBUS_")}
            env.pop("TMUX", None)
            env.pop("TMUX_PANE", None)

            def tmux(*args):
                return subprocess.run(["tmux", "-L", socket, *args], env=env,
                                      text=True, capture_output=True, check=True).stdout.rstrip("\n")

            def wait(predicate, timeout=5):
                deadline = time.monotonic() + timeout
                while time.monotonic() < deadline:
                    if predicate():
                        return
                    time.sleep(.03)
                self.fail("tmux condition timed out: " + tmux("capture-pane", "-p", "-t", source))

            def screen(pane):
                return tmux("capture-pane", "-p", "-t", pane)

            def send(pane, text):
                tmux("send-keys", "-t", pane, "-l", text)

            def rows():
                return [json.loads(line) for line in records.read_text().splitlines()] if records.exists() else []

            focus_client = None
            focus_fd = None
            stop_focus = None
            try:
                source = tmux("-f", "/dev/null", "new-session", "-d", "-s", "fixture", "-x", "180", "-y", "36",
                              "-c", str(root), "-P", "-F", "#{pane_id}",
                              "/usr/bin/bash", "--noprofile", "--rcfile", str(rc), "-i")
                right = tmux("split-window", "-h", "-t", source, "-c", str(right_cwd), "-P", "-F", "#{pane_id}",
                             "/usr/bin/bash", "--noprofile", "--rcfile", str(rc), "-i")
                wait(lambda: "filex-test$" in screen(right))
                original_keys = tmux("show-options", "-s", "-v", "extended-keys")
                original_focus = tmux("show-options", "-s", "-v", "focus-events")
                original_format = tmux("show-options", "-s", "-v", "extended-keys-format")
                send(source, "filex --config " + shlex.quote(str(config)))
                tmux("send-keys", "-t", source, "Enter")
                wait(lambda: screen(source).splitlines()[0].strip() == "filex")
                self.assertEqual(tmux("display-message", "-p", "-t", source, "#{pane_current_path}"), str(root))
                send(source, "v")
                wait(lambda: "左 ●" in screen(source))
                tmux("send-keys", "-t", source, "Tab")
                wait(lambda: "右 ●" in screen(source))
                tmux("send-keys", "-t", source, "Tab")
                send(source, "v")
                wait(lambda: screen(source).splitlines()[0].strip() == "filex")
                (root / "ctrl-f5-probe.txt").write_text("externally created")
                self.assertNotIn("ctrl-f5-probe.txt", screen(source))
                tmux("send-keys", "-t", source, "C-F5")
                wait(lambda: "ctrl-f5-probe.txt" in screen(source))
                tmux("send-keys", "-t", source, "C-n")
                send(source, "new-file.txt")
                tmux("send-keys", "-t", source, "Enter")
                wait(lambda: (root / "new-file.txt").exists())
                tmux("send-keys", "-t", source, "C-S-n")
                send(source, "new-folder")
                tmux("send-keys", "-t", source, "Enter")
                wait(lambda: (root / "new-folder").is_dir())
                # Return to root for sibling navigation checks.
                tmux("send-keys", "-t", source, "Left")
                self.assertEqual(tmux("show-options", "-s", "-v", "extended-keys-format"), "csi-u")
                self.assertEqual(tmux("show-options", "-s", "-v", "focus-events"), "on")
                import pty, fcntl, struct, termios, select, threading
                focus_fd, slave = pty.openpty()
                fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH",36,180,0,0))
                focus_client = subprocess.Popen(["tmux","-L",socket,"attach-session","-t","fixture"],env=env,stdin=slave,stdout=slave,stderr=slave)
                os.close(slave)
                stop_focus = threading.Event()
                def drain_focus():
                    while not stop_focus.is_set():
                        if select.select([focus_fd],[],[],.02)[0]:
                            try: os.read(focus_fd,65536)
                            except OSError: return
                threading.Thread(target=drain_focus,daemon=True).start()
                other_window = tmux("new-window","-d","-t","fixture","-P","-F","#{window_id}","sleep 120").strip()
                source_window = tmux("display-message","-p","-t",source,"#{window_id}").strip()
                for _ in range(5):
                    tmux("select-window","-t",other_window); time.sleep(.06)
                    tmux("select-window","-t",source_window); time.sleep(.06)
                # Root up/down stay at root; enter a sibling level only with Right.
                tmux("send-keys", "-t", source, "Down", "Right")
                time.sleep(.1)
                send(source, "E")
                time.sleep(.1)
                tmux("send-keys", "-t", source, "Down")
                time.sleep(.1)
                self.assertIn(str(root / "b-folder"), screen(source))
                send(source, "E")
                time.sleep(.1)
                tmux("send-keys", "-t", source, "Up", "Right")
                time.sleep(.1)
                self.assertIn(str(root / "a-folder/child.txt"), screen(source))
                tmux("send-keys", "-t", source, "Up")
                time.sleep(.1)
                self.assertIn(str(root / "b-folder/b-child.txt"), screen(source))
                tmux("send-keys", "-t", source, "Down")
                time.sleep(.1)
                self.assertIn(str(root / "a-folder/child.txt"), screen(source))
                tmux("send-keys", "-t", source, "Left")
                time.sleep(.1)
                self.assertIn("child.txt", screen(source))  # branch stays open
                tmux("send-keys", "-t", source, "C-f")
                send(source, "literal")
                tmux("send-keys", "-t", source, "Enter")
                time.sleep(.15)
                tmux("send-keys", "-t", source, "M-C-c")
                wait(lambda: clipboard.exists())
                self.assertEqual(clipboard.read_text(), str(path))
                send(source, "o")
                wait(lambda: len(rows()) == 1)
                self.assertEqual(rows()[0], [right, str(path), str(root)])
                self.assertEqual(tmux("display-message", "-p", "-t", right, "#{pane_active}"), "1")
                # While the right pane runs the editor, use the current terminal instead.
                send(source, "o")
                wait(lambda: len(rows()) == 2)
                self.assertEqual(rows()[1], [source, str(path), str(root)])
                wait(lambda: "filex-test$" in screen(right))
                time.sleep(1.3)
                self.assertEqual(tmux("display-message", "-p", "-t", right, "#{pane_current_path}"), str(right_cwd))
                # A typed but unsubmitted shell command is not an idle pane.
                draft = "echo FILEX-DRAFT"
                send(right, draft)
                wait(lambda: draft in screen(right))
                send(source, "o")
                wait(lambda: len(rows()) == 3)
                self.assertEqual(rows()[2], [source, str(path), str(root)])
                self.assertIn(draft, screen(right))
                self.assertFalse((root / "INJECTED").exists())
                time.sleep(1.3)
                tmux("resize-window", "-t", source, "-x", "110", "-y", "16")
                time.sleep(.1)
                tmux("send-keys", "-t", source, "C-f", "C-u")
                send(source, "a-folder")
                tmux("send-keys", "-t", source, "Enter")
                time.sleep(.15)
                send(source, "x")
                wait(lambda: tmux("display-message", "-p", "-t", source, "#{pane_current_command}") == "bash")
                self.assertEqual(tmux("display-message", "-p", "-t", source, "#{pane_current_path}"), str(root / "a-folder"))
                self.assertEqual(tmux("show-options", "-s", "-v", "focus-events"), original_focus)
                self.assertEqual(tmux("show-options", "-s", "-v", "extended-keys"), original_keys)
                self.assertEqual(tmux("show-options", "-s", "-v", "extended-keys-format"), original_format)
            finally:
                if stop_focus: stop_focus.set()
                if focus_client:
                    focus_client.terminate(); focus_client.wait(timeout=3)
                if focus_fd is not None: os.close(focus_fd)
                subprocess.run(["tmux", "-L", socket, "kill-server"], env=env, capture_output=True)


if __name__ == "__main__":
    unittest.main()
