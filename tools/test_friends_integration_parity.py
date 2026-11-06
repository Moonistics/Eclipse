#!/usr/bin/env python3
"""Real clients over LAN, direct invitations and a local relay, with a server-thread host.

Registered with parity.py so changes to the drivers and the game binary select
this integration coverage in CI as well as the deterministic library probes.
"""
import json
from pathlib import Path
import subprocess
import sys

import lab
import test_friends
from integration_env import suite_lock


if __name__ == '__main__':
    subprocess.run(['cargo', 'build', '--locked', '--profile', 'fast', '-p', 'eclipse-client', '-p', 'eclipse-client-lib',
                    '-p', 'eclipse-relay', '--bin', 'eclipse-client', '--bin', 'eclipse-ctl',
                    '--bin', 'eclipse-relay'], cwd=lab.ROOT, check=True)
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--format-version', '1', '--no-deps'], cwd=lab.ROOT, text=True))
    bins = Path(metadata['target_directory']) / 'fast'
    sys.argv = [sys.argv[0], '--client', str(bins / 'eclipse-client'),
                '--relay', str(bins / 'eclipse-relay'), '--baseline',
                *(['--offscreen'] if sys.platform == 'darwin' else [])]
    with suite_lock():
        # The wrapper only selects binaries; test_friends isolates each game
        # through integration_env and lab.game_env.
        test_friends.main()
