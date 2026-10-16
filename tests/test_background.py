"""PTY coverage for explicit detached folder commands."""
from pathlib import Path
import tempfile
import time
import unittest
from test_runtime_fixes import ROOT, Terminal


class BackgroundTests(unittest.TestCase):
    def test_folder_command_survives_exit_with_log_and_correct_cwd(self):
        with tempfile.TemporaryDirectory(prefix='filex-background-') as d:
            root = Path(d)
            folder = root/"work ' 中文"; folder.mkdir()
            marker = folder/'completed.txt'
            state = root/'state'
            config = root/'.settings.conf'
            config.write_text('[input_method]\nauto_english=false\n[terminal]\nmode="tty"\ncharset="ascii"\n')
            terminal = Terminal([str(ROOT/'filex.sh'), str(root), '--config', str(config)], root,
                                {'XDG_STATE_HOME': str(state), 'SHELL': '/bin/sh'})
            try:
                terminal.until(lambda: b'Space:mark' in terminal.output)
                terminal.send('\x06'+folder.name+'\r!')
                terminal.send('sleep 1; printf "%s" "$PWD" > completed.txt; echo logged-output; echo logged-error >&2\r')
                terminal.until(lambda: b'Started PID ' in terminal.output)
                self.assertFalse(marker.exists())
                terminal.send('Q')
                self.assertEqual(terminal.process.wait(timeout=5), 0)
                end = time.monotonic() + 5
                while not marker.exists() and time.monotonic() < end:
                    time.sleep(.02)
                self.assertEqual(marker.read_text(), str(folder))
                logs = list((state/'filex/jobs').glob('*.log'))
                self.assertEqual(len(logs), 1)
                text = logs[0].read_text()
                self.assertIn('\nlogged-output\n', text)
                self.assertIn('logged-error\n', text.split('\n\n', 1)[1])
            finally:
                terminal.close()

    def test_cancel_and_file_selection_do_not_launch(self):
        with tempfile.TemporaryDirectory(prefix='filex-background-cancel-') as d:
            root = Path(d); (root/'a.txt').write_text('file')
            state = root/'state'
            config = root/'.settings.conf'
            config.write_text('[input_method]\nauto_english=false\n[terminal]\nmode="tty"\ncharset="ascii"\n[keys]\nrun_background="B"\n')
            terminal = Terminal([str(ROOT/'filex.sh'), str(root), '--config', str(config)], root,
                                {'XDG_STATE_HOME': str(state), 'SHELL': '/bin/sh'})
            try:
                terminal.until(lambda: b'Space:mark' in terminal.output)
                terminal.send('Btouch should-not-exist')
                terminal.send('\x1b')
                terminal.send('\x06a.txt\rB')
                terminal.until(lambda: b'Select a folder to run' in terminal.output)
                self.assertFalse((root/'should-not-exist').exists())
                self.assertFalse(state.exists())
                terminal.send('Q')
                self.assertEqual(terminal.process.wait(timeout=5), 0)
            finally:
                terminal.close()
