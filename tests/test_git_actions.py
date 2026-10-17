"""Exercise Git writes only in temporary repositories with a local bare remote."""
from pathlib import Path
import subprocess
import tempfile
import unittest
from test_runtime_fixes import ROOT, Terminal


class GitActionTests(unittest.TestCase):
    def git(self, repo, *args):
        return subprocess.run(['git', '-C', str(repo), *args], check=True, capture_output=True).stdout.decode().strip()

    def fixture(self, base):
        repo = base/"repo ' 中文"; repo.mkdir()
        remote = base/'remote.git'
        self.git(repo, 'init', '-q', '-b', 'main')
        self.git(repo, 'config', 'user.name', 'Fixture')
        self.git(repo, 'config', 'user.email', 'fixture@example.invalid')
        self.git(repo, 'config', 'commit.gpgsign', 'false')
        self.git(repo, 'config', 'core.hooksPath', '/dev/null')
        subprocess.run(['git', 'init', '--bare', '-q', str(remote)], check=True)
        (repo/'initial.txt').write_text('initial')
        self.git(repo, 'add', '.'); self.git(repo, 'commit', '-qm', 'initial')
        self.git(repo, 'remote', 'add', 'origin', str(remote))
        self.git(repo, 'push', '-qu', 'origin', 'main')
        config = base/'settings.conf'
        config.write_text('[input_method]\nauto_english=false\n[terminal]\nmode="tty"\ncharset="ascii"\n[git]\nenabled=false\n')
        terminal = Terminal([str(ROOT/'filex.sh'), str(repo), '--config', str(config)], repo)
        self.addCleanup(terminal.close)
        terminal.until(lambda: b'Space:mark' in terminal.output)
        return repo, remote, terminal

    def wait_result(self, terminal, success=True):
        terminal.until(lambda: (b'Git operation completed - Enter' if success else b'Git operation failed (later steps skipped)') in terminal.output)
        terminal.send('\r')

    def test_publish_cancel_and_literal_commit_message_to_local_remote(self):
        with tempfile.TemporaryDirectory(prefix='filex-git-actions-') as d:
            repo, remote, terminal = self.fixture(Path(d))
            before = self.git(repo, 'rev-parse', 'HEAD')
            (repo/'new.txt').write_text('new')
            terminal.send('G\rCancelled message\r')
            terminal.until(lambda: b'WHOLE REPOSITORY:' in terminal.output)
            terminal.send('\x1b')
            self.assertEqual(self.git(repo, 'diff', '--cached', '--name-only'), '')
            self.assertEqual(self.git(repo, 'rev-parse', 'HEAD'), before)
            message = "feat: 中文 ' \" $(touch INJECTED) --amend"
            terminal.send('G\r'+message+'\r\r')
            self.wait_result(terminal)
            self.assertEqual(self.git(repo, 'log', '-1', '--format=%s'), message)
            self.assertEqual(self.git(remote, 'rev-parse', 'refs/heads/main'), self.git(repo, 'rev-parse', 'HEAD'))
            self.assertFalse((repo/'INJECTED').exists())
            terminal.send('\x0eafter-git.txt\r')
            terminal.until(lambda: (repo/'after-git.txt').exists())
            terminal.send('Q'); self.assertEqual(terminal.process.wait(timeout=5), 0)

    def test_add_selected_only_and_failed_commit_skips_push(self):
        with tempfile.TemporaryDirectory(prefix='filex-git-stage-') as d:
            repo, remote, terminal = self.fixture(Path(d))
            original = self.git(remote, 'rev-parse', 'refs/heads/main')
            (repo/'chosen.txt').write_text('chosen')
            (repo/'other.txt').write_text('other')
            terminal.send('\x12')  # refresh externally created files
            terminal.send('\x06chosen.txt\rG\x1b[B\r\r')
            self.wait_result(terminal)
            self.assertEqual(self.git(repo, 'diff', '--cached', '--name-only'), 'chosen.txt')
            hooks = Path(d)/'hooks'; hooks.mkdir()
            precommit = hooks/'pre-commit'; precommit.write_text('#!/bin/sh\necho EXPECTED_HOOK_FAILURE >&2\nexit 1\n'); precommit.chmod(0o700)
            self.git(repo, 'config', 'core.hooksPath', str(hooks))
            terminal.output.clear()
            terminal.send('G\rwill fail\r\r')
            self.wait_result(terminal, success=False)
            self.assertIn(b'EXPECTED_HOOK_FAILURE', terminal.output)
            self.assertEqual(self.git(remote, 'rev-parse', 'refs/heads/main'), original)
            self.assertEqual(self.git(repo, 'rev-parse', 'HEAD'), original)
            self.assertEqual(self.git(repo, 'diff', '--cached', '--name-only').splitlines(), ['chosen.txt', 'other.txt'])
            terminal.send('Q'); self.assertEqual(terminal.process.wait(timeout=5), 0)
