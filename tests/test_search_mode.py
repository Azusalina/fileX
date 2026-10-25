"""Search-result selection through actual PTY key events."""
import os
from pathlib import Path
import tempfile
import unittest
from test_runtime_fixes import ROOT, Terminal


class SearchModeTests(unittest.TestCase):
    def test_result_arrows_wrap_and_escape_keeps_current_result(self):
        with tempfile.TemporaryDirectory(prefix='filex-find-mode-') as d:
            base=Path(d);root=base/'files';root.mkdir()
            for folder in ['a','b','c']:
                (root/folder).mkdir();(root/folder/f'needle-{folder}.txt').touch()
            config=base/'settings.conf';config.write_text('[input_method]\nauto_english=false\n[terminal]\nmode="tty"\ncharset="ascii"\n')
            callback=base/'cwd'
            terminal=Terminal([str(ROOT/'filex.sh'),str(root),'--config',str(config)],root,{'FILEX_CWD_FILE':str(callback)})
            try:
                terminal.until(lambda:b'Space:mark' in terminal.output)
                terminal.send('\x06needle-\r')
                terminal.until(lambda:b'FIND [3]' in terminal.output)
                for key,index in [('\x1b[B',2),('\x1b[B',3),('\x1b[B',1),('\x1b[A',3)]:
                    terminal.output.clear();terminal.send(key)
                    terminal.until(lambda:f'{index}/3'.encode() in terminal.output)
                terminal.output.clear();terminal.send('\x1b')
                terminal.until(lambda:b'Find mode closed' in terminal.output)
                self.assertNotIn(b'FIND [',terminal.output)
                terminal.send('x');self.assertEqual(terminal.process.wait(timeout=5),0)
                self.assertEqual(callback.read_bytes(),os.fsencode(root/'c')+b'\0')
            finally:
                terminal.close()

    def test_zero_matches_and_cancelled_search_input_exit_mode(self):
        with tempfile.TemporaryDirectory(prefix='filex-find-empty-') as d:
            root=Path(d);(root/'one.txt').touch();config=root/'.settings.conf';config.write_text('[input_method]\nauto_english=false\n[terminal]\nmode="tty"\ncharset="ascii"\n')
            terminal=Terminal([str(ROOT/'filex.sh'),str(root),'--config',str(config)],root)
            try:
                terminal.until(lambda:b'Space:mark' in terminal.output)
                terminal.send('\x06missing-name\r')
                terminal.until(lambda:b'FIND [0]' in terminal.output)
                terminal.send('\x1b[B\x1b[A')
                terminal.until(lambda:b'No matches' in terminal.output)
                terminal.send('\x06')
                terminal.send('\x1b')
                terminal.output.clear();terminal.send('\x1b[C')
                self.assertNotIn(b'FIND [',terminal.output)
                terminal.send('Q');self.assertEqual(terminal.process.wait(timeout=5),0)
            finally:
                terminal.close()
