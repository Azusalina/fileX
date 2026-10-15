"""Portable package diagnostics and shell integration checks, without host mutations."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from test_runtime_fixes import ROOT, Terminal

spec=importlib.util.spec_from_file_location('check_system',ROOT/'scripts/check-system.py')
check=importlib.util.module_from_spec(spec);spec.loader.exec_module(check)

class PortabilityTests(unittest.TestCase):
    def test_distro_package_families_are_separate(self):
        for identity,like,expected in [('ubuntu','debian','debian'),('kali','debian','debian'),('fedora','','fedora'),('arch','','arch')]:
            with self.subTest(identity=identity),tempfile.TemporaryDirectory() as d:
                release=Path(d)/'release';release.write_text(f'ID={identity}\nID_LIKE="{like}"\nPRETTY_NAME="Fixture {identity}"\n')
                data=check.read_release(release);self.assertEqual(check.family(data),expected)
                self.assertIn('python3' if expected!='arch' else 'python',check.PACKAGES[expected])
        self.assertEqual(check.family({'ID':'unknown'}),'unknown')

    def test_theme_defaults_sample_and_personal_rules_agree(self):
        from filex.config import Config,load_config
        expected=Config().extension_colors
        self.assertGreaterEqual(len(expected),100)
        self.assertEqual(load_config(ROOT/'fileX.example.conf').extension_colors,expected)
        self.assertEqual(load_config(ROOT/'filex.conf').extension_colors,expected)
        self.assertEqual(expected['.js'],'#FFD700')
        self.assertEqual(expected['.tar.gz'],'#FF5555')

    def test_zsh_installation_selects_zdotdir_and_preserves_bash_files(self):
        with tempfile.TemporaryDirectory() as d:
            home=Path(d);zdot=home/'z-config';zdot.mkdir();(home/'.bashrc').write_text('preserve bash\n')
            env={**os.environ,'HOME':str(home),'ZDOTDIR':str(zdot),'SHELL':'/bin/zsh'}
            install=['python3',str(ROOT/'scripts/install_alias.py')]
            subprocess.run(install,env=env,check=True,capture_output=True)
            rc=zdot/'.zshrc';profile=zdot/'.zprofile';before=(rc.read_bytes(),profile.read_bytes())
            subprocess.run(install,env=env,check=True,capture_output=True)
            self.assertEqual(before,(rc.read_bytes(),profile.read_bytes()))
            self.assertNotIn('.zshrc',profile.read_text()) # Zsh loads it exactly once itself.
            self.assertEqual((home/'.bashrc').read_text(),'preserve bash\n')
            subprocess.run(install+['--remove'],env=env,check=True,capture_output=True)
            self.assertFalse(profile.exists());self.assertNotIn('fileX alias',rc.read_text())

if __name__=='__main__':unittest.main()
