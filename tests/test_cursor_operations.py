"""Real terminal regression for destination + and cursor editing."""
from pathlib import Path
import tempfile
import unittest
from test_runtime_fixes import ROOT, Terminal


class CursorOperationsTests(unittest.TestCase):
    def test_replace_enter_cancel_and_typed_destination(self):
        with tempfile.TemporaryDirectory(prefix='filex-replace-pty-') as d:
            root = Path(d)
            source = root/'source.txt'; source.write_text('new')
            target = root/'target'; target.mkdir()
            existing = target/'source.txt'; existing.write_text('old')
            config = root/'.settings.conf'
            config.write_text('[input_method]\nauto_english=false\n[terminal]\nmode="tty"\ncharset="ascii"\n')
            terminal = Terminal([str(ROOT/'filex.sh'), str(root), '--config', str(config)], root)
            try:
                terminal.until(lambda: b'Space:mark' in terminal.output)
                terminal.send('\x06source.txt\rc')
                terminal.send('t\x15'+str(target)+'\r')
                terminal.until(lambda: b'Replace existing file:' in terminal.output)
                self.assertEqual(existing.read_text(), 'old')
                terminal.send('\x1b')
                self.assertEqual(existing.read_text(), 'old')
                terminal.send('t\x15'+str(target)+'\r')
                terminal.send('\r')
                terminal.until(lambda: existing.read_text() == 'new')
                self.assertTrue(source.exists())
                terminal.send('\x06\x15source.txt\rm')
                renamed = root/'renamed.txt'
                terminal.send('t\x15'+str(renamed)+'\r')
                terminal.until(renamed.exists)
                self.assertEqual(renamed.read_text(), 'new')
                self.assertFalse(source.exists())
                terminal.send('Q')
                self.assertEqual(terminal.process.wait(timeout=5), 0)
            finally:
                terminal.close()

    def test_cursor_replace_file_and_batch_cancel_before_any_write(self):
        with tempfile.TemporaryDirectory(prefix='filex-replace-batch-') as d:
            root = Path(d)
            a = root/'alpha.txt'; a.write_text('new-a')
            b = root/'beta.txt'; b.write_text('new-b')
            dest = root/'target'; dest.mkdir()
            (dest/a.name).write_text('old-a'); (dest/b.name).write_text('old-b')
            direct = root/'direct.txt'; direct.write_text('direct-old')
            config = root/'.settings.conf'
            config.write_text('[input_method]\nauto_english=false\n[terminal]\nmode="tty"\ncharset="ascii"\n')
            terminal = Terminal([str(ROOT/'filex.sh'), str(root), '--config', str(config)], root)
            try:
                terminal.until(lambda: b'Space:mark' in terminal.output)
                terminal.send('\x06alpha.txt\r ')
                terminal.send('\x06\x15beta.txt\r c')
                terminal.send('g\x15'+str(dest)+'\r')
                terminal.send('\r')
                terminal.until(lambda: b'Replace existing file:' in terminal.output)
                terminal.send('\r')
                terminal.send('\x1b')
                self.assertEqual((dest/a.name).read_text(), 'old-a')
                self.assertEqual((dest/b.name).read_text(), 'old-b')
                terminal.send('\x1b')  # exit find mode, preserve transfer and marks
                terminal.send('\x1b')  # cancel transfer
                terminal.send('\x1b')  # clear marks
                terminal.send('g\x15'+str(root)+'\r')
                terminal.send('\x06\x15alpha.txt\rc')
                terminal.send('\x06\x15direct.txt\r')
                terminal.send('\r')  # selected file is the destination
                self.assertEqual(direct.read_text(), 'direct-old')
                terminal.send('\r')
                terminal.until(lambda: direct.read_text() == 'new-a')
                self.assertTrue(a.exists())
                terminal.send('Q')
                self.assertEqual(terminal.process.wait(timeout=5), 0)
            finally:
                terminal.close()

    def test_plus_entry_copies_batch_and_moves_into_folder(self):
        with tempfile.TemporaryDirectory(prefix='filex-plus-place-') as d:
            root = Path(d)
            a = root/'a.txt'; a.write_text('alpha')
            b = root/'b.txt'; b.write_text('beta')
            target = root/'target'; target.mkdir()
            (target/'existing-folder').mkdir()
            (target/'existing.txt').write_text('keep')
            moved = root/'moved'; moved.mkdir()
            config = root/'.settings.conf'
            config.write_text('[input_method]\nauto_english=false\n[terminal]\nmode="tty"\ncharset="ascii"\n')
            terminal = Terminal([str(ROOT/'filex.sh'), str(root), '--config', str(config)], root)
            try:
                terminal.until(lambda: b'Space:mark' in terminal.output)
                terminal.send('\x06a.txt\r ')
                terminal.send('\x06\x15b.txt\r c')
                terminal.send('g\x15'+str(target)+'\r\x1b[C\r')
                terminal.until(lambda: (target/'a.txt').exists() and (target/'b.txt').exists())
                self.assertEqual((target/'a.txt').read_text(), 'alpha')
                self.assertTrue(a.exists() and b.exists())
                terminal.send('g\x15'+str(root)+'\r')
                terminal.send('\x06\x15a.txt\rm')
                terminal.send('g\x15'+str(moved)+'\r+')
                terminal.until(lambda: (moved/'a.txt').exists() and not a.exists())
                self.assertEqual((moved/'a.txt').read_text(), 'alpha')
                terminal.send('Q')
                self.assertEqual(terminal.process.wait(timeout=5), 0)
            finally:
                terminal.close()

    def test_multiple_sources_plus_creation_and_mid_name_edit(self):
        with tempfile.TemporaryDirectory(prefix='filex-cursor-') as d:
            root = Path(d)
            (root/'a中.txt').write_text('alpha')
            (root/'b.txt').write_text('beta')
            destination = root/'target'; destination.mkdir()
            config = root/'.settings.conf'
            config.write_text('[input_method]\nauto_english=false\n[terminal]\nmode="tty"\ncharset="ascii"\n')
            terminal = Terminal([str(ROOT/'filex.sh'), str(root), '--config', str(config)], root)
            try:
                terminal.until(lambda: b'Space:mark' in terminal.output)
                terminal.send('\x06a中.txt\r ')
                self.assertIn(b"+--", terminal.output)
                self.assertIn(b"\x1b[95m", terminal.output)
                terminal.send('\x06\x15b.txt\r c')
                terminal.until(lambda: b'COPY 2 items' in terminal.output)
                terminal.send('g\x15'+str(destination)+'\r')
                terminal.send('Acreated 子目录\r\r')
                created = destination/'created 子目录'
                terminal.until(lambda: (created/'a中.txt').exists() and (created/'b.txt').exists())
                self.assertEqual((created/'a中.txt').read_text(), 'alpha')
                self.assertTrue((root/'a中.txt').exists())
                terminal.send('g\x15'+str(created)+'\r\x1b[Cr')
                terminal.send('\x1b[H\x1b[C\x1b[C\x7fé\r')
                terminal.until(lambda: (created/'aé.txt').exists())
                self.assertFalse((created/'a中.txt').exists())
                terminal.send('Q')
                self.assertEqual(terminal.process.wait(timeout=5), 0)
            finally:
                terminal.close()

    def test_open_menu_hides_unrelated_runners(self):
        with tempfile.TemporaryDirectory(prefix='filex-menu-') as d:
            root = Path(d)
            (root/'a.txt').write_text('text')
            (root/'code.py').write_text('print(1)')
            config = root/'.settings.conf'
            config.write_text('[input_method]\nauto_english=false\n[terminal]\nmode="tty"\ncharset="ascii"\n')
            terminal = Terminal([str(ROOT/'filex.sh'), str(root), '--config', str(config)], root)
            try:
                terminal.until(lambda: b'Space:mark' in terminal.output)
                start = len(terminal.output)
                terminal.send('\x1b[C\r')
                menu = terminal.output[start:]
                self.assertIn(b'Open with', menu)
                for runner in [b'Python', b'Rscript', b'Bash', b'unzip', b'Toolkit Markdown']:
                    self.assertNotIn(runner, menu)
                terminal.send('\x1b')
                terminal.send('\x06code.py\r')
                start = len(terminal.output)
                terminal.send('\r')
                menu = terminal.output[start:]
                self.assertIn(b'Python', menu)
                self.assertNotIn(b'Rscript', menu)
                self.assertNotIn(b'Bash', menu)
                terminal.send('\x1b')
                terminal.send('Q')
                self.assertEqual(terminal.process.wait(timeout=5), 0)
            finally:
                terminal.close()

    def test_git_arrow_background_updates_and_folder_order(self):
        import subprocess
        with tempfile.TemporaryDirectory(prefix='filex-git-pty-') as d:
            root = Path(d)
            def git(*args):
                subprocess.run(['git','-c','user.name=Fixture','-c','user.email=fixture@example.invalid','-c','commit.gpgsign=false','-c','core.hooksPath=/dev/null','-C',str(root),*args],check=True,capture_output=True)
            git('init','-q')
            (root/'index.html').write_text('initial')
            (root/'node_modules').mkdir()
            (root/'node_modules/keep').write_text('keep')
            git('add','.'); git('commit','-qm','initial')
            (root/'index.html').write_text('modified')
            (root/'new.txt').write_text('new')
            config = root/'.settings.conf'
            config.write_text('[input_method]\nauto_english=false\n[terminal]\nmode="tty"\ncharset="ascii"\n[order]\ntop=["index.html"]\nbottom=["node_modules"]\n')
            terminal = Terminal([str(ROOT/'filex.sh'),str(root),'--config',str(config)],root)
            try:
                terminal.until(lambda: b'>M index.html' in terminal.output and b'>? new.txt' in terminal.output)
                terminal.send('\x1b[C')
                terminal.until(lambda: b'Git: modified / unstaged' in terminal.output)
                (root/'index.html').write_text('initial')
                terminal.send('\x1b[15~')
                terminal.until(lambda: b'>C index.html' in terminal.output)
                terminal.send('Q')
                self.assertEqual(terminal.process.wait(timeout=5),0)
            finally:
                terminal.close()
