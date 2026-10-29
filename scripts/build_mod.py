#!/usr/bin/env python3
"""Package our Fabric adapter + native libraries; never packages Minecraft files.

Targets Fabric's production intermediary namespace directly. Loom is unnecessary
for this one hook: its only game references are verified intermediary strings.
The resulting mod still MUST be tested in a real, authorized Minecraft instance.
"""
import argparse
import hashlib
import json
import os
import platform
from pathlib import Path
import shutil
import ssl
import subprocess
import urllib.request
import urllib.error
import zipfile

ROOT = Path(__file__).resolve().parents[1]
DEPS = {
    **{name: f"https://repo.maven.apache.org/maven2/io/netty/netty-{name}/4.1.97.Final/netty-{name}-4.1.97.Final.jar" for name in ("common", "buffer", "transport")},
    "mixin": "https://maven.fabricmc.net/net/fabricmc/sponge-mixin/0.13.3+mixin.0.8.5/sponge-mixin-0.13.3+mixin.0.8.5.jar",
}

def download(url, maximum):
    request = urllib.request.Request(url, headers={"User-Agent": "Rusted-build/0.1"})
    with urllib.request.urlopen(request, context=ssl.create_default_context(), timeout=90) as response:
        if response.geturl() != url:
            raise RuntimeError("Unexpected dependency redirect: " + response.geturl())
        data = response.read(maximum + 1)
    if len(data) > maximum:
        raise RuntimeError("Dependency exceeds limit")
    return data

def dependencies():
    directory = ROOT / "build/dependencies"
    directory.mkdir(parents=True, exist_ok=True)
    paths, records = [], []
    # Official Maven SHA-1 sidecars are fetched over verified TLS. No checksum override.
    for name, url in DEPS.items():
        path = directory / (name + ".jar")
        expected = download(url + ".sha1", 1024).decode("ascii").strip().split()[0].lower()
        if len(expected) != 40 or any(c not in "0123456789abcdef" for c in expected):
            raise RuntimeError("Invalid repository checksum: " + name)
        data = path.read_bytes() if path.exists() else download(url, 8 * 1024 * 1024)
        if hashlib.sha1(data).hexdigest() != expected:
            raise RuntimeError("Dependency SHA-1 mismatch: " + name)
        path.write_bytes(data)
        paths.append(path)
        records.append({"name": name, "url": url, "sha1": expected, "sha256": hashlib.sha256(data).hexdigest()})
    (directory / "provenance.json").write_text(json.dumps(records, indent=2), encoding="utf-8")
    return paths

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--linux", type=Path, help="Path to release librusted_backend.so")
    parser.add_argument("--windows", type=Path, help="Path to release rusted_backend.dll")
    parser.add_argument("--host", action="store_true", help="Package the current host's target/release native library")
    args = parser.parse_args()
    if args.host:
        if args.linux or args.windows:
            parser.error("Use --host or explicit platform paths, not both")
        if platform.system() == "Windows":
            args.windows = ROOT / "target/release/rusted_backend.dll"
        elif platform.system() == "Linux":
            args.linux = ROOT / "target/release/librusted_backend.so"
        else:
            parser.error("Only Windows/Linux hosts are currently supported")
    if not args.linux and not args.windows:
        parser.error("Supply --host or a compiled native library (--linux / --windows)")
    paths = dependencies()
    classes = ROOT / "build/mod-classes"
    if classes.exists():
        shutil.rmtree(classes)
    classes.mkdir(parents=True)
    sources = sorted((ROOT / "java/src/main/java").rglob("*.java"))
    compiler = ["javac"] if shutil.which("javac") else ["java", "com.sun.tools.javac.Main"]
    subprocess.run([*compiler, "--release", "17", "-proc:none", "-cp", os.pathsep.join(map(str, paths)),
                    "-d", str(classes), *map(str, sources)], cwd=ROOT, check=True)
    jar = ROOT / "build/rusted-backend-0.1.0-mc1.20.4.jar"
    # Registry archives can give license files a Unix-epoch timestamp. Clamp
    # ZIP timestamps to its supported range without changing file contents.
    # Write a temporary archive so a failed build never looks like a final mod.
    temporary = jar.with_suffix(".jar.tmp")
    with zipfile.ZipFile(temporary, "w", zipfile.ZIP_DEFLATED, strict_timestamps=False) as output:
        for item in classes.rglob("*.class"):
            output.write(item, item.relative_to(classes).as_posix())
        resources = ROOT / "java/src/main/resources"
        for item in resources.rglob("*"):
            if item.is_file():
                output.write(item, item.relative_to(resources).as_posix())
        output.write(ROOT / "LICENSE", "LICENSE")
        output.write(ROOT / "THIRD_PARTY.md", "META-INF/rusted/THIRD_PARTY.md")
        metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1"], cwd=ROOT))
        for package in metadata["packages"]:
            if package["name"] == "rusted-backend":
                continue
            directory = Path(package["manifest_path"]).parent
            for notice in directory.iterdir():
                if notice.is_file() and notice.name.lower().startswith(("license", "copying", "copyright", "notice")):
                    output.write(notice, f"META-INF/rusted/licenses/{package['name']}-{package['version']}/{notice.name}")
        for platform, path, name in (("linux", args.linux, "librusted_backend.so"), ("windows", args.windows, "rusted_backend.dll")):
            if path:
                resource = f"native/{platform}-x86_64/{name}"
                output.write(path, resource)
                output.writestr(resource + ".sha256", hashlib.sha256(path.read_bytes()).hexdigest())
    temporary.replace(jar)
    print("Packaged", jar)
    print("No Minecraft code/resources included. Actual game injection NOT validated by this build.")

if __name__ == "__main__":
    try:
        main()
    except urllib.error.URLError as error:
        raise SystemExit(f"Dependency access failed: {error}. Required repositories: maven.fabricmc.net and repo.maven.apache.org; review the environment network settings. No verification has been disabled.")
