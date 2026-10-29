"""File opening through the real TUI in a private 3x3 tmux window."""
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
import uuid

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'target/release/filex'


@unittest.skipUnless(shutil.which('tmux'), 'requires tmux')
class PaneChooserTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='filex-pane-chooser-')
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.root = self.base/'files'; self.root.mkdir()
        self.note = self.root/"literal ' ; $(touch INJECTED).txt"; self.note.write_text('keep original')
        self.records = self.base/'records.jsonl'
        self.conf = self.base/'settings.conf'
        code = ("import json,os,sys,time; "
                f"f=open({str(self.records)!r},'a'); "
                "f.write(json.dumps([os.environ['TMUX_PANE'],sys.argv[1],os.getcwd()])+'\\n'); "
                "f.close();time.sleep(.25)")
        self.conf.write_text('[input_method]\nauto_english=false\n[openers.fixture]\nlabel="Fixture"\nterminal=true\ncommand='+
                             json.dumps([sys.executable,'-c',code,'{path}'])+'\n[open_defaults]\n".txt"="fixture"\n')
        self.rc = self.base/'bashrc';self.rc.write_text("PS1='filex-test$ '\n")
        self.socket = 'filex-chooser-'+uuid.uuid4().hex
        self.env = {k:v for k,v in os.environ.items() if k not in {
            'TMUX','TMUX_PANE','FILEX_CWD_FILE','DISPLAY','WAYLAND_DISPLAY','DBUS_SESSION_BUS_ADDRESS',
            'SSH_CONNECTION','SSH_TTY','NO_COLOR'} and not k.startswith('KONSOLE_DBUS_')}
        self.env['TERM']='xterm-256color'
        self.addCleanup(lambda: subprocess.run(['tmux','-L',self.socket,'kill-server'],env=self.env,capture_output=True,timeout=5))
        shell = ['/usr/bin/bash','--noprofile','--rcfile',str(self.rc),'-i']
        top = self.tmux('-f','/dev/null','new-session','-d','-s','fixture','-x','180','-y','60',
                        '-c',str(self.root),'-P','-F','#{pane_id}',*shell)
        middle = self.tmux('split-window','-v','-l','40','-t',top,'-P','-F','#{pane_id}',*shell)
        bottom = self.tmux('split-window','-v','-l','19','-t',middle,'-P','-F','#{pane_id}',*shell)
        self.grid=[]
        for left in (top,middle,bottom):
            centre = self.tmux('split-window','-h','-l','120','-t',left,'-P','-F','#{pane_id}',*shell)
            right = self.tmux('split-window','-h','-l','59','-t',centre,'-P','-F','#{pane_id}',*shell)
            self.grid.append([left,centre,right])
        self.source = self.grid[1][1]
        self.targets = [self.grid[0][1],self.grid[2][1],self.grid[1][0],self.grid[1][2]]
        self.corners = [self.grid[0][0],self.grid[0][2],self.grid[2][0],self.grid[2][2]]
        for pane in sum(self.grid,[]):
            self.wait(lambda: 'filex-test$' in self.screen(pane))
        self.send(self.source,shlex.join([str(BINARY),str(self.root),'--ascii','--config',str(self.conf)]))
        self.tmux('send-keys','-t',self.source,'Enter')
        self.wait(lambda:'Space:mark' in self.screen(self.source))
        self.tmux('send-keys','-t',self.source,'Right')

    def tmux(self,*args):
        return subprocess.run(['tmux','-L',self.socket,*args],env=self.env,capture_output=True,text=True,check=True,timeout=5).stdout.rstrip('\n')

    def screen(self,pane):return self.tmux('capture-pane','-p','-t',pane)
    def send(self,pane,text):self.tmux('send-keys','-t',pane,'-l',text)
    def rows(self):return [json.loads(line) for line in self.records.read_text().splitlines()] if self.records.exists() else []
    def wait(self,condition,timeout=6):
        deadline=time.monotonic()+timeout
        while time.monotonic()<deadline:
            if condition():return
            time.sleep(.03)
        self.fail('tmux condition timed out: '+self.screen(getattr(self,'source',self.grid[0][0])))

    def chooser(self):
        self.send(self.source,'o')
        self.wait(lambda:'Open file in terminal' in self.screen(self.source))
        return self.screen(self.source)

    def choose(self,index):
        for _ in range(index):self.tmux('send-keys','-t',self.source,'Down')
        self.tmux('send-keys','-t',self.source,'Enter')

    def test_four_directions_current_cancel_other_window_and_disabled_setting(self):
        other=self.tmux('new-window','-d','-t','fixture','-P','-F','#{pane_id}',
                        '/usr/bin/bash','--noprofile','--rcfile',str(self.rc),'-i')
        for index,(direction,target) in enumerate(zip(('Up','Down','Left','Right'),self.targets),1):
            listing=self.chooser()
            for name,pane in zip(('Up','Down','Left','Right'),self.targets):
                self.assertIn(f'{name} · {pane} · bash',listing)
            for pane in [*self.corners,other]:self.assertNotIn(f' · {pane} · ',listing)
            self.choose(index)
            self.wait(lambda:len(self.rows())==index)
            self.assertEqual(self.rows()[-1],[target,str(self.note),str(self.root)])
            self.assertEqual(self.tmux('display-message','-p','-t',target,'#{pane_active}'),'1')
            self.wait(lambda:self.tmux('display-message','-p','-t',target,'#{pane_current_command}')=='bash')
            time.sleep(.06)
        self.chooser();self.tmux('send-keys','-t',self.source,'Escape')
        self.wait(lambda:'Space:mark' in self.screen(self.source))
        self.assertEqual(len(self.rows()),4)
        self.chooser();self.choose(0);self.wait(lambda:len(self.rows())==5)
        self.assertEqual(self.rows()[-1],[self.source,str(self.note),str(self.root)])
        self.wait(lambda:'Space:mark' in self.screen(self.source))
        self.conf.write_text(self.conf.read_text()+'\n[tmux]\nopen_in_right_pane=false\n')
        self.send(self.source,',');time.sleep(.1);self.send(self.source,'o')
        self.wait(lambda:len(self.rows())==6)
        self.assertEqual(self.rows()[-1][0],self.source)
        self.assertFalse((self.root/'INJECTED').exists());self.assertEqual(self.note.read_text(),'keep original')

    def test_busy_draft_copy_mode_stale_and_closed_targets_do_not_receive_commands(self):
        up,down,left,right=self.targets
        self.send(up,'sleep 60');self.tmux('send-keys','-t',up,'Enter')
        self.send(down,'echo KEEP-DRAFT')
        self.tmux('copy-mode','-t',left)
        self.wait(lambda:self.tmux('display-message','-p','-t',up,'#{pane_current_command}')=='sleep')
        listing=self.chooser()
        for pane in (up,down,left):self.assertNotIn(f' · {pane} · ',listing)
        self.assertIn(f'Right · {right} · bash',listing)
        self.send(right,'echo LATE-DRAFT')
        self.choose(1)
        self.wait(lambda:'Pane changed or busy' in self.screen(self.source))
        self.assertEqual(self.rows(),[])
        self.assertIn('echo KEEP-DRAFT',self.screen(down));self.assertIn('echo LATE-DRAFT',self.screen(right))
        self.assertEqual(self.tmux('display-message','-p','-t',up,'#{pane_current_command}'),'sleep')
        # Only clear text in this fixture's own disposable pane.
        self.tmux('send-keys','-t',right,'C-u');time.sleep(.08)
        self.chooser();self.tmux('kill-pane','-t',right);self.choose(1)
        self.wait(lambda:'Pane changed or busy' in self.screen(self.source))
        self.assertEqual(self.rows(),[])


if __name__=='__main__':unittest.main()
