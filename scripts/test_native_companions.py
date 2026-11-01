#!/usr/bin/env python3
"""Run a native HUD companion from a loader JAR using disposable developer fixtures."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import zipfile
from build_native_companion import build

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native', type=Path, required=True, help='Built fps_overlay.hmod test module')
    parser.add_argument('--target-dir', type=Path, default=ROOT / 'build/horizon/target')
    args = parser.parse_args()
    (ROOT / 'build').mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='native-port-test-', dir=ROOT / 'build') as owned:
        scratch = Path(owned)
        api = scratch / 'src/net/fabricmc/api/ClientModInitializer.java'
        api.parent.mkdir(parents=True)
        api.write_text('package net.fabricmc.api; public interface ClientModInitializer { void onInitializeClient(); }')
        java = scratch / 'src/JavaOnly.java'
        java.write_text('public final class JavaOnly implements net.fabricmc.api.ClientModInitializer {'
                        'public void onInitializeClient() { throw new AssertionError("Java must not execute"); }}')
        subprocess.run(['javac', '-d', str(scratch / 'classes'), str(api), str(java)], check=True)
        original = scratch / 'original.jar'
        with zipfile.ZipFile(original, 'w') as archive:
            archive.writestr('fabric.mod.json', json.dumps({'schemaVersion': 1, 'id': 'fps_overlay',
                'version': '1.0', 'entrypoints': {'client': ['JavaOnly']}}))
            archive.writestr('JavaOnly.class', (scratch / 'classes/JavaOnly.class').read_bytes())
        mods = scratch / 'mods'
        mods.mkdir()
        build(original, args.native.resolve(), mods / 'fps-overlay.jar')
        env = os.environ.copy()
        for name in ['HORIZON_HOME', 'HORIZON_LAUNCHER_HOME', 'HORIZON_LAUNCHER_TRASH',
                     'HORIZON_LAUNCHER_CLIPBOARD', 'HORIZON_LAUNCHER_DESKTOP', 'XDG_DATA_HOME', 'XDG_CONFIG_HOME']:
            env[name] = str(scratch / name)
        env.update(HORIZON_LAUNCHER_NO_SELF_UPDATE='1', HORIZON_NO_AUDIO='1')
        subprocess.run(['cargo', '+1.99.1', 'run', '--release', '--locked', '--offline',
            '--manifest-path', str(ROOT / 'engine/Cargo.toml'), '--target-dir', str(args.target_dir.resolve()),
            '-p', 'horizon-mods', '--features', 'runtime', '--example', 'rusted_native_port', '--',
            str(mods), str(scratch / 'cache')], env=env, check=True)


if __name__ == '__main__':
    main()
