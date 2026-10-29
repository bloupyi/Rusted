#!/usr/bin/env python3
"""Build and execute the real JNI bridge with Java, no Minecraft installation needed."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[1]

def run(*args):
    print("+", " ".join(map(str, args)), flush=True)
    subprocess.run(list(map(str, args)), cwd=ROOT, check=True)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--no-build", action="store_true")
    parser.add_argument("--benchmark", action="store_true", help="Measure the native codec versus Java, not the whole game")
    args = parser.parse_args()
    if not args.no_build:
        run("cargo", "build", "--release", "--locked")
    system = platform.system()
    if system not in ("Windows", "Linux") or platform.machine().lower() not in ("amd64", "x86_64"):
        raise SystemExit("Only Windows/Linux x86_64 currently packaged")
    library = "rusted_backend.dll" if system == "Windows" else "librusted_backend.so"
    native = ROOT / "target/release" / library
    classes = ROOT / "build/bridge-classes"
    classes.mkdir(parents=True, exist_ok=True)
    compiler = ["javac"] if shutil.which("javac") else ["java", "com.sun.tools.javac.Main"]
    run(*compiler, "--release", "17", "-d", classes,
        ROOT / "java/src/main/java/dev/rusted/backend/NativeCodec.java",
        ROOT / "java/src/test/java/dev/rusted/backend/NativeCompatibilityTest.java",
        ROOT / "java/src/test/java/dev/rusted/backend/CodecBenchmark.java")
    reports = ROOT / "build/reports"
    reports.mkdir(parents=True, exist_ok=True)
    # Verify the normal packaged-resource extraction + hash path, not only System.load.
    jar = ROOT / "build/bridge-test.jar"
    resource = f"native/{system.lower()}-x86_64/{library}"
    with zipfile.ZipFile(jar, "w", zipfile.ZIP_DEFLATED) as output:
        for item in classes.rglob("*.class"):
            output.write(item, item.relative_to(classes).as_posix())
        output.write(native, resource)
        output.writestr(resource + ".sha256", hashlib.sha256(native.read_bytes()).hexdigest())
    run("java", "-Xcheck:jni", "-cp", jar,
        "dev.rusted.backend.NativeCompatibilityTest", reports / "jni.json")
    run("java", "-Xcheck:jni", "-Drusted.enabled=true", "-Drusted.native.path=" + str(ROOT / "build/absent-library"),
        "-cp", classes, "dev.rusted.backend.NativeCompatibilityTest",
        reports / "fallback.json", "fallback")
    corrupt = ROOT / "build/bridge-bad-checksum.jar"
    with zipfile.ZipFile(jar) as source, zipfile.ZipFile(corrupt, "w", zipfile.ZIP_DEFLATED) as output:
        for item in source.infolist():
            data = source.read(item.filename)
            output.writestr(item.filename, b"0" * 64 if item.filename == resource + ".sha256" else data)
    run("java", "-Xcheck:jni", "-Drusted.enabled=true", "-cp", corrupt,
        "dev.rusted.backend.NativeCompatibilityTest", reports / "checksum.json", "checksum")
    if args.benchmark:
        run("java", "-Xms256m", "-Xmx512m", "-Drusted.native.path=" + str(native.resolve()),
            "-cp", classes, "dev.rusted.backend.CodecBenchmark", reports / "benchmark.json")
    runtime = subprocess.check_output(["java", "-version"], stderr=subprocess.STDOUT, text=True).strip()
    (reports / "environment.json").write_text(json.dumps({
        "os": platform.platform(), "arch": platform.machine(), "python": platform.python_version(),
        "java": runtime, "native_sha256": hashlib.sha256(native.read_bytes()).hexdigest(),
        "minecraft_launched": False, "windows_runtime_tested": system == "Windows"
    }, indent=2), encoding="utf-8")
    print("Bridge verified; no Minecraft launch or full-game compatibility claim.")

if __name__ == "__main__":
    main()
