#!/usr/bin/env python3
"""Compare fishing retrieval against a real 26.3 server and player.

The Java side calls FishingHook.retrieve with seeded hook and table sequences.
The Rust side compares seeded cast velocity, a bobbing tick, catch-timer
transitions and the rod's fallback loot path, plus named-sequence loot and XP.
It does not yet cover the UUID-to-seed relationship.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

try:
    from . import lab
except ImportError:
    import lab


def main():
    work = Path(tempfile.mkdtemp(prefix="fishing-rng-", dir=lab.LOCAL))
    java_options = os.environ.get("JAVA_TOOL_OPTIONS", "")
    env = lab.game_env(
        JAVA_TOOL_OPTIONS=java_options + " -Declipse.fishingReference=true",
        CARGO_TARGET_DIR=str(lab.cargo_target()),
        ECLIPSE_FISHING_TICK_REFERENCE=str(work / "rust-fishing-ticks.json"),
        ECLIPSE_FISHING_BOBBING_REFERENCE=str(work / "rust-fishing-bobbing.json"),
        ECLIPSE_HOME=str(work / "home"),
        ECLIPSE_LAUNCHER_HOME=str(work / "launcher"),
        XDG_DATA_HOME=str(work / "data"),
        XDG_CONFIG_HOME=str(work / "config"),
    )
    subprocess.run(["cargo", "build", "--profile", "fast", "-p", "eclipse-client", "--bin", "eclipse-client"],
                   cwd=lab.ROOT, env=env, check=True)
    binary = lab.cargo_target() / "fast" / ("eclipse-client.exe" if os.name == "nt" else "eclipse-client")
    subprocess.run([sys.executable, str(lab.ROOT / "tools/test_command_exec_parity.py"),
                    "--only", "scoreboard", "--allow-gaps", "--binary", str(binary), "--work-dir", str(work)],
                   cwd=lab.ROOT, env=env, check=True)
    reference = json.loads((work / "fishing-ticks-reference.json").read_text())
    actual = json.loads((work / "rust-fishing-ticks.json").read_text())
    if len(reference) != 25 or len(actual) != 25:
        raise SystemExit(f"expected 25 fishing tick cases: {len(reference)} vs {len(actual)}")
    if reference != actual:
        for expected, got in zip(reference, actual):
            if expected != got:
                raise SystemExit(f"fishing tick differs: vanilla {expected}; Eclipse {got}")
        raise SystemExit(f"fishing tick record count differs: {len(reference)} vs {len(actual)}")
    expected_bobbing = json.loads((work / "fishing-bobbing-reference.json").read_text())
    actual_bobbing = json.loads((work / "rust-fishing-bobbing.json").read_text())
    if len(expected_bobbing) != 5 or len(actual_bobbing) != 5:
        raise SystemExit(f"expected 5 bobbing cases: {len(expected_bobbing)} vs {len(actual_bobbing)}")
    if expected_bobbing != actual_bobbing:
        for expected, actual in zip(expected_bobbing, actual_bobbing):
            if expected != actual:
                raise SystemExit(f"fishing bobbing differs: vanilla {expected}; Eclipse {actual}")
        raise SystemExit(f"fishing bobbing count differs: {len(expected_bobbing)} vs {len(actual_bobbing)}")
    fallback = work / "fallback"
    fallback_env = env.copy()
    fallback_env.pop("ECLIPSE_FISHING_TICK_REFERENCE", None)
    fallback_env.pop("ECLIPSE_FISHING_BOBBING_REFERENCE", None)
    fallback_env["ECLIPSE_FISHING_FALLBACK_REFERENCE"] = str(fallback / "rust-fishing-fallback.json")
    fallback_env["ECLIPSE_FISHING_FALLBACK_FIXTURE"] = "1"
    fallback_env["JAVA_TOOL_OPTIONS"] = java_options + " -Declipse.fishingFallback=true"
    subprocess.run([sys.executable, str(lab.ROOT / "tools/test_command_exec_parity.py"),
                    "--only", "scoreboard", "--allow-gaps", "--binary", str(binary), "--work-dir", str(fallback)],
                   cwd=lab.ROOT, env=fallback_env, check=True)
    expected = json.loads((fallback / "fishing-fallback-reference.json").read_text())
    actual = json.loads((fallback / "rust-fishing-fallback.json").read_text())
    if len(expected) != 5 or len(actual) != 5:
        raise SystemExit(f"expected 5 fallback cases: {len(expected)} vs {len(actual)}")
    if expected != actual:
        for reference_row, actual_row in zip(expected, actual):
            if reference_row != actual_row:
                raise SystemExit(f"fishing fallback differs: vanilla {reference_row}; Eclipse {actual_row}")
        raise SystemExit(f"fishing fallback record count differs: {len(expected)} vs {len(actual)}")
    env["ECLIPSE_FISHING_RESOURCES"] = str(lab.RESOURCES)
    env["ECLIPSE_FISHING_REFERENCE"] = str(work / "fishing-reference.json")
    env["ECLIPSE_FISHING_CAST_REFERENCE"] = str(work / "fishing-casts-reference.json")
    subprocess.run(["cargo", "test", "--profile", "fast", "-p", "eclipse-client", "--lib",
                    "game::fishing::tests::fishing_retrieve_reference", "--", "--exact", "--ignored"],
                   cwd=lab.ROOT, env=env, check=True)
    print("fishing: seeded catch timers, loot and XP match vanilla")


if __name__ == "__main__":
    main()
