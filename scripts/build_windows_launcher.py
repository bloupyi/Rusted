#!/usr/bin/env python3
"""Build the Windows launcher with a static C runtime; no VC++ redist is needed."""
from pathlib import Path
import os
import shutil
import subprocess
from check_launcher_runtime import verify

ROOT = Path(__file__).resolve().parents[1]
TARGET = 'x86_64-pc-windows-msvc'

if __name__ == '__main__':
    if os.name != 'nt':
        raise SystemExit('Release builds require genuine Windows and the MSVC toolchain.')
    subprocess.run(['cargo', 'build', '--release', '--locked', '-p', 'rusted-launcher',
        '--target', TARGET, '--config', f'target.{TARGET}.rustflags=["-C","target-feature=+crt-static"]'],
        cwd=ROOT, check=True)
    output = ROOT / 'target/release/rusted-launcher.exe'
    output.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / f'target/{TARGET}/release/rusted-launcher.exe', output)
    verify(output)
    print('Windows launcher built with the static Microsoft C runtime.')
