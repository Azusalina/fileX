"""Overlapping real TUI instances, isolated from the live desktop through a DBus CLI fixture."""
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

ROOT = Path(__file__).resolve().parents[1]


class TerminalHostTests(unittest.TestCase):
    def test_last_instance_restores_only_the_original_konsole_action_state(self):
        for original in ("true", "false"):
            with self.subTest(original=original), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                state = root / ".action-state"
                state.write_text(original)
                helpers = root / ".bin"
                helpers.mkdir()
                runtime = root / ".runtime"
                runtime.mkdir(mode=0o700)
                helper = helpers / "qdbus6"
                helper.write_text(f'''#!{sys.executable}
import sys
from pathlib import Path
state = Path({str(state)!r})
args = sys.argv[2:]
if not args: print('/Windows/1')
elif args[-1].endswith('sessionList'): print('1')
elif 'org.kde.KMainWindow.actionIsEnabled' in args: print(state.read_text())
elif 'org.kde.KMainWindow.disableAction' in args: state.write_text('false'); print('true')
elif 'org.kde.KMainWindow.enableAction' in args: state.write_text('true'); print('true')
else: sys.exit(1)
''')
                helper.chmod(0o700)
                config = root / ".settings.conf"
                config.write_text('[input_method]\nauto_english=false\n[terminal]\ntmux_extended_keys=false\n')
                env = {k: v for k, v in os.environ.items() if k not in {"TMUX", "TMUX_PANE"} and not k.startswith("KONSOLE_DBUS_")}
                env.update(TERM="xterm-256color", PATH=str(helpers) + os.pathsep + env["PATH"],
                           XDG_RUNTIME_DIR=str(runtime), KONSOLE_DBUS_SERVICE="filex-fixture", KONSOLE_DBUS_SESSION="/Sessions/1")
                children = []
                def start():
                    master, slave = pty.openpty()
                    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 26, 110, 0, 0))
                    process = subprocess.Popen([str(ROOT / "filex.sh"), str(root), "--config", str(config)],
                                               stdin=slave, stdout=slave, stderr=slave, env=env)
                    os.close(slave)
                    children.append((process, master))
                    output = bytearray()
                    end = time.monotonic() + 5
                    while time.monotonic() < end and "↑↓".encode() not in output:
                        if select.select([master], [], [], .03)[0]:
                            output.extend(os.read(master, 65536))
                    self.assertIn("↑↓".encode(), output)
                    return process, master
                try:
                    first, first_fd = start()
                    self.assertEqual(state.read_text(), "false")
                    second, second_fd = start()
                    os.write(first_fd, b"Q")
                    self.assertEqual(first.wait(timeout=5), 0)
                    self.assertEqual(state.read_text(), "false")
                    os.write(second_fd, b"Q")
                    self.assertEqual(second.wait(timeout=5), 0)
                    self.assertEqual(state.read_text(), original)
                finally:
                    for process, master in children:
                        if process.poll() is None:
                            process.terminate()
                            process.wait(timeout=5)
                        os.close(master)
