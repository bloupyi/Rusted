#!/usr/bin/env python3
"""Compile only original Rusted launcher glue against the public Fabric Loader API."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1]
VERSION = '0.3.5'
DEST = ROOT / 'build/bootstrap'
LOCK = json.loads((ROOT / 'bootstrap-java/loader.lock.json').read_text())


def build(loader=None):
    DEST.mkdir(parents=True, exist_ok=True)
    if loader is None:
        loader = DEST / 'fabric-loader-api.jar'
        if not loader.exists():
            request = urllib.request.Request(LOCK['url'], headers={'User-Agent':'Rusted-build/' + VERSION})
            with urllib.request.urlopen(request, timeout=40) as response:
                body = response.read(4 * 1024 * 1024 + 1)
            if hashlib.sha256(body).hexdigest() != LOCK['sha256']:
                raise ValueError('Fabric API binary does not match the pinned digest')
            loader.write_bytes(body)
    if hashlib.sha256(loader.read_bytes()).hexdigest() != LOCK['sha256']:
        raise ValueError('Unexpected Fabric Loader API binary')
    classes = DEST / 'classes'
    if classes.exists(): shutil.rmtree(classes)
    classes.mkdir()
    sources = sorted((ROOT / 'bootstrap-java/src/main/java').rglob('*.java'))
    compiler = ['javac'] if shutil.which('javac') else ['java', 'com.sun.tools.javac.Main']
    subprocess.run([*compiler, '--release', '17', '-proc:none', '-cp', str(loader), '-d', str(classes), *map(str,sources)],check=True,cwd=ROOT)
    metadata = {'schemaVersion':1,'id':'rusted_bootstrap','version':VERSION,'name':'Rusted automatic startup',
                'description':'Starts the Rusted native backend from an imported pack. Java mods use the original JVM.',
                'environment':'client','license':'MIT','icon':'rusted-icon.png',
                'entrypoints':{'preLaunch':['dev.rusted.bootstrap.RustedBootstrap']},
                'depends':{'fabricloader':'>=' + LOCK['version'],'minecraft':LOCK['minecraft'],'java':'>=25'}}
    members = {'fabric.mod.json':(json.dumps(metadata,indent=2)+'\n').encode(),
               'LICENSE':(ROOT/'LICENSE').read_bytes(),'rusted-icon.png':(ROOT/'artwork/rusted-icon.png').read_bytes()}
    for p in sorted(classes.rglob('*.class')):
        name=p.relative_to(classes).as_posix()
        if not name.startswith('dev/rusted/bootstrap/'): raise ValueError('Non-original class in bootstrap')
        members[name]=p.read_bytes()
    jar=DEST/'rusted-bootstrap.jar'
    with zipfile.ZipFile(jar,'w',compression=zipfile.ZIP_DEFLATED) as archive:
        for name,body in sorted(members.items()):
            info=zipfile.ZipInfo(name,date_time=(1980,1,1,0,0,0));info.compress_type=zipfile.ZIP_DEFLATED
            archive.writestr(info,body)
    record={'schema':1,'version':VERSION,'jar_sha256':hashlib.sha256(jar.read_bytes()).hexdigest(),
            'api_only_compile_dependency':LOCK,'includes_minecraft_classes':False,'includes_third_party_classes':False,
            'source_sha256':{p.relative_to(ROOT).as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in sources}}
    (DEST/'bootstrap.json').write_text(json.dumps(record,indent=2)+'\n')
    print('Original Rusted bootstrap:',jar,jar.stat().st_size,'bytes')
    return jar


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--loader-jar',type=Path)
    args=parser.parse_args();build(args.loader_jar)


if __name__=='__main__':main()
