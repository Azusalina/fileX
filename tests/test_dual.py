"""Real PTY checks for mirrored panes and transfers; only isolated fixtures."""
import fcntl
import os
from pathlib import Path
import signal
import struct
import tempfile
import termios
import unittest
from test_runtime_fixes import ROOT, Terminal


class DualPaneTests(unittest.TestCase):
    def start(self, root, config, callback):
        terminal=Terminal([str(ROOT/'filex.sh'),str(root),'--config',str(config)],root,{'FILEX_CWD_FILE':str(callback)})
        terminal.until(lambda: '↑↓'.encode() in terminal.output)
        self.addCleanup(terminal.close)
        return terminal

    def destination(self, terminal, directory):
        terminal.send('v\tg\x15'+str(directory)+'\r')
        terminal.until(lambda: '右 ●'.encode() in terminal.output)

    def test_mirrored_navigation_copy_move_conflict_and_exit(self):
        with tempfile.TemporaryDirectory(prefix='filex-dual-pty-') as d:
            root=Path(d); left=root/'left'; left.mkdir(); right=root/"right ' 中文"; right.mkdir()
            dest=right/'chosen'; dest.mkdir(); (dest/'seed.txt').write_text('seed')
            source=left/'copy.txt'; source.write_text('copied payload'); moved=left/'move.txt'; moved.write_text('moved payload')
            config=root/'settings.conf'; config.write_text('[input_method]\nauto_english=false\n')
            callback=root/'cwd'; terminal=self.start(left,config,callback)
            self.destination(terminal,right)
            terminal.send('\x1b[D\x1b[D\x1b[D\x1b[C') # chosen -> expand -> seed -> chosen
            terminal.send('\t\x1b[Cc\r')
            terminal.until(lambda:(dest/'copy.txt').exists()); self.assertEqual((dest/'copy.txt').read_text(),'copied payload')
            terminal.send('c\r');terminal.until(lambda:'目标已存在'.encode() in terminal.output)
            self.assertEqual(source.read_text(),'copied payload');self.assertEqual((dest/'copy.txt').read_text(),'copied payload')
            terminal.send('c\x1b') # cancel without transfer
            terminal.send('\x1b[Bm\r');terminal.until(lambda:(dest/'move.txt').exists() and not moved.exists())
            self.assertEqual((dest/'move.txt').read_text(),'moved payload')
            terminal.send('\t\x1b[D') # destination stays selected / expanded after transfer
            terminal.send('p');terminal.until(lambda:b'copied payload' in terminal.output); terminal.send('\x1b\x1b[Cv')
            terminal.send('x');self.assertEqual(terminal.process.wait(timeout=5),0)
            self.assertEqual(callback.read_bytes(),os.fsencode(dest)+b'\0')

    def test_directory_and_symlink_copy_preserve_source(self):
        with tempfile.TemporaryDirectory(prefix='filex-dual-copy-') as d:
            root=Path(d); left=root/'left';left.mkdir();right=root/'right';right.mkdir()
            folder=left/'pack';folder.mkdir();(folder/'nested.txt').write_text('nested content')
            (left/'target.txt').write_text('original');(left/'a.js').symlink_to('target.txt')
            config=root/'settings.conf';config.write_text('[input_method]\nauto_english=false\n')
            terminal=self.start(left,config,root/'cwd');self.destination(terminal,right)
            terminal.send('\t\x1b[Cc\r');terminal.until(lambda:(right/'pack/nested.txt').exists())
            self.assertEqual((right/'pack/nested.txt').read_text(),'nested content');self.assertTrue(folder.exists())
            terminal.send('\x1b[Bc\r');terminal.until(lambda:(right/'a.js').is_symlink())
            self.assertEqual(os.readlink(right/'a.js'),'target.txt');self.assertTrue((left/'a.js').is_symlink())
            terminal.send('Q');self.assertEqual(terminal.process.wait(timeout=5),0)

    def test_custom_toggle_switch_and_narrow_terminal_recovery(self):
        with tempfile.TemporaryDirectory(prefix='filex-dual-keys-') as d:
            root=Path(d);config=root/'.settings.conf'
            config.write_text('[input_method]\nauto_english=false\n[keys]\ntoggle_dual="t"\nswitch_pane="s"\n')
            terminal=self.start(root,config,root/'cwd');terminal.send('ts')
            terminal.until(lambda:'右 ●'.encode() in terminal.output)
            fcntl.ioctl(terminal.master,termios.TIOCSWINSZ,struct.pack('HHHH',20,50,0,0));os.kill(terminal.process.pid,signal.SIGWINCH)
            terminal.until(lambda:b'75' in terminal.output)
            terminal.send('t\x0eafter-resize.txt\r');terminal.until(lambda:(root/'after-resize.txt').exists())
            terminal.send('Q');self.assertEqual(terminal.process.wait(timeout=5),0)

    def test_startup_dual_and_mirrored_symlink_target_exit(self):
        with tempfile.TemporaryDirectory(prefix='filex-dual-link-') as d:
            root=Path(d);left=root/'left';left.mkdir();target_folder=root/'target';target_folder.mkdir()
            target=target_folder/'b.txt';target.write_text('destination');(left/'a.js').symlink_to(target)
            config=root/'settings.conf';config.write_text('[input_method]\nauto_english=false\n[layout]\ndual_pane=true\n')
            callback=root/'cwd';terminal=self.start(left,config,callback)
            terminal.until(lambda:'左 ●'.encode() in terminal.output)
            terminal.send('\t\x1b[D\x1b[D')
            terminal.until(lambda:'→ 返回链接'.encode() in terminal.output)
            self.assertIn(b'\x1b[7m\x1b[1mb.txt',terminal.output)
            terminal.send('\x1b[C\x1b[Dx')
            self.assertEqual(terminal.process.wait(timeout=5),0)
            self.assertEqual(callback.read_bytes(),os.fsencode(target_folder)+b'\0')
