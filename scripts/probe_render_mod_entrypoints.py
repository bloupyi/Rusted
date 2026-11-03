#!/usr/bin/env python3
"""Run real pinned render-mod initializers as a component diagnostic, NOT a game compatibility test.

The probe intentionally bypasses player eligibility only in a disposable developer process.
No mixins are applied. No source code or bytecode implementation is inspected.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--probe', type=Path, required=True)
    parser.add_argument('--java-home', type=Path, required=True)
    parser.add_argument('--libraries', type=Path, required=True)
    parser.add_argument('--jars', type=Path, required=True, help='Directory containing the unchanged pinned downloads')
    args = parser.parse_args()
    pins = json.loads((ROOT / 'horizon/render-mod-test-jars.json').read_text())
    (ROOT / 'build').mkdir(exist_ok=True)
    scratch = Path(tempfile.mkdtemp(prefix='real-render-mods-', dir=ROOT / 'build'))
    libraries = scratch / 'libraries.json'
    libraries.write_text(json.dumps([str(p.resolve()) for p in sorted(args.libraries.rglob('*.jar'))]))
    results = []
    for pin in pins['jars']:
        jar = args.jars.resolve() / pin['filename']
        if hashlib.sha512(jar.read_bytes()).hexdigest() != pin['sha512']:
            raise RuntimeError('Expected unchanged published bytes: ' + pin['filename'])
        with zipfile.ZipFile(jar) as archive:
            metadata = json.loads(archive.read('fabric.mod.json'))
        home = scratch / pin['project']
        home.mkdir()
        env = os.environ.copy()
        for key in ['HORIZON_HOME', 'HORIZON_LAUNCHER_HOME', 'XDG_DATA_HOME', 'XDG_CONFIG_HOME',
                    'HORIZON_LAUNCHER_TRASH', 'HORIZON_LAUNCHER_CLIPBOARD', 'HORIZON_LAUNCHER_DESKTOP']:
            env[key] = str(home / key)
        env.update(HORIZON_NO_AUDIO='1', HORIZON_LAUNCHER_NO_SELF_UPDATE='1')
        env.pop('HORIZON_SESSION_FILE', None)
        result = subprocess.run([str(args.probe.resolve()), str(args.java_home.resolve()), str(home),
                                 str(libraries), str(jar)], cwd=home, env=env,
                                capture_output=True, text=True, timeout=60)
        (home / 'component.log').write_text(result.stdout + result.stderr)
        results.append({**pin, 'component_exit_code': result.returncode,
            'component_output': result.stdout + result.stderr,
            'standard_entrypoint_declared': any(metadata.get('entrypoints', {}).get(key) for key in ['preLaunch', 'main', 'client']),
            'required_mixins': metadata.get('mixins', []),
            'access_widener': metadata.get('accessWidener'),
            'nested_jar_count': len(metadata.get('jars', [])),
            'mixins_applied': False, 'native_world_started': False, 'rendering_validated': False,
            'compatibility_established': False})
    report = {'schema': 1, 'kind': 'initializer-component-diagnostic',
              'account_used': False, 'compatibility_established': False, 'mods': results}
    destination = scratch / 'result.json'
    destination.write_text(json.dumps(report, indent=2) + '\n')
    print(destination)
    # Initializer success by itself can never pass a game-compatibility gate.
    raise SystemExit(1)


if __name__ == '__main__':
    main()
