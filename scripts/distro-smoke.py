#!/usr/bin/env python3
"""Optional rootless Linux userspace smoke test: official OCI layers + bubblewrap.
Downloads go to a user cache. No Docker daemon, sudo, host package changes or tty switching.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import posixpath
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.parse
import urllib.request

ROOT=Path(__file__).resolve().parents[1]
IMAGES=['library/ubuntu:24.04','library/fedora:latest','kalilinux/kali-rolling:latest']

def fetch(url,headers=None):
    with urllib.request.urlopen(urllib.request.Request(url,headers=headers or {}),timeout=60) as response:return response.read()

def rootfs(image,cache):
    repo,tag=image.rsplit(':',1)
    auth=json.loads(fetch('https://auth.docker.io/token?'+urllib.parse.urlencode({'service':'registry.docker.io','scope':f'repository:{repo}:pull'})))['token']
    headers={'Authorization':'Bearer '+auth,'Accept':'application/vnd.oci.image.index.v1+json,application/vnd.docker.distribution.manifest.list.v2+json,application/vnd.oci.image.manifest.v1+json,application/vnd.docker.distribution.manifest.v2+json'}
    manifest=fetch(f'https://registry-1.docker.io/v2/{repo}/manifests/{tag}',headers)
    data=json.loads(manifest)
    if 'manifests' in data:
        arch={'x86_64':'amd64','aarch64':'arm64'}[os.uname().machine]
        digest=next(item['digest'] for item in data['manifests'] if item.get('platform',{}).get('architecture')==arch and item.get('platform',{}).get('os')=='linux')
        manifest=fetch(f'https://registry-1.docker.io/v2/{repo}/manifests/{digest}',headers);data=json.loads(manifest)
        assert 'sha256:'+hashlib.sha256(manifest).hexdigest()==digest
    digest='sha256:'+hashlib.sha256(manifest).hexdigest()
    work=Path(tempfile.mkdtemp(prefix='root-',dir=cache));root=work/'fs';root.mkdir()
    for item in data['layers']:
        layer=cache/(item['digest'].split(':',1)[1]+'.tar')
        if not layer.exists():
            print('Downloading',image,item['size'],flush=True)
            tmp=layer.with_suffix('.part');checksum=hashlib.sha256()
            with urllib.request.urlopen(urllib.request.Request(f'https://registry-1.docker.io/v2/{repo}/blobs/{item["digest"]}',headers=headers),timeout=60) as response,tmp.open('wb') as out:
                while chunk:=response.read(1024*1024):checksum.update(chunk);out.write(chunk)
            assert 'sha256:'+checksum.hexdigest()==item['digest'];tmp.replace(layer)
        with layer.open('rb') as stream:
            assert 'sha256:'+hashlib.file_digest(stream,'sha256').hexdigest()==item['digest']
        def safe(member,dest):
            path=root/member.name
            if not path.resolve().is_relative_to(root.resolve()):raise ValueError('Escaping OCI member')
            name=PurePosixPath(member.name).name
            if name.startswith('.wh.'):
                victim=path.parent/(name[4:])
                victims=list(path.parent.iterdir()) if name=='.wh..wh..opq' and path.parent.exists() else [victim]
                for entry in victims:
                    if entry.is_symlink() or entry.is_file():entry.unlink()
                    elif entry.is_dir():shutil.rmtree(entry)
                return None
            if member.isdev() or member.isfifo():return None
            if member.issym() and member.linkname.startswith('/'):
                member=member.replace(linkname=posixpath.relpath(member.linkname, '/'+str(PurePosixPath(member.name).parent)))
            return tarfile.data_filter(member,dest)
        with tarfile.open(layer) as archive:archive.extractall(root,filter=safe)
    return work,root,digest

def sandbox(root,args,network=False):
    command=['bwrap','--unshare-all']
    if network:command+=['--share-net']
    command+=['--uid','0','--gid','0','--bind',str(root),'/','--proc','/proc','--dev','/dev','--ro-bind',str(ROOT),'/app',
        '--clearenv','--setenv','HOME','/root','--setenv','PATH','/usr/local/bin:/usr/bin:/bin','--setenv','TERM','linux','--setenv','SHELL','/bin/bash','--setenv','FILEX_ROOT','/app']
    return command+args

if __name__=='__main__':
    if sys.version_info < (3,12):raise SystemExit('Distro smoke helper requires Python 3.12+; the explorer itself does not.')
    parser=argparse.ArgumentParser();parser.add_argument('--image',action='append');parser.add_argument('--install-tools',action='store_true');args=parser.parse_args()
    if not shutil.which('bwrap'):raise SystemExit('bubblewrap required; see README. Python 3.12+ required for safe OCI extraction.')
    cache=Path.home()/'.cache/filex-tools/distro-smoke';cache.mkdir(parents=True,exist_ok=True)
    results=[]
    for image in args.image or IMAGES:
        work,root,digest=rootfs(image,cache)
        try:
            if args.install_tools:
                resolv=root/'etc/resolv.conf'
                if resolv.is_symlink():resolv.unlink()
                resolv.write_text(Path('/etc/resolv.conf').read_text())
                setup='if command -v apt-get >/dev/null; then apt-get -o APT::Sandbox::User=root update -qq && DEBIAN_FRONTEND=noninteractive apt-get -o APT::Sandbox::User=root install -y -qq python3 libglib2.0-bin nano zsh; else dnf -y install python3 glib2 nano zsh; fi'
                subprocess.run(sandbox(root,['/bin/bash','-c',setup],True),check=True,timeout=300,stdout=subprocess.DEVNULL)
            # Run a runtime subset against this userspace; test fixtures never use host HOME.
            smoke='''set -eu
/app/target/release/filex --version
mkdir -p /root/files
printf payload > /root/files/note.js
/app/target/release/filex /root/files --print-tree --ascii
if command -v python3 >/dev/null; then
  /app/scripts/install.sh
  bash --login -c 'command -v filex; filex --version'
  SHELL=/bin/zsh python3 /app/scripts/install_alias.py --shell zsh
  zsh -lic 'whence -w filex; filex --version'
  python3 /app/scripts/check-system.py
  python3 /app/scripts/distro-tty-smoke.py
fi
'''
            completed=subprocess.run(sandbox(root,['/bin/bash','-c',smoke]),check=True,text=True,capture_output=True,timeout=60)
            release=(root/'etc/os-release').read_text()
            results.append({'image':image,'manifest_digest':digest,'os_release':release,'passed':True,'binary_sha256':hashlib.sha256((ROOT/'target/release/filex').read_bytes()).hexdigest(),'stdout':completed.stdout})
            print('PASS',image,flush=True)
        finally:shutil.rmtree(work)
    (ROOT/'distro-validation.json').write_text(json.dumps({'scope':'Rootless official Linux userspace on host kernel; not full VM/VT hardware validation','results':results},ensure_ascii=False,indent=2)+'\n')
