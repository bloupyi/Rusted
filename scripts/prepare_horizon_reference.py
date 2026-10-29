#!/usr/bin/env python3
"""Official reference files in a private development cache. No credentials/EULA/worlds."""
import concurrent.futures
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import tarfile
import urllib.request

from prepare_minecraft import ROOT, artifact, digest

BASE = ROOT / "build/horizon/reference"
GAME = BASE / "minecraft"
MANIFEST_SHA1 = "11f1aefe473eefba0af62e3286c33b692807b2eb"


def fetch(info, path):
    if not path.exists():
        old = ROOT / "build/minecraft"
        candidates = [old / "assets/objects" / path.parent.name / path.name, old / "client-libraries" / path.relative_to(GAME / "libraries") if path.is_relative_to(GAME / "libraries") else BASE / "unused"]
        for source in candidates:
            if source.is_file() and source.stat().st_size == info["size"] and digest(source, "sha1") == info["sha1"]:
                path.parent.mkdir(parents=True, exist_ok=True)
                try:
                    path.hardlink_to(source)
                except OSError:
                    shutil.copyfile(source, path)
                return path
    return artifact(info["url"], path, info["sha1"], size=info["size"])


def main():
    manifest = artifact(f"https://piston-meta.mojang.com/v1/packages/{MANIFEST_SHA1}/26.3.json", GAME / "versions/26.3/26.3.json", MANIFEST_SHA1)
    metadata = json.loads(manifest.read_text())
    fetch(metadata["downloads"]["client"], GAME / "versions/26.3/26.3.jar")
    fetch(metadata["downloads"]["server"], BASE / "server-26.3.jar")
    tasks = []
    for library in metadata["libraries"]:
        info = library.get("downloads", {}).get("artifact")
        if info:
            path = PurePosixPath(info["path"])
            if path.is_absolute() or ".." in path.parts:
                raise ValueError("Invalid library path")
            tasks.append((info, GAME / "libraries" / str(path)))
    index = metadata["assetIndex"]
    path = fetch(index, GAME / "assets/indexes" / (index["id"] + ".json"))
    objects = json.loads(path.read_text())["objects"]
    unique = {value["hash"]: value["size"] for value in objects.values()}
    print(f"Reference 26.3: {len(unique)} asset objects, {sum(unique.values()) / 1024**2:.1f} MiB; private cache only", flush=True)
    for sha1, size in unique.items():
        tasks.append(({"url": f"https://resources.download.minecraft.net/{sha1[:2]}/{sha1}", "sha1": sha1, "size": size}, GAME / "assets/objects" / sha1[:2] / sha1))
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        list(pool.map(lambda entry: fetch(*entry), tasks))
    # A Java runtime is enough: original precompiled exporters are verified separately.
    info_path = BASE / "adoptium-linux.json"
    if not info_path.is_file():
        release = json.load(urllib.request.urlopen("https://api.github.com/repos/adoptium/temurin25-binaries/releases/latest"))
        info = next(a for a in release["assets"] if a["name"].startswith("OpenJDK25U-jre_x64_linux_hotspot_") and a["name"].endswith(".tar.gz"))
        info_path.write_text(json.dumps(info, indent=2))
    jre = json.loads(info_path.read_text())
    expected = jre["digest"].removeprefix("sha256:")
    archive = artifact(jre["browser_download_url"], BASE / jre["name"], expected, "sha256", jre["size"])
    destination = ROOT / "build/horizon/jre25"
    destination.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive) as source:
        source.extractall(destination, filter="data")
    java_home = next(p.parent.parent for p in destination.glob("*/bin/java"))
    record = {"minecraft": str(GAME), "server_jar": str(BASE / "server-26.3.jar"), "java_home": str(java_home), "manifest_sha1": MANIFEST_SHA1,
        "client_sha1": metadata["downloads"]["client"]["sha1"], "server_sha1": metadata["downloads"]["server"]["sha1"], "assets": len(unique),
        "jre_url": jre["browser_download_url"], "jre_sha256": expected, "no_authentication_performed": True, "no_eula_accepted": True}
    (BASE / "reference.json").write_text(json.dumps(record, indent=2) + "\n")
    print("Verified official reference files and Java25 in private cache; no account data or game files distributed.", flush=True)


if __name__ == "__main__":
    main()
