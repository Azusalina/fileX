"""Interactive foreground commands use the selected folder and return to the TUI."""
import os
from pathlib import Path
import signal
import tempfile
import unittest
import zipfile
from test_runtime_fixes import ROOT, Terminal


class ForegroundTests(unittest.TestCase):
    def start(self, root, config):
        t=Terminal([str(ROOT/'filex.sh'),str(root),'--config',str(config)],root,{'SHELL':'/bin/sh'})
        self.addCleanup(t.close);t.until(lambda:b'Space:mark' in t.output);return t

    def test_command_output_interactive_input_unzip_and_failure_return(self):
        with tempfile.TemporaryDirectory(prefix='filex-foreground-') as d:
            base=Path(d);root=base/"folder ' 中文";root.mkdir();config=base/'settings.conf'
            config.write_text('[input_method]\nauto_english=false\n[terminal]\nmode="tty"\ncharset="ascii"\n')
            with zipfile.ZipFile(root/'sample.zip','w') as z:z.writestr('unpacked.txt','hello')
            t=self.start(root,config);t.send('@')
            t.send('printf "ENTER-VALUE:"; read value; printf "%s" "$value" > value.txt; pwd; echo COMMAND-STDERR >&2; unzip sample.zip\r')
            t.until(lambda:b'ENTER-VALUE:' in t.output)
            t.send('typed answer\r')
            t.until(lambda:b'Command exited (' in t.output)
            self.assertEqual((root/'value.txt').read_text(),'typed answer');self.assertEqual((root/'unpacked.txt').read_text(),'hello')
            self.assertIn(b'COMMAND-STDERR',t.output);self.assertIn(str(root).encode(),t.output)
            t.send('\r');t.send('@exit 7\r');t.until(lambda:b'exit status: 7' in t.output)
            t.send('\r');t.send('\x0eafter-command.txt\r');t.until(lambda:(root/'after-command.txt').exists())
            t.send('Q');self.assertEqual(t.process.wait(timeout=5),0)

    def test_ctrl_c_stops_foreground_child_without_exiting_filex(self):
        with tempfile.TemporaryDirectory(prefix='filex-foreground-signal-') as d:
            root=Path(d);config=root/'.settings.conf';config.write_text('[input_method]\nauto_english=false\n[terminal]\nmode="tty"\ncharset="ascii"\n')
            t=self.start(root,config);child_pid=None
            try:
                t.send('@echo $$ > child.pid; exec sleep 30\r')
                t.until(lambda:(root/'child.pid').exists());child_pid=int((root/'child.pid').read_text())
                os.kill(t.process.pid,signal.SIGINT);os.kill(child_pid,signal.SIGINT)
                t.until(lambda:b'Command exited (' in t.output)
                t.send('\r');t.send('\x0eafter-interrupt.txt\r');t.until(lambda:(root/'after-interrupt.txt').exists())
                t.send('Q');self.assertEqual(t.process.wait(timeout=5),0)
            finally:
                if child_pid:
                    try:os.kill(child_pid,signal.SIGKILL)
                    except ProcessLookupError:pass
