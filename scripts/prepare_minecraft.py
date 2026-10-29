#!/usr/bin/env python3
"""Prepare official Minecraft 1.20.4 server classes and Fabric for local tests.

No account data, no EULA acceptance, no world, no modification of an installation.
All proprietary artifacts stay in the ignored build directory.
"""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import ssl
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1]
VERSION = "1.20.4"
LOADER = "0.15.11"
MANIFEST = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"

def get(url, limit=8 * 1024 * 1024):
    if not url.startswith("https://"):
        raise ValueError("Only verified HTTPS downloads are supported")
    with urllib.request.urlopen(url, context=ssl.create_default_context(), timeout=120) as response:
        data = response.read(limit + 1)
    if len(data) > limit:
        raise ValueError("Download exceeds limit: " + url)
    return data

def digest(path, algorithm):
    result = hashlib.new(algorithm)
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()

def artifact(url, path, expected, algorithm="sha1", size=None):
    if len(expected) != hashlib.new(algorithm).digest_size * 2 or any(c not in "0123456789abcdef" for c in expected):
        raise ValueError("Invalid artifact digest")
    if path.exists():
        if digest(path, algorithm) != expected or (size is not None and path.stat().st_size != size):
            raise ValueError("Cached artifact mismatch; inspect/remove the bad cache explicitly: " + str(path))
        return path
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".part")
    if temporary.exists():
        raise ValueError("Unfinished download exists; inspect/remove it explicitly: " + str(temporary))
    count = 0
    checksum = hashlib.new(algorithm)
    try:
        with urllib.request.urlopen(url, context=ssl.create_default_context(), timeout=120) as response, temporary.open("xb") as output:
            while chunk := response.read(1024 * 1024):
                count += len(chunk)
                if count > (size if size is not None else 128 * 1024 * 1024):
                    raise ValueError("Artifact exceeds declared/configured limit")
                checksum.update(chunk)
                output.write(chunk)
        if checksum.hexdigest() != expected or (size is not None and count != size):
            raise ValueError("Downloaded artifact digest/size mismatch: " + url)
        temporary.replace(path)
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise
    print("Verified", path.name, count, "bytes", flush=True)
    return path

def prepare():
    cache = ROOT / "build/minecraft"
    cache.mkdir(parents=True, exist_ok=True)
    manifest = json.loads(get(MANIFEST))
    entry = next(v for v in manifest["versions"] if v["id"] == VERSION)
    metadata = artifact(entry["url"], cache / "version.json", entry["sha1"])
    version = json.loads(metadata.read_text())
    (cache / "manifest-entry.json").write_text(json.dumps(entry, indent=2))
    for key, name in (("server", "server.jar"), ("server_mappings", "server-mappings.txt")):
        info = version["downloads"][key]
        artifact(info["url"], cache / name, info["sha1"], size=info["size"])

    fabric_url = f"https://meta.fabricmc.net/v2/versions/loader/{VERSION}/{LOADER}/server/json"
    fabric_bytes = get(fabric_url)
    (cache / "fabric-server.json").write_bytes(fabric_bytes)
    fabric = json.loads(fabric_bytes)
    classpath = []
    provenance = []
    for info in fabric["libraries"]:
        group, name, version_id = info["name"].split(":")
        relative = f"{group.replace('.', '/')}/{name}/{version_id}/{name}-{version_id}.jar"
        url = info["url"].rstrip("/") + "/" + relative
        expected = info.get("sha256")
        algorithm = "sha256" if expected else "sha1"
        if not expected:
            expected = get(url + ".sha1", 1024).decode("ascii").strip().split()[0].lower()
        path = artifact(url, cache / "fabric-libraries" / relative, expected, algorithm, info.get("size"))
        classpath.append(str(path.resolve()))
        provenance.append({"name": info["name"], "url": url, algorithm: expected})

    # The official server bundler includes SHA-256 hashes for every inner JAR.
    # Extract only those enumerated entries; never use extractall or trust paths.
    game = None
    with zipfile.ZipFile(cache / "server.jar") as bundle:
        for category in ("versions", "libraries"):
            records = bundle.read(f"META-INF/{category}.list").decode("utf-8").splitlines()
            for line in records:
                checksum, identity, relative = line.split("\t")
                safe = PurePosixPath(relative)
                if safe.is_absolute() or ".." in safe.parts:
                    raise ValueError("Unsafe bundled artifact path")
                path = cache / "bundled" / category / relative
                if not path.exists():
                    data = bundle.read(f"META-INF/{category}/{relative}")
                    if hashlib.sha256(data).hexdigest() != checksum:
                        raise ValueError("Bundled artifact SHA-256 mismatch")
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(data)
                if digest(path, "sha256") != checksum:
                    raise ValueError("Cached bundled artifact SHA-256 mismatch")
                classpath.append(str(path.resolve()))
                if category == "versions":
                    game = str(path.resolve())
    if game is None:
        raise ValueError("Official server bundle contained no game JAR")
    result = {"version": VERSION, "loader": LOADER, "main_class": fabric["mainClass"],
              "classpath": classpath, "game_jar": game, "fabric_source": fabric_url,
              "fabric_libraries": provenance, "server_sha1": version["downloads"]["server"]["sha1"],
              "eula_accepted_by_script": False}
    (cache / "runtime.json").write_text(json.dumps(result, indent=2))
    print("Official server classes and Fabric prepared. No server/world has been started.")
    return result

if __name__ == "__main__":
    argparse.ArgumentParser(description=__doc__).parse_args()
    prepare()
