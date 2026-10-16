"""Real terminal regression for destination + and cursor editing."""
from pathlib import Path
import tempfile
import unittest
from test_runtime_fixes import ROOT, Terminal


class CursorOperationsTests(unittest.TestCase):
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
                terminal.send('\x1b[C\rcreated 子目录\r\r')
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
