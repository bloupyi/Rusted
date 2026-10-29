#!/usr/bin/env python3
"""Download the official client and assets into an isolated development cache.

No account tokens are requested or stored. This prepares files, not entitlement
to authenticated multiplayer. No Minecraft binary/resource is added to source.
"""
import argparse
import concurrent.futures
import json
from pathlib import Path, PurePosixPath
import platform
import re
import zipfile

from prepare_minecraft import ROOT, VERSION, LOADER, artifact, digest, get, prepare

def allowed(library):
    rules = library.get("rules", [])
    result = not rules
    os_name = {"Linux": "linux", "Windows": "windows", "Darwin": "osx"}[platform.system()]
    # The manifest lists multiple Windows/macOS native architectures with only
    # an OS rule. Do not flatten an ARM/x86 DLL over the selected x86_64 one.
    parts = library["name"].split(":")
    if len(parts) == 4 and parts[-1].startswith("natives-"):
        expected = {"linux": "natives-linux", "windows": "natives-windows", "osx": "natives-macos"}[os_name]
        if parts[-1] != expected:
            return False
    for rule in rules:
        os_rule = rule.get("os", {})
        if os_rule.get("name", os_name) != os_name:
            continue
        if "arch" in os_rule and not re.search(os_rule["arch"], platform.machine()):
            continue
        if "version" in os_rule and not re.search(os_rule["version"], platform.release()):
            continue
        result = rule["action"] == "allow"
    return result

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prepared-server", action="store_true")
    parser.add_argument("--workers", type=int, default=8)
    args = parser.parse_args()
    if platform.system() not in ("Linux", "Windows"):
        parser.error("The initial native backend supports Linux and Windows only")
    if platform.machine().lower() not in ("amd64", "x86_64"):
        parser.error("This initial client preparation supports x86_64 only")
    if not 1 <= args.workers <= 12:
        parser.error("Use 1 to 12 bounded download workers")
    if not args.prepared_server:
        prepare()
    cache = ROOT / "build/minecraft"
    version = json.loads((cache / "version.json").read_text())
    client = version["downloads"]["client"]
    path = artifact(client["url"], cache / "client.jar", client["sha1"], size=client["size"])
    classpath = [str(path.resolve())]
    natives = cache / "natives" / platform.system().lower()
    natives.mkdir(parents=True, exist_ok=True)
    for library in version["libraries"]:
        if not allowed(library):
            continue
        info = library["downloads"]["artifact"]
        relative = PurePosixPath(info["path"])
        if relative.is_absolute() or ".." in relative.parts:
            raise ValueError("Invalid library path")
        path = artifact(info["url"], cache / "client-libraries" / str(relative), info["sha1"], size=info["size"])
        classpath.append(str(path.resolve()))
        if ":natives-" in library["name"]:
            with zipfile.ZipFile(path) as archive:
                for name in archive.namelist():
                    if not name.lower().endswith((".so", ".dll", ".dylib")):
                        continue
                    safe = PurePosixPath(name)
                    if safe.is_absolute() or ".." in safe.parts:
                        raise ValueError("Invalid native library path")
                    # Extraction is flat and controlled, never archive.extractall().
                    (natives / safe.name).write_bytes(archive.read(name))

    asset_index = version["assetIndex"]
    assets = cache / "assets"
    index = artifact(asset_index["url"], assets / "indexes" / (asset_index["id"] + ".json"), asset_index["sha1"], size=asset_index["size"])
    objects = json.loads(index.read_text())["objects"]
    unique = {entry["hash"]: entry["size"] for entry in objects.values()}
    print(f"Official asset objects: {len(unique)}, {sum(unique.values()) / 1024**2:.1f} MiB. Workers: {args.workers}", flush=True)
    def fetch_asset(item):
        sha1, size = item
        target = assets / "objects" / sha1[:2] / sha1
        artifact("https://resources.download.minecraft.net/" + sha1[:2] + "/" + sha1, target, sha1, size=size)
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.workers) as pool:
        list(pool.map(fetch_asset, unique.items()))

    fabric = json.loads(get(f"https://meta.fabricmc.net/v2/versions/loader/{VERSION}/{LOADER}/profile/json"))
    for library in fabric["libraries"]:
        group, name, version_id = library["name"].split(":")
        relative = f"{group.replace('.', '/')}/{name}/{version_id}/{name}-{version_id}.jar"
        url = library["url"].rstrip("/") + "/" + relative
        checksum = library.get("sha256")
        algorithm = "sha256" if checksum else "sha1"
        if not checksum:
            checksum = get(url + ".sha1", 1024).decode("ascii").strip().split()[0].lower()
        path = artifact(url, cache / "fabric-libraries" / relative, checksum, algorithm, library.get("size"))
        classpath.append(str(path.resolve()))
    runtime = {"version": VERSION, "loader": LOADER, "main_class": fabric["mainClass"],
               "classpath": classpath, "assets": str(assets.resolve()), "asset_index": asset_index["id"],
               "natives": str(natives.resolve()), "client_sha1": client["sha1"], "asset_objects": len(unique)}
    (cache / "client-runtime.json").write_text(json.dumps(runtime, indent=2))
    print("Official client and complete asset index verified. No account authentication performed.")

if __name__ == "__main__":
    main()
