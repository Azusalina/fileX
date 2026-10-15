"""Isolated Bash login startup checks; no changes to the real home directory."""
import json
import os
from pathlib import Path
import shlex
import subprocess
import tempfile
import unittest
from test_runtime_fixes import ROOT, Terminal

INSTALL = ROOT/'scripts/install_alias.py'

class LoginTests(unittest.TestCase):
    def test_fresh_login_finds_command_and_function_and_x_changes_same_shell(self):
        with tempfile.TemporaryDirectory(prefix='filex login ') as d:
            home=Path(d);root=home/'files';root.mkdir();folder=root/"child ' 中文";folder.mkdir()
            (home/'.bashrc').write_text("[[ $- != *i* ]] && return\nPS1='LOGIN-TEST$ '\n")
            env={**os.environ,'HOME':str(home),'PATH':'/usr/bin:/bin'}
            subprocess.run(['/usr/bin/python',str(INSTALL)],env=env,check=True,capture_output=True)
            profile=home/'.bash_profile';first=profile.read_bytes()
            subprocess.run(['/usr/bin/python',str(INSTALL)],env=env,check=True,capture_output=True)
            self.assertEqual(profile.read_bytes(),first)
            subprocess.run([str(ROOT/'scripts/install.sh'),'--prefix',str(home/'.local')],env=env,check=True,capture_output=True)
            result=subprocess.run(['/bin/bash','--login','-c','command -v filex; filex --version'],env=env,text=True,capture_output=True,timeout=5)
            self.assertEqual(result.returncode,0,result.stderr)
            self.assertIn(str(home/'.local/bin/filex'),result.stdout)
            conf=home/'settings.conf';conf.write_text('')
            terminal=Terminal(['/bin/bash','--login','-i'],root,{'HOME':str(home),'PATH':'/usr/bin:/bin','TERM':'linux','DISPLAY':'','WAYLAND_DISPLAY':'','DBUS_SESSION_BUS_ADDRESS':''})
            try:
                terminal.until(lambda:b'LOGIN-TEST$' in terminal.output)
                marker=home/'function';terminal.send('declare -F filex > '+shlex.quote(str(marker))+'\r')
                terminal.until(marker.exists);self.assertEqual(marker.read_text().strip(),'filex')
                terminal.send('filex --config '+shlex.quote(str(conf))+'\r')
                terminal.until(lambda:b'Up/Down peers' in terminal.output)
                terminal.send('\x1b[Cx')
                record=home/'cwd';terminal.send('printf "%s" "$PWD" > '+shlex.quote(str(record))+'\r')
                terminal.until(record.exists);self.assertEqual(record.read_text(),str(folder))
                terminal.send('exit\r');self.assertEqual(terminal.process.wait(timeout=5),0)
            finally:terminal.close()
            subprocess.run(['/usr/bin/python',str(INSTALL),'--remove'],env=env,check=True,capture_output=True)
            self.assertFalse(profile.exists())
            self.assertIn("PS1='LOGIN-TEST$ '",(home/'.bashrc').read_text())

    def test_existing_login_priority_preserves_content_and_avoids_double_source(self):
        with tempfile.TemporaryDirectory() as d:
            home=Path(d); rc=home/'.bashrc';rc.write_text('FILEX_RC_COUNT=$(( ${FILEX_RC_COUNT:-0}+1 ))\n')
            profile=home/'.bash_login';original='export FILEX_PRESERVED=yes\n. "$HOME/.bashrc"\n';profile.write_text(original)
            lower=home/'.profile';lower.write_text('export UNRELATED_PROFILE=yes\n')
            env={**os.environ,'HOME':str(home),'PATH':'/usr/bin:/bin'}
            subprocess.run(['/usr/bin/python',str(INSTALL)],env=env,check=True,capture_output=True)
            self.assertFalse((home/'.bash_profile').exists())
            result=subprocess.run(['/bin/bash','--login','-ic','declare -F filex; printf "%s:%s" "$FILEX_RC_COUNT" "$FILEX_PRESERVED"'],env=env,text=True,capture_output=True,timeout=5)
            self.assertEqual(result.returncode,0,result.stderr);self.assertIn('filex\n1:yes',result.stdout)
            self.assertEqual(lower.read_text(),'export UNRELATED_PROFILE=yes\n')
            subprocess.run(['/usr/bin/python',str(INSTALL),'--remove'],env=env,check=True,capture_output=True)
            self.assertEqual(profile.read_text().strip(),original.strip())
            self.assertEqual(rc.read_text().strip(),'FILEX_RC_COUNT=$(( ${FILEX_RC_COUNT:-0}+1 ))')

if __name__=='__main__':unittest.main()
