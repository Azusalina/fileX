"""Temporary branch isolation never changes persistent view preferences."""
from pathlib import Path
import tempfile
import unittest
from test_runtime_fixes import ROOT, Terminal


class TemporaryFocusTests(unittest.TestCase):
    def start(self, root, config):
        t=Terminal([str(ROOT/'filex.sh'),str(root),'--config',str(config)],root)
        self.addCleanup(t.close)
        t.until(lambda:b'Space:mark' in t.output)
        return t

    def test_folder_file_and_custom_hotkey_restore_without_config_writes(self):
        with tempfile.TemporaryDirectory(prefix='filex-temporary-focus-') as d:
            base=Path(d);root=base/'files';root.mkdir()
            for name in ['branch-a','branch-b']:
                (root/name).mkdir();(root/name/f'{name}-child.txt').touch()
            (root/'loose.txt').touch()
            config=base/'settings.conf';original='[input_method]\nauto_english=false\n[terminal]\nmode="tty"\ncharset="ascii"\n[keys]\ntemporary_focus="B"\n';config.write_text(original)
            t=self.start(root,config)
            for name in ['branch-b','branch-a']:
                t.send('\x06\x15'+name+'\r');t.send('\x1b');t.send('\r')
            t.output.clear();t.send('B')
            t.until(lambda:b'FOCUS [B toggle here]' in t.output)
            self.assertNotIn(b'branch-b',t.output);self.assertNotIn(b'loose.txt',t.output)
            t.send('\x1b[B\x1b[A\x0einside.txt\r')
            t.until(lambda:(root/'branch-a/inside.txt').exists())
            t.send('\x1b[D');t.output.clear();t.send('B')
            t.until(lambda:b'Temporary focus restored' in t.output)
            self.assertIn(b'branch-b-child.txt',t.output)
            self.assertIn(b'loose.txt',t.output)
            t.send('\x06\x15loose.txt\r');t.send('\x1b')
            t.output.clear();t.send('B')
            t.until(lambda:b'FOCUS [B toggle here]' in t.output)
            self.assertNotIn(b'branch-a',t.output);self.assertNotIn(b'branch-b',t.output)
            t.send('B');t.send('Q');self.assertEqual(t.process.wait(timeout=5),0)
            self.assertEqual(config.read_text(),original)
            t=self.start(root,config);self.assertNotIn(b'FOCUS [',t.output)
            self.assertIn(b'branch-b',t.output);t.send('Q');self.assertEqual(t.process.wait(timeout=5),0)

    def test_default_F_restores_during_pending_copy(self):
        with tempfile.TemporaryDirectory(prefix='filex-temporary-transfer-') as d:
            base=Path(d);root=base/'files';root.mkdir();dest=root/'destination';dest.mkdir()
            (root/'alpha.txt').write_text('copy me');(root/'beta.txt').touch()
            config=base/'settings.conf';original='[input_method]\nauto_english=false\n[terminal]\nmode="tty"\ncharset="ascii"\n';config.write_text(original)
            t=self.start(root,config)
            t.send('\x06alpha.txt\r');t.send('\x1b');t.send('Fc')
            t.until(lambda:b'COPY 1 items' in t.output)
            t.send('F');t.until(lambda:b'Temporary focus restored' in t.output)
            t.send('t\x15'+str(dest)+'\r')
            t.until(lambda:(dest/'alpha.txt').exists())
            self.assertTrue((root/'alpha.txt').exists());self.assertEqual((dest/'alpha.txt').read_text(),'copy me')
            self.assertEqual(config.read_text(),original)
            t.send('Q');self.assertEqual(t.process.wait(timeout=5),0)

    def test_nested_focus_preserves_ancestor_sibling_branches(self):
        with tempfile.TemporaryDirectory(prefix='filex-local-focus-') as d:
            base=Path(d);root=base/'files';root.mkdir()
            for name in ['a/b', 'a/hiddenSiblingC', 'a/hiddenSiblingD', 'other/child']:
                (root/name).mkdir(parents=True)
            (root/'a/b/kept.txt').touch()
            config=base/'settings.conf'
            config.write_text('[input_method]\nauto_english=false\n[terminal]\nmode="tty"\ncharset="ascii"\n')
            t=self.start(root,config)
            t.send('\x06\x15kept.txt\r');t.send('\x1b');t.send('\x1b[D')
            t.output.clear();t.send('F')
            t.until(lambda:b'FOCUS [F toggle here]' in t.output)
            self.assertIn(b'other',t.output)
            self.assertIn(b'kept.txt',t.output)
            self.assertNotIn(b'hiddenSiblingC',t.output)
            self.assertNotIn(b'hiddenSiblingD',t.output)
            t.send('\x06\x15child\r')
            t.until(lambda:b'FIND' in t.output)
            t.send('\x1b');t.send('\x06\x15kept.txt\r');t.send('\x1b');t.send('\x1b[D');t.output.clear();t.send('F')
            t.until(lambda:b'Temporary focus restored' in t.output)
            t.send('Q');self.assertEqual(t.process.wait(timeout=5),0)

    def test_nested_levels_and_independent_branch_with_parent_restore(self):
        with tempfile.TemporaryDirectory(prefix='filex-multiple-focus-') as d:
            base=Path(d);root=base/'files';root.mkdir()
            for name in ['branchA/deepA', 'branchA/hiddenAtA', 'branchB/deepB', 'branchB/hiddenAtB']:
                (root/name).mkdir(parents=True)
            for name in ['branchA/deepA/primary.txt', 'branchA/deepA/hiddenLeaf.txt']:
                (root/name).touch()
            config=base/'settings.conf';original='[input_method]\nauto_english=false\n[terminal]\nmode="tty"\ncharset="ascii"\n';config.write_text(original)
            t=self.start(root,config)
            def select(name):
                t.send('\x06\x15'+name+'\r');t.send('\x1b')
            select('deepA');t.send('F')
            select('primary.txt');t.send('F')
            select('deepB');t.send('F')
            t.output.clear();t.send('\x12')
            t.until(lambda:b'FOCUS [' in t.output)
            for name in [b'hiddenAtA',b'hiddenAtB',b'hiddenLeaf.txt']:
                self.assertNotIn(name,t.output)
            select('deepA');t.output.clear();t.send('F')
            t.until(lambda:b'Temporary focus restored' in t.output)
            self.assertIn(b'hiddenAtA',t.output)
            self.assertIn(b'hiddenLeaf.txt',t.output)
            self.assertNotIn(b'hiddenAtB',t.output)
            select('deepB');t.output.clear();t.send('F')
            t.until(lambda:b'Temporary focus restored' in t.output)
            self.assertIn(b'hiddenAtB',t.output)
            self.assertNotIn(b'FOCUS [',t.output)
            self.assertEqual(config.read_text(),original)
            t.send('Q');self.assertEqual(t.process.wait(timeout=5),0)
