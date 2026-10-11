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
            env = {**os.environ}
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

            try:
                source = tmux("-f", "/dev/null", "new-session", "-d", "-s", "fixture", "-x", "180", "-y", "36",
                              "-c", str(root), "-P", "-F", "#{pane_id}",
                              "/usr/bin/bash", "--noprofile", "--rcfile", str(rc), "-i")
                right = tmux("split-window", "-h", "-t", source, "-c", str(right_cwd), "-P", "-F", "#{pane_id}",
                             "/usr/bin/bash", "--noprofile", "--rcfile", str(rc), "-i")
                wait(lambda: "filex-test$" in screen(right))
                send(source, "filex --config " + shlex.quote(str(config)))
                tmux("send-keys", "-t", source, "Enter")
                wait(lambda: screen(source).splitlines()[0].strip() == "filex")
                self.assertEqual(tmux("display-message", "-p", "-t", source, "#{pane_current_path}"), str(root))
                # Root up/down stay at root; enter a sibling level only with Right.
                tmux("send-keys", "-t", source, "Down", "Right")
                time.sleep(.1)
                send(source, "E")
                time.sleep(.1)
                tmux("send-keys", "-t", source, "Down")
                time.sleep(.1)
                self.assertIn(str(root / "b-folder"), screen(source))
                tmux("send-keys", "-t", source, "Up", "Right")
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
                send(source, "Q")
                wait(lambda: tmux("display-message", "-p", "-t", source, "#{pane_current_command}") == "bash")
            finally:
                subprocess.run(["tmux", "-L", socket, "kill-server"], env=env, capture_output=True)


if __name__ == "__main__":
    unittest.main()
