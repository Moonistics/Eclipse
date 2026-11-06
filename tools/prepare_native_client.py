#!/usr/bin/env python3
"""Prepare private pinned resources for Eclipse's native development client.

The preparation itself is eclipse-prepare (crates/eclipse-prepare), the Rust port of what this
script did; this wrapper keeps its command line for the development tools that call it. It runs
the eclipse-prepare beside this folder (a release folder), else builds it with cargo. Arguments
are passed through: --prepare, --server-jar PATH, --minecraft-dir PATH.
"""
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def main():
    arguments = ['--root', str(ROOT), *sys.argv[1:]]
    beside = ROOT / ('eclipse-prepare.exe' if os.name == 'nt' else 'eclipse-prepare')
    if beside.is_file():
        command = [str(beside), *arguments]
    else:
        command = ['cargo', 'run', '--quiet', '--release', '-p', 'eclipse-prepare', '--', *arguments]
    return subprocess.call(command, cwd=ROOT)


if __name__ == '__main__':
    sys.exit(main())
