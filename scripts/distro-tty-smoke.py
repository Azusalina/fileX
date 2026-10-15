#!/usr/bin/env python3
"""Run inside an isolated distro root (see distro-smoke.py), not on the real home."""
import fcntl
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import termios
import time

root=Path('/root/tty-fixture');root.mkdir()
script=root/'run.py';script.write_text("from pathlib import Path\nPath('runner.txt').write_text('python3 fallback OK')\n")
config=Path('/root/tty.conf');config.write_text('[input_method]\nauto_english=false\n[open_defaults]\n".py"="python"\n')
class Terminal:
    def __init__(self,command):
        self.master,self.slave=pty.openpty();fcntl.ioctl(self.slave,termios.TIOCSWINSZ,struct.pack('HHHH',28,150,0,0))
        self.before=termios.tcgetattr(self.slave);self.data=bytearray()
        self.process=subprocess.Popen(command,stdin=self.slave,stdout=self.slave,stderr=self.slave)
    def drain(self):
        if select.select([self.master],[],[],.03)[0]:self.data.extend(os.read(self.master,65536))
    def until(self,test):
        deadline=time.monotonic()+6
        while not test() and time.monotonic()<deadline:self.drain()
        assert test(),self.data.decode(errors='replace')[-2000:]
    def send(self,text):os.write(self.master,text.encode());time.sleep(.05);self.drain()
    def close(self):
        if self.process.poll() is None:self.process.terminate();self.process.wait(timeout=3)
        os.close(self.master);os.close(self.slave)

terminal=Terminal(['/app/filex.sh',str(root),'--config',str(config)])
try:
    terminal.until(lambda:b'Up/Down peers' in terminal.data);terminal.send('\x1b[Co')
    terminal.until(lambda:(root/'runner.txt').exists())
    assert (root/'runner.txt').read_text()=='python3 fallback OK'
    terminal.until(lambda:b'Enter to return' in terminal.data);terminal.send('\r\x0ecreated.txt\r')
    terminal.until(lambda:(root/'created.txt').exists());terminal.send('r\x15renamed.txt\r')
    terminal.until(lambda:(root/'renamed.txt').exists());terminal.send('ccopied.txt\r')
    terminal.until(lambda:(root/'copied.txt').exists());terminal.send('mmoved.txt\r')
    terminal.until(lambda:(root/'moved.txt').exists());terminal.send('D\r')
    terminal.until(lambda:not (root/'moved.txt').exists())
    assert (Path('/root/.local/share/Trash/files/moved.txt')).exists()
    terminal.send('Q');terminal.until(lambda:terminal.process.poll() is not None)
    assert terminal.process.returncode==0;assert termios.tcgetattr(terminal.slave)==terminal.before
finally:terminal.close()

# Zsh must change the caller directory, not spawn a nested shell for x.
(root/'selected folder').mkdir();terminal=Terminal(['/bin/zsh','-li'])
try:
    terminal.send("filex '/root/tty-fixture' --config '/root/tty.conf'\r")
    terminal.until(lambda:b'Up/Down peers' in terminal.data);terminal.send('\x1b[Cx')
    terminal.send("printf '%s' \"$PWD\" > /root/zsh-cwd\r")
    terminal.until(lambda:Path('/root/zsh-cwd').exists())
    assert Path('/root/zsh-cwd').read_text()==str(root/'selected folder')
    terminal.send('exit\r');terminal.until(lambda:terminal.process.poll() is not None)
    assert terminal.process.returncode==0;assert termios.tcgetattr(terminal.slave)==terminal.before
finally:terminal.close()
print('PASS: native TTY create/rename/copy/move/trash, Python3 opener, termios restoration, Zsh x in caller shell')
