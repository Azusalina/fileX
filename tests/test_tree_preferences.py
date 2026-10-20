"""Real terminal checks for grouping, scoped order and remembered view exclusions."""
import fcntl
import os
from pathlib import Path
import signal
import struct
import subprocess
import tempfile
import termios
import tomllib
import unittest
from test_runtime_fixes import ROOT, Terminal


class TreePreferenceTests(unittest.TestCase):
    def start(self, root, config, extra=None):
        t = Terminal([str(ROOT/'filex.sh'), str(root), '--config', str(config)], root, extra)
        self.addCleanup(t.close)
        t.until(lambda: b'Space:mark' in t.output)
        return t

    def test_reorder_save_preserves_comments_link_and_survives_restart(self):
        with tempfile.TemporaryDirectory(prefix='filex-order-ui-') as d:
            base=Path(d);root=base/'files';root.mkdir()
            for name in ['a.txt','b.txt','100%.txt']:(root/name).touch()
            config=base/'settings.conf';config.write_text('# keep comment\n[input_method]\nauto_english=false\n')
            link=base/'alias.conf';link.symlink_to(config)
            t=self.start(root,link)
            t.send('\x06b.txt\rO')
            t.until(lambda:b'K/J move earlier/later' in t.output)
            t.send('KK\r')
            t.until(lambda:b'Directory order saved' in t.output)
            self.assertTrue(link.is_symlink());self.assertIn('# keep comment',config.read_text())
            rule=tomllib.loads(config.read_text())['directory_order'][str(root)]
            self.assertEqual(rule['sequence'][0], '=b.txt')
            t.send('Q');self.assertEqual(t.process.wait(timeout=5),0)
            printed=subprocess.run([str(ROOT/'filex.sh'),str(root),'--config',str(link),'--print-tree'],check=True,capture_output=True,text=True).stdout
            self.assertLess(printed.index('b.txt'),printed.index('100%.txt'))

    def test_hide_is_persistent_separate_from_dot_toggle_and_can_be_restored(self):
        with tempfile.TemporaryDirectory(prefix='filex-exclude-ui-') as d:
            base=Path(d);root=base/'files';root.mkdir();cache=root/'cache';cache.mkdir();(cache/'private.txt').write_text('kept');(root/'.dot').touch()
            config=base/'settings.conf';config.write_text('[input_method]\nauto_english=false\n')
            t=self.start(root,config);t.send('\x06cache\rz')
            t.until(lambda:b'Hidden in filex' in t.output)
            self.assertIn(str(cache),tomllib.loads(config.read_text())['browser']['excluded_paths'])
            t.send('Q');self.assertEqual(t.process.wait(timeout=5),0)
            t=self.start(root,config)
            self.assertNotIn(b'cache',t.output)
            t.output.clear();t.send('.')
            t.until(lambda:b'.dot' in t.output);self.assertNotIn(b'cache',t.output)
            t.send('Z');t.until(lambda:b'Filex hidden items:' in t.output);t.send('\r')
            t.until(lambda:b'Item restored in filex' in t.output)
            self.assertIn(b'.dot', t.output)
            self.assertEqual(tomllib.loads(config.read_text())['browser']['excluded_paths'],[])
            self.assertTrue((cache/'private.txt').exists())
            t.send('Q');self.assertEqual(t.process.wait(timeout=5),0)

    def test_group_toggle_keeps_branches_navigation_and_narrow_terminal(self):
        with tempfile.TemporaryDirectory(prefix='filex-groups-ui-') as d:
            root=Path(d);folder=root/'child';folder.mkdir();(folder/'deep.txt').touch();(root/'README.md').touch()
            config=root/'.settings.conf';config.write_text('[input_method]\nauto_english=false\n[keys]\ntoggle_grouped="t"\n')
            t=self.start(root,config);t.send('\x1b[C\r')
            t.until(lambda:b'deep.txt' in t.output)
            t.send('t');t.until(lambda:b'Classic horizontal tree' in t.output)
            t.send('t');t.until(lambda:b'Grouped tree:' in t.output)
            fcntl.ioctl(t.master,termios.TIOCSWINSZ,struct.pack('HHHH',20,50,0,0));os.kill(t.process.pid,signal.SIGWINCH)
            t.send('\x0ecreated.txt\r');t.until(lambda:(folder/'created.txt').exists())
            t.send('Q');self.assertEqual(t.process.wait(timeout=5),0)

    def test_folder_and_link_copy_use_single_tree_destination(self):
        with tempfile.TemporaryDirectory(prefix='filex-single-copy-') as d:
            base=Path(d);left=base/'left';left.mkdir();right=base/'right';right.mkdir();folder=left/'pack';folder.mkdir();(folder/'nested.txt').write_text('nested')
            (left/'target.txt').write_text('target');(left/'a.js').symlink_to('target.txt')
            config=base/'settings.conf';config.write_text('[input_method]\nauto_english=false\n')
            t=self.start(left,config)
            t.send('\x06pack\rc');t.send('t\x15'+str(right)+'\r')
            t.until(lambda:(right/'pack/nested.txt').exists())
            t.send('g\x15'+str(left)+'\r');t.send('\x06\x15a.js\rc');t.send('t\x15'+str(right)+'\r')
            t.until(lambda:(right/'a.js').is_symlink());self.assertEqual(os.readlink(right/'a.js'),'target.txt')
            t.send('Q');self.assertEqual(t.process.wait(timeout=5),0)
