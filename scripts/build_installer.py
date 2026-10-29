#!/usr/bin/env python3
"""Embed a verified runtime ZIP in the Rust-only Windows GUI installer."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import zipfile
from package_horizon_native import ROOT, VERSION, pe_amd64


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', help='Cross target, e.g. x86_64-pc-windows-gnu')
    parser.add_argument('--payload', type=Path, default=ROOT / f'build/Rusted-Native-Windows-{VERSION}.zip')
    args = parser.parse_args()
    with zipfile.ZipFile(args.payload) as archive:
        if archive.testzip() is not None:
            raise ValueError('Paquet corrompu')
        if 'Rusted/Configurer-Rusted.exe' not in archive.namelist():
            raise ValueError('Configurateur graphique absent du paquet')
    env = os.environ.copy()
    env['RUSTED_INSTALL_PAYLOAD'] = str(args.payload.resolve())
    command = ['cargo', 'build', '--locked', '--release', '-p', 'rusted-launch', '--bin', 'rusted-setup']
    if args.target:
        command += ['--target', args.target]
    subprocess.run(command, cwd=ROOT, env=env, check=True)
    binary = ROOT / 'target' / (args.target or '') / 'release/rusted-setup.exe'
    pe_amd64(binary)
    destination = ROOT / f'build/Installer-Rusted-Windows-{VERSION}.exe'
    if destination.exists():
        raise FileExistsError(destination)
    shutil.copyfile(binary, destination)
    print(destination, destination.stat().st_size, 'bytes')


if __name__ == '__main__':
    main()
