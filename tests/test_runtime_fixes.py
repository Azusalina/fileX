import fcntl
import json
import os
from pathlib import Path
import pty
import select
import shlex
import struct
import subprocess
import sys
import tempfile
import termios
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]


class Terminal:
    def __init__(self, command, root, extra=None):
        self.master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 28, 150, 0, 0))
        env = {k:v for k,v in os.environ.items() if not k.startswith('KONSOLE_DBUS_') and k not in {'TMUX','TMUX_PANE','FILEX_CWD_FILE','NO_COLOR'}}
        env.update(TERM='xterm-256color', **(extra or {}))
        self.process = subprocess.Popen(command, cwd=root, env=env, stdin=slave, stdout=slave, stderr=slave)
        os.close(slave)
        self.output = bytearray()

    def drain(self, seconds=.05):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            if select.select([self.master], [], [], .01)[0]:
                try: self.output.extend(os.read(self.master, 65536))
                except OSError: break

    def until(self, condition, seconds=8):
        end = time.monotonic() + seconds
        while not condition() and time.monotonic() < end: self.drain()
        if not condition(): raise AssertionError(self.output.decode(errors='replace')[-2000:])

    def send(self, text):
        os.write(self.master, text.encode()); self.drain(.1)

    def close(self):
        if self.process.poll() is None: self.process.terminate(); self.process.wait(timeout=5)
        os.close(self.master)


class RuntimeFixTests(unittest.TestCase):
    def test_direct_entry_x_and_repeated_exit_without_manual_source(self):
        with tempfile.TemporaryDirectory(prefix='filex-direct-x-') as d:
            root=Path(d); first=root/"a ' ; $(literal)\n目录"; second=first/'b 中文'; second.mkdir(parents=True)
            rc=root/'.bashrc'; rc.write_text("PS1='FILEX-DIRECT$ '\n")
            subprocess.run([sys.executable,str(ROOT/'scripts/install_alias.py'),'--rc',str(rc)],check=True,capture_output=True)
            shell=root/'.shell'; shell.write_text('#!/bin/bash\nexec /bin/bash --noprofile --rcfile "$FILEX_TEST_RC" -i\n'); shell.chmod(0o700)
            config=root/'.settings.conf'; config.write_text('[input_method]\nauto_english=false\n')
            terminal=Terminal([str(ROOT/'filex.sh'),str(root),'--config',str(config)],root,{'SHELL':str(shell),'FILEX_TEST_RC':str(rc)})
            try:
                terminal.until(lambda: '↑↓'.encode() in terminal.output)
                terminal.send('\x1b[Cx')
                terminal.until(lambda: b'FILEX-DIRECT$' in terminal.output)
                record=root/'.cwd'; terminal.send("printf '%s' \"$PWD\" > "+shlex.quote(str(record))+'\r')
                terminal.until(record.exists); self.assertEqual(record.read_text(),str(first))
                terminal.send('filex --config '+shlex.quote(str(config))+'\r')
                terminal.drain(.3); terminal.send('\x1b[Cx')
                terminal.send("printf '%s' \"$PWD\" > "+shlex.quote(str(record))+'\r')
                terminal.until(lambda: record.read_text()==str(second))
                terminal.send('exit\r'); self.assertEqual(terminal.process.wait(timeout=5),0)
            finally: terminal.close()

    def test_markdown_failure_visible_and_terminal_remains_responsive(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d); (root/'note.md').write_text('# test')
            config=root/'.settings.conf'; config.write_text('[input_method]\nauto_english=false\n[openers.markdown]\ncopy_path=false\ncommand='+json.dumps(['node',str(ROOT/'scripts/open_markdown.mjs'),'--toolkit',str(root/'missing'),'--','{path}'])+'\n')
            terminal=Terminal([str(ROOT/'filex.sh'),str(root),'--config',str(config)],root)
            try:
                terminal.until(lambda: '↑↓'.encode() in terminal.output)
                terminal.send('\x1b[Co')
                terminal.until(lambda: 'Markdown 启动失败'.encode() in terminal.output)
                terminal.send('\x0eafter-failure.txt\r')
                terminal.until(lambda: (root/'after-failure.txt').exists())
                terminal.send('Q'); self.assertEqual(terminal.process.wait(timeout=5),0)
            finally: terminal.close()

    def test_focus_return_keeps_prompt_and_keyboard_usable(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d); config=root/'.settings.conf';config.write_text('[input_method]\nauto_english=false\n')
            terminal=Terminal([str(ROOT/'filex.sh'),str(root),'--config',str(config)],root)
            try:
                terminal.until(lambda: '↑↓'.encode() in terminal.output)
                terminal.send('\x0ekeep_\x1b[O\x1b[Itext.txt\r')
                terminal.until(lambda: (root/'keep_text.txt').exists())
                for _ in range(8): terminal.send('\x1b[O\x1b[I')
                self.assertGreaterEqual(terminal.output.count(b'\x1b[=1u'),9)
                terminal.send('\x1b[78;6ufolder_after_focus\r')
                terminal.until(lambda: (root/'folder_after_focus').is_dir())
                terminal.send('Q');self.assertEqual(terminal.process.wait(timeout=5),0)
            finally: terminal.close()
