#!/usr/bin/env python3
"""Read-only Linux dependency report; never installs packages or changes repositories."""
import argparse
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess

PACKAGES = {
 'arch': 'sudo pacman -S --needed base-devel python glib2 nano',
 'fedora': 'sudo dnf install gcc glibc-devel python3 glib2 nano',
 'debian': 'sudo apt update && sudo apt install build-essential python3 libglib2.0-bin nano',
}

def read_release(path):
    data={}
    for line in Path(path).read_text().splitlines():
        if '=' in line and not line.lstrip().startswith('#'):
            key,value=line.split('=',1)
            try: tokens=shlex.split(value);data[key]=tokens[0] if tokens else ''
            except ValueError: continue
    return data

def family(data):
    ids=[data.get('ID',''),*data.get('ID_LIKE','').split()]
    for candidate,names in [('arch',{'arch','manjaro','endeavouros'}),('fedora',{'fedora','rhel','centos'}),('debian',{'debian','ubuntu','kali','linuxmint','pop'})]:
        if set(ids)&names:return candidate
    return 'unknown'

def report(data):
    variant=family(data)
    compiler=shutil.which('rustc');version=None
    if compiler:
        try:
            out=subprocess.run([compiler,'--version'],capture_output=True,text=True,timeout=5).stdout
            match=re.search(r'rustc (\d+)\.(\d+)\.(\d+)',out)
            version=list(map(int,match.groups())) if match else None
        except (OSError,subprocess.TimeoutExpired):pass
    return {'distribution':data.get('PRETTY_NAME',data.get('ID','unknown')), 'family':variant,
        'install_prerequisites':PACKAGES.get(variant,'Install a C linker, Python 3, optional gio/nano via your distribution package manager'),
        'rust_minimum':'1.88', 'rust_version':version,'rust_supported':bool(version and version>=[1,88,0]),
        'commands':{name:shutil.which(name) for name in ('cargo','rustc','python3','gio','nano','nvim','vim','tmux','bash','zsh')},
        'notes':['Browsing and file operations need no desktop service; trash requires gio.',
                 'Build from source on the target distribution; do not copy a newer-glibc Arch binary to older Linux.',
                 'Kali is Debian-based, not Fedora-based. Bash and Zsh shell integration are available.',
                 'If distro Rust is too old, use the official Rust rustup installer; repositories are never modified by this script.']}

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--os-release',type=Path,default=Path('/etc/os-release'));args=parser.parse_args()
    print(json.dumps(report(read_release(args.os_release)),ensure_ascii=False,indent=2))
