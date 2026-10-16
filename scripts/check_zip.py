#!/usr/bin/env python3
"""Validate archive entries before handing extraction to Bash/unzip."""
import os
import re
import stat
import sys
import zipfile


def check(path):
    with zipfile.ZipFile(path) as archive:
        for item in archive.infolist():
            name = item.filename.replace('\\', '/')
            parts = name.rstrip('/').split('/')
            mode = item.external_attr >> 16
            if (not name or name.startswith('/') or any(p in ('..', '') for p in parts)
                    or any(':' in p for p in parts) or any(ord(c) < 32 for c in name)
                    or stat.S_ISLNK(mode) or (stat.S_IFMT(mode) not in (0, stat.S_IFREG, stat.S_IFDIR))
                    or item.external_attr & 0x400):
                raise ValueError(f'Unsafe or unsupported ZIP entry: {item.filename!r}')
            if os.name == 'nt' and any(p.endswith((' ', '.')) or re.fullmatch(r'(CON|PRN|AUX|NUL|COM[1-9¹²³]|LPT[1-9¹²³])(?:\..*)?', p, re.I) for p in parts):
                raise ValueError(f'Unsupported Windows name: {item.filename!r}')


if __name__ == '__main__':
    try:
        check(sys.argv[1])
    except (IndexError, OSError, ValueError, zipfile.BadZipFile) as error:
        sys.exit(str(error))
