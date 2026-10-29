#!/usr/bin/env python3
"""Validate full Prism and Modrinth imports: loader + original bootstrap + native payload, no wrapper/helper."""
import hashlib
import io
import json
from pathlib import Path
import zipfile
from package_horizon_native import ROOT, VERSION


def check(archive, prefix, jar_name):
    record=json.loads(archive.read(prefix+'bootstrap.json'))
    bootstrap=archive.read(jar_name)
    assert hashlib.sha256(bootstrap).hexdigest()==record['jar_sha256']
    assert bootstrap==archive.read(prefix+'bootstrap/rusted-bootstrap.jar')
    with zipfile.ZipFile(io.BytesIO(bootstrap)) as jar:
        metadata=json.loads(jar.read('fabric.mod.json'))
        assert metadata['id']=='rusted_bootstrap' and metadata['version']==VERSION
        assert metadata['entrypoints']['preLaunch']==['dev.rusted.bootstrap.RustedBootstrap']
        assert all(not n.endswith('.class') or n.startswith('dev/rusted/bootstrap/') for n in jar.namelist())
    integrity=json.loads(archive.read(prefix+'runtime-manifest.json'))
    for name,digest in integrity['files'].items():
        assert hashlib.sha256(archive.read(prefix+name)).hexdigest()==digest,name
    assert archive.read(prefix+'runtime/rusted.exe').startswith(b'MZ')
    assert archive.read(prefix+'rusted-launch.exe').startswith(b'MZ')
    assert prefix+'native-data/shaderpacks/Rusted-Soft-Light.hshader' in archive.namelist()
    assert not any('Configurer-Rusted' in n for n in archive.namelist())
    for name in archive.namelist():
        assert '/.local/' not in name
        if name.endswith('.jar'):assert name in (jar_name,prefix+'bootstrap/rusted-bootstrap.jar')
    assert archive.testzip() is None


def main():
    with zipfile.ZipFile(ROOT/f'build/Rusted-Native-Prism-{VERSION}.zip') as archive:
        components=json.loads(archive.read('mmc-pack.json'))['components']
        assert any(c['uid']=='net.fabricmc.fabric-loader' and c['version']=='0.20.1' for c in components)
        ini=archive.read('instance.cfg').decode()
        assert 'OverrideCommands=true' in ini and '\nWrapperCommand=\n' in ini
        assert '$INST_' not in ini and 'rusted-launch.exe' not in ini
        check(archive,'minecraft/rusted/','minecraft/mods/rusted-bootstrap.jar')
    with zipfile.ZipFile(ROOT/f'build/Rusted-Native-Modrinth-{VERSION}.mrpack') as archive:
        index=json.loads(archive.read('modrinth.index.json'))
        assert index['dependencies']=={'minecraft':'26.3','fabric-loader':'0.20.1'}
        check(archive,'overrides/rusted/','overrides/mods/rusted-bootstrap.jar')
    print('Full import layouts and every runtime hash verified: Fabric + original bootstrap, no wrapper or configurator, both launchers.')


if __name__=='__main__':main()
