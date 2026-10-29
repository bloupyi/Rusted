#!/usr/bin/env python3
"""Get original precompiled Horizon exporters; verify release SHA256 and exact source key."""
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import tempfile
import urllib.request
import zipfile

from horizon import ROOT
from rusted import SOURCE, verify

ASSET = json.loads((ROOT / "horizon/release-assets.lock.json").read_text())["exporters"]
DESTINATION = ROOT / "build/horizon/original-exporters"


def source_key():
    digest = hashlib.sha256()
    digest.update(b"exporters-v1 --release -proc:none -encoding UTF-8")
    digest.update((SOURCE / "crates/horizon-prepare/src/exporters.rs").read_bytes().replace(b"\r", b""))
    paths = sorted((SOURCE / "mods/horizon/src/test/java/dev/horizonlab/parity").rglob("*.java"), key=lambda p: p.relative_to(SOURCE).as_posix())
    paths.append(SOURCE / "research-lock.json")
    for path in paths:
        body = path.read_bytes().replace(b"\r", b"")
        digest.update(f"{path.relative_to(SOURCE).as_posix()}\0{len(body)}\0".encode())
        digest.update(body)
    return digest.hexdigest()


def prepare():
    verify()
    if source_key() != ASSET["source_key"]:
        raise ValueError("Les exporters de la release ne correspondent pas exactement a la source epinglee.")
    archive = ROOT / "build/horizon/upstream-windows-v0.2.93.zip"
    if not archive.exists() or hashlib.sha256(archive.read_bytes()).hexdigest() != ASSET["sha256"]:
        archive.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.NamedTemporaryFile(dir=archive.parent, delete=False) as temporary:
            temporary_path = Path(temporary.name)
            try:
                with urllib.request.urlopen(ASSET["url"], timeout=120) as response:
                    shutil.copyfileobj(response, temporary)
                temporary.close()
                if hashlib.sha256(temporary_path.read_bytes()).hexdigest() != ASSET["sha256"]:
                    raise ValueError("Empreinte de la release incorrecte.")
                temporary_path.replace(archive)
            finally:
                temporary_path.unlink(missing_ok=True)
    with zipfile.ZipFile(archive) as source:
        prefix = "horizon-windows-x64/exporters/"
        manifest_bytes = source.read(prefix + "manifest.json")
        manifest = json.loads(manifest_bytes)
        if manifest["key"] != ASSET["source_key"] or len(manifest["classes"]) != ASSET["classes"]:
            raise ValueError("Manifest des exporters incorrect.")
        for name, expected in manifest["classes"].items():
            path = PurePosixPath(name)
            if path.is_absolute() or ".." in path.parts or not name.endswith(".class") or not name.startswith(("server/dev/horizonlab/parity/", "client/dev/horizonlab/parity/")):
                raise ValueError("Classe autre que les exporters originaux Horizon.")
            data = source.read(prefix + name)
            if hashlib.sha256(data).hexdigest() != expected:
                raise ValueError("Classe d'export corrompue.")
            target = DESTINATION / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        (DESTINATION / "manifest.json").write_bytes(manifest_bytes)
    print(f"{len(manifest['classes'])} classes originales verifiees, correspondant exactement a la source Horizon epinglee.")


if __name__ == "__main__":
    prepare()
