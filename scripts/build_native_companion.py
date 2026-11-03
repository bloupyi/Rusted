#!/usr/bin/env python3
"""Developer tool: add an explicitly ported native module to an existing loader JAR.

This does not translate Java. Authors must supply the implemented .hmod themselves.
The original JAR is kept unchanged; the output is a separate dual-runtime archive.
"""
import argparse
import hashlib
import json
from pathlib import Path
import tomllib
import zipfile


def build(source: Path, native: Path, output: Path):
    package = native.read_bytes()
    with zipfile.ZipFile(native) as z:
        manifest = tomllib.loads(z.read('horizon-mod.toml').decode('utf-8'))
        if z.read('mod.wasm')[:8] != b'\0asm\x0d\0\x01\0':
            raise ValueError('Expected a native WASM component, not Java bytecode')
    declaration = json.dumps({'schema': 1, 'minecraft': '26.3', 'id': manifest['id'],
        'package': 'META-INF/rusted/port.hmod',
        'sha256': hashlib.sha256(package).hexdigest()}, indent=2).encode()
    with zipfile.ZipFile(source) as original:
        if any(n.lower().startswith('meta-inf/rusted/') for n in original.namelist()):
            raise ValueError('Source already contains a native companion')
        ids = []
        for name, quilt in [('fabric.mod.json', False), ('quilt.mod.json', True)]:
            if name in original.namelist():
                metadata = json.loads(original.read(name))
                ids.append((metadata['quilt_loader'] if quilt else metadata)['id'])
        for name in ['META-INF/mods.toml', 'META-INF/neoforge.mods.toml']:
            if name in original.namelist():
                mods = tomllib.loads(original.read(name).decode())['mods']
                if len(mods) != 1:
                    raise ValueError('One source mod per native port is supported')
                ids.append(mods[0]['modId'])
        if not ids or any(id != manifest['id'] for id in ids):
            raise ValueError('Native port id must match all source loader identities')
        # Exclusive create prevents modifying the input or replacing an existing export.
        with zipfile.ZipFile(output, 'x', compression=zipfile.ZIP_DEFLATED) as out:
            for entry in original.infolist():
                out.writestr(entry, original.read(entry))
            out.writestr('META-INF/rusted/native.json', declaration)
            out.writestr('META-INF/rusted/port.hmod', package)
    return output


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--native', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    print(build(args.jar, args.native, args.output))
