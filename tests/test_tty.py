"""Headless and Linux-console protocol checks using isolated real PTYs."""
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import shlex
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time
import unittest
import uuid

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'target/release/filex'


def headless_env():
    env = {k: v for k, v in os.environ.items() if k not in {
        'DISPLAY', 'WAYLAND_DISPLAY', 'DBUS_SESSION_BUS_ADDRESS', 'TMUX', 'TMUX_PANE',
        'SSH_CONNECTION', 'SSH_TTY', 'FILEX_CWD_FILE', 'NO_COLOR', 'COLORTERM',
    } and not k.startswith('KONSOLE_DBUS_')}
    return {**env, 'TERM': 'linux'}


class Console:
    def __init__(self, root, conf, flags=(), extra=None):
        self.master, self.slave = pty.openpty()
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack('HHHH', 28, 150, 0, 0))
        self.original = termios.tcgetattr(self.slave)
        self.output = bytearray()
        self.process = subprocess.Popen([str(BINARY), str(root), '--config', str(conf), *flags],
            stdin=self.slave, stdout=self.slave, stderr=self.slave, env={**headless_env(), **(extra or {})})

    def drain(self, seconds=.05):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if select.select([self.master], [], [], .01)[0]:
                self.output.extend(os.read(self.master, 65536))

    def until(self, condition):
        deadline = time.monotonic() + 6
        while not condition() and time.monotonic() < deadline:
            self.drain()
        if not condition():
            raise AssertionError(self.output.decode(errors='replace')[-2500:])

    def send(self, keys):
        os.write(self.master, keys.encode()); self.drain(.1)

    def finish(self, key='Q'):
        if key: self.send(key)
        self.until(lambda: self.process.poll() is not None)
        self.drain()
        assert self.process.returncode == 0, self.output.decode(errors='replace')[-1500:]
        assert termios.tcgetattr(self.slave) == self.original, 'TTY settings were not restored'

    def close(self):
        if self.process.poll() is None:
            self.process.terminate()
            try: self.process.wait(timeout=3)
            except subprocess.TimeoutExpired: self.process.kill(); self.process.wait()
        os.close(self.master); os.close(self.slave)


class TtyTests(unittest.TestCase):
    def start(self, root, conf, **kw):
        terminal = Console(root, conf, **kw)
        self.addCleanup(terminal.close)
        terminal.until(lambda: b'Up/Down peers' in terminal.output if kw.get('extra', {}).get('TERM', 'linux') == 'linux' or '--tty' in kw.get('flags', ()) else b'filex' in terminal.output)
        return terminal

    def test_pipes_dumb_ascii_and_broken_pipe(self):
        with tempfile.TemporaryDirectory() as d:
            base = Path(d); root = base/'files'; root.mkdir(); (root/'note.txt').touch()
            conf = base/'settings.conf'; conf.write_text('')
            command = [str(BINARY), str(root), '--config', str(conf)]
            automatic = subprocess.run(command, env=headless_env(), capture_output=True, timeout=3)
            explicit = subprocess.run(command+['--print-tree', '--ascii'], env=headless_env(), capture_output=True, timeout=3)
            self.assertEqual(automatic.returncode, 0, automatic.stderr)
            self.assertEqual(automatic.stdout, explicit.stdout)
            self.assertIn(b'note.txt', automatic.stdout); self.assertNotIn(b'\x1b', automatic.stdout)
            dumb = subprocess.run(command, env={**headless_env(), 'TERM':'dumb'}, capture_output=True, timeout=3)
            self.assertEqual(dumb.returncode, 0)
            for i in range(1200): (root/f'item{i:04}.txt').touch()
            process = subprocess.Popen(command, env=headless_env(), stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            process.stdout.close()
            self.assertEqual(process.wait(timeout=3), 0, process.stderr.read())
            process.stderr.close()

    def test_console_operations_internal_path_and_legacy_f5(self):
        with tempfile.TemporaryDirectory() as d:
            base = Path(d); root = base/'files'; root.mkdir(); (root/'seed.txt').write_text('payload')
            conf = base/'settings.conf'; conf.write_text('')
            terminal = self.start(root, conf, extra={'COLORTERM':'truecolor'})
            terminal.send('\x0enew.txt\rAnew-folder\r')
            terminal.until(lambda: (root/'new.txt').exists() and (root/'new-folder').is_dir())
            terminal.send('/seed.txt\rR\x15renamed.txt\r')
            terminal.until(lambda: (root/'renamed.txt').exists())
            terminal.send('ycg\x15\x16' + '\x7f' * len('renamed.txt') + 'new-folder\r\r')
            terminal.until(lambda: (root/'new-folder/renamed.txt').exists())
            self.assertEqual((root/'new-folder/renamed.txt').read_text(), 'payload')
            terminal.send('g\x15'+str(root)+'\r')
            # Linux console F5 uses ESC [ [ E, not the xterm ESC [ 15 ~ sequence.
            (root/'external.txt').touch(); terminal.send('\x1b[[E')
            terminal.until(lambda: b'external.txt' in terminal.output)
            terminal.send('q'); terminal.until(lambda: b'Created' in terminal.output)
            terminal.finish()
            for sequence in (b'\x1b[>4;', b'\x1b[>1u', b'\x1b[?1004', b'\x1b[?1049', b'\x1b[38;2;', b'\x1b[38;5;'):
                self.assertNotIn(sequence, terminal.output)
            self.assertTrue(terminal.output.isascii())

    def test_headless_default_editor_no_gui_and_restored_terminal(self):
        with tempfile.TemporaryDirectory() as d:
            base=Path(d); root=base/'files'; root.mkdir(); note=root/'note.md'; note.write_text('# headless')
            calls=base/'calls'; helpers=base/'bin'; helpers.mkdir()
            for name in ('fcitx5-remote','qdbus6','wl-copy','xdg-open','firefox','codium','node'):
                file=helpers/name; file.write_text('#!/bin/sh\nprintf "%s\\n" '+name+' >> '+str(calls)+'\nexit 1\n'); file.chmod(0o700)
            record=base/'editor.json'
            script="import json,sys,os;open(sys.argv[1],'w').write(json.dumps([sys.argv[2],os.getcwd()]));print('EDITOR-DONE')"
            conf=base/'settings.conf';conf.write_text('[terminal]\ncli_opener="fixture"\n[openers.fixture]\nlabel="Fixture editor"\nterminal=true\ncopy_path=true\ncommand='+json.dumps([sys.executable,'-c',script,str(record),'{path}'])+'\n')
            terminal=self.start(root,conf,extra={'PATH':str(helpers)+':'+os.environ['PATH'], 'DISPLAY':':fake','WAYLAND_DISPLAY':'fake'})
            terminal.send('\x1b[Co');terminal.until(record.exists)
            self.assertEqual(json.loads(record.read_text()), [str(note),str(root)])
            terminal.until(lambda: b'EDITOR-DONE' in terminal.output)
            terminal.send('\x0eafter-editor.txt\r');terminal.until(lambda:(root/'after-editor.txt').exists())
            terminal.finish();self.assertFalse(calls.exists())

    def test_no_editor_falls_back_to_preview_and_ctrl_c_restores(self):
        with tempfile.TemporaryDirectory() as d:
            base=Path(d);root=base/'files';root.mkdir();(root/'note.md').write_text('HEADLESS-PREVIEW')
            conf=base/'settings.conf';conf.write_text('')
            terminal=self.start(root,conf,extra={'PATH':str(base/'missing')})
            terminal.send('\x1b[Co');terminal.until(lambda:b'HEADLESS-PREVIEW' in terminal.output)
            terminal.send('\x1b');terminal.finish('\x03')

    def test_script_explicit_execution_pause_and_return(self):
        with tempfile.TemporaryDirectory() as d:
            base=Path(d);root=base/'files';root.mkdir();record=base/'script-cwd'
            script=root/'run.sh';script.write_text('printf "%s" "$PWD" > '+str(record)+'\nprintf "SCRIPT-DONE\\n"\n')
            conf=base/'settings.conf';conf.write_text('[open_defaults]\n".sh"="bash"\n')
            terminal=self.start(root,conf);terminal.send('\x1b[Co')
            terminal.until(record.exists);self.assertEqual(record.read_text(),str(root))
            terminal.until(lambda:b'Enter to return to filex' in terminal.output)
            terminal.send('\r\x0eafter-script.txt\r');terminal.until(lambda:(root/'after-script.txt').exists())
            terminal.finish()

    def test_dual_mirror_copy_move_target_and_exit_callback(self):
        with tempfile.TemporaryDirectory() as d:
            base=Path(d);left=base/'left';left.mkdir();right=base/'right';right.mkdir();dest=right/'dest';dest.mkdir()
            (dest/'seed.txt').write_text('seed');(left/'copy.txt').write_text('copy');(left/'move.txt').write_text('move')
            conf=base/'settings.conf';conf.write_text('');callback=base/'cwd'
            terminal=self.start(left,conf,extra={'FILEX_CWD_FILE':str(callback)})
            terminal.send('v\tg\x15'+str(right)+'\r');terminal.until(lambda:b'right *' in terminal.output)
            # Right side arrows are mirrored. Enter dest then expand it and return to dest.
            terminal.send('\x1b[D\x1b[D\x1b[D\x1b[C\t\x1b[Cc\r')
            terminal.until(lambda:(dest/'copy.txt').exists())
            terminal.send('\t\x1b[Bm\r');terminal.until(lambda:(dest/'move.txt').exists() and not (left/'move.txt').exists())
            terminal.send('x');terminal.finish(None)
            self.assertEqual(callback.read_bytes(),os.fsencode(dest)+b'\0')
            self.assertIn(b'< dest',terminal.output)

    def test_sigterm_hup_and_forced_tty_no_color(self):
        for stop in (signal.SIGTERM, signal.SIGHUP):
            with self.subTest(stop=stop), tempfile.TemporaryDirectory() as d:
                base=Path(d);root=base/'files';root.mkdir();conf=base/'settings.conf';conf.write_text('[terminal]\ncolor_mode="truecolor"\n')
                terminal=self.start(root,conf,flags=('--tty',),extra={'TERM':'xterm-256color','DISPLAY':':fake','NO_COLOR':'1'})
                terminal.send('\x0epartial-name');terminal.process.send_signal(stop);terminal.finish(None)
                self.assertFalse((root/'partial-name').exists())
                self.assertNotIn(b'\x1b[38;',terminal.output)
                self.assertNotIn(b'\x1b[>4;',terminal.output)

    def test_terminal_config_validation_and_round_trip(self):
        with tempfile.TemporaryDirectory() as d:
            base=Path(d);conf=base/'settings.conf'
            conf.write_text('[terminal]\nmode="tty"\ncolor_mode="256"\ncharset="unicode"\ncli_opener="nano"\n')
            result=subprocess.run([str(BINARY),str(base),'--config',str(conf)],capture_output=True,env=headless_env())
            self.assertEqual(result.returncode,0,result.stderr)
            self.assertIn('▾'.encode(),result.stdout)
            for key,value in [('mode','other'),('charset','other'),('color_mode','other'),('cli_opener','firefox')]:
                conf.write_text('[terminal]\n'+key+'='+json.dumps(value)+'\n')
                result=subprocess.run([str(BINARY),str(base),'--config',str(conf)],capture_output=True,env=headless_env())
                self.assertEqual(result.returncode,1)

    def test_shared_sixteen_colors_match_console_and_konsole_rgb(self):
        channels=[]
        for name in ('default_red','default_grn','default_blu'):
            channels.append([int(v) for v in (Path('/sys/module/vt/parameters')/name).read_text().strip().split(',')])
        palette=list(zip(*channels))
        def rendered_colors(output, native):
            colors={};fg=palette[15];index=15;bold=False;reverse=False
            previous=0
            for match in re.finditer(rb'\x1b\[([0-9;?<>]*)([@-~])',output):
                segment=output[previous:match.start()].decode(errors='replace')
                if segment.startswith(('. ','· ','> ','▸ ','v ','▾ ','------')):
                    for name in ('node_modules','.git','gold.js','a.js','b.txt'):
                        if name in segment:
                            colors[name]=(palette[index|8] if native and bold else fg,reverse,bold)
                previous=match.end()
                if match.group(2)!=b'm' or not re.fullmatch(rb'[0-9;]*',match.group(1)):continue
                codes=[int(v or b'0') for v in match.group(1).split(b';')];i=0
                while i<len(codes):
                    code=codes[i]
                    if code==0:fg=palette[15];index=15;bold=False;reverse=False
                    elif code==1:bold=True
                    elif code==7:reverse=True
                    elif 30<=code<=37:index=code-30;fg=palette[index]
                    elif 90<=code<=97:index=code-90+8;fg=palette[index]
                    elif code in (38,48) and codes[i+1]==2:
                        if code==38:fg=tuple(codes[i+2:i+5])
                        i+=4
                    i+=1
            return colors
        with tempfile.TemporaryDirectory() as d:
            base=Path(d);root=base/'files';root.mkdir()
            (root/'node_modules').mkdir();(root/'.git').mkdir();(root/'gold.js').touch();(root/'b.txt').touch();(root/'a.js').symlink_to('b.txt')
            conf=base/'settings.conf';conf.write_text('[terminal]\ncolor_mode="consistent"\n[input_method]\nauto_english=false\n[browser]\nshow_hidden=true\n')
            terminals=[]
            try:
                for extra in ({}, {'TERM':'xterm-256color','DISPLAY':':fake'}):
                    terminal=Console(root,conf,extra=extra);terminals.append(terminal)
                    terminal.until(lambda:b'gold.js' in terminal.output)
                    terminal.send('\x1b[C')
                native=rendered_colors(terminals[0].output,True);desktop=rendered_colors(terminals[1].output,False)
                self.assertEqual(native,desktop)
                self.assertEqual(set(native),{'node_modules','.git','gold.js','a.js','b.txt'})
                self.assertNotIn(b'\x1b[2m',terminals[0].output)
                self.assertNotIn(b'\x1b[2m',terminals[1].output)
                self.assertNotIn(b'\x1b]P',terminals[0].output)
                for terminal in terminals:terminal.finish()
            finally:
                for terminal in terminals:terminal.close()

    def test_enter_expands_folder_keeps_cursor_and_w_opens_menu(self):
        with tempfile.TemporaryDirectory() as d:
            base=Path(d);root=base/'files';root.mkdir()
            for name in ('a-folder','b-folder'):
                folder=root/name;folder.mkdir();(folder/('leaf-'+name+'.txt')).write_text('content')
            conf=base/'settings.conf';conf.write_text('')
            terminal=self.start(root,conf)
            terminal.send('\x1b[C\r')
            terminal.until(lambda:b'leaf-a-folder.txt' in terminal.output)
            self.assertNotIn(b'Open with:',terminal.output)
            terminal.send('\r\r\x0ecreated-in-a.txt\r')
            terminal.until(lambda:(root/'a-folder/created-in-a.txt').exists())
            self.assertFalse((root/'created-in-a.txt').exists())
            terminal.send('\x1b[D\x1b[B\r')
            terminal.until(lambda:b'leaf-b-folder.txt' in terminal.output)
            start=len(terminal.output);terminal.send('w')
            terminal.until(lambda:b'Open with:' in terminal.output[start:])
            terminal.send('\x1b\r\x0ecreated-in-b.txt\r')
            terminal.until(lambda:(root/'b-folder/created-in-b.txt').exists())
            terminal.finish()

    @unittest.skipUnless(shutil.which('gio'), 'gio is an optional trash dependency')
    def test_headless_trash_confirmation_cancellation_and_preservation(self):
        with tempfile.TemporaryDirectory(dir=Path.home()/'.cache', prefix='filex-tty-trash-') as d:
            base=Path(d);root=base/'files';root.mkdir();source=root/'note.txt';source.write_text('preserve payload')
            conf=base/'settings.conf';conf.write_text('');data=base/'data'
            terminal=self.start(root,conf,extra={'XDG_DATA_HOME':str(data)})
            terminal.send('\x1b[C\x1b[3~');terminal.until(lambda:b'Press Enter again' in terminal.output)
            terminal.send('YES\x1b');self.assertTrue(source.exists())
            terminal.send('D\r');terminal.until(lambda:not source.exists())
            self.assertEqual((data/'Trash/files/note.txt').read_text(),'preserve payload')
            info=(data/'Trash/info/note.txt.trashinfo').read_text()
            self.assertIn('DeletionDate=',info);self.assertIn('Path=',info)
            terminal.finish()

    @unittest.skipUnless(shutil.which('tmux'), 'tmux is optional')
    def test_headless_tmux_right_idle_pane_and_unmodified_protocol_options(self):
        with tempfile.TemporaryDirectory() as d:
            base=Path(d);root=base/'files';root.mkdir();note=root/'note.txt';note.write_text('headless tmux')
            records=base/'record.json';conf=base/'settings.conf';rc=base/'bashrc';rc.write_text("PS1='filex-test$ '\n")
            script="import json,os,sys;open(sys.argv[1],'w').write(json.dumps([os.environ['TMUX_PANE'],sys.argv[2],os.getcwd()]))"
            conf.write_text('[openers.fixture]\nlabel="Fixture"\nterminal=true\ncommand='+json.dumps([sys.executable,'-c',script,str(records),'{path}'])+'\n[open_defaults]\n".txt"="fixture"\n')
            socket='filex-tty-'+uuid.uuid4().hex;env=headless_env()
            def tmux(*args):
                return subprocess.run(['tmux','-L',socket,*args],env=env,capture_output=True,text=True,check=True,timeout=5).stdout.strip()
            def wait(predicate):
                until=time.monotonic()+6
                while not predicate() and time.monotonic()<until: time.sleep(.04)
                self.assertTrue(predicate(),tmux('capture-pane','-p','-t',source))
            try:
                source=tmux('-f','/dev/null','new-session','-d','-s','tty','-x','180','-y','36','-c',str(root),'-P','-F','#{pane_id}', '/usr/bin/bash','--noprofile','--rcfile',str(rc),'-i')
                right=tmux('split-window','-h','-t',source,'-P','-F','#{pane_id}','/usr/bin/bash','--noprofile','--rcfile',str(rc),'-i')
                original_keys=tmux('show-options','-s','-v','extended-keys')
                original_focus=tmux('show-options','-s','-v','focus-events')
                tmux('send-keys','-t',source,'-l',shlex.join([str(BINARY),str(root),'--config',str(conf)]));tmux('send-keys','-t',source,'Enter')
                wait(lambda:'Up/Down peers' in tmux('capture-pane','-p','-t',source))
                self.assertEqual(tmux('show-options','-s','-v','extended-keys'),original_keys)
                self.assertEqual(tmux('show-options','-s','-v','focus-events'),original_focus)
                tmux('send-keys','-t',source,'Right');tmux('send-keys','-t',source,'-l','o')
                wait(records.exists);self.assertEqual(json.loads(records.read_text()),[right,str(note),str(root)])
                tmux('send-keys','-t',source,'-l','Q');wait(lambda:tmux('display-message','-p','-t',source,'#{pane_current_command}')=='bash')
            finally:
                subprocess.run(['tmux','-L',socket,'kill-server'],env=env,capture_output=True,timeout=5)


if __name__ == '__main__': unittest.main()
