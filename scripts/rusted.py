#!/usr/bin/env python3
"""Build Rusted from its maintained engine sources, and show changes against Horizon."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

from horizon import ROOT, TARGET, LOCK, build_environment
TARGET = Path(os.environ.get("RUSTED_TARGET_DIR", TARGET)).resolve()

SOURCE = ROOT / "engine"
BASELINE = ROOT / "horizon/source-baseline.json"


def source_files():
    # Engine files only: never prepare or distribute a player's resources as source.
    return sorted((p for p in SOURCE.rglob("*") if p.is_file() and
                  not any(part in {".local", "target", ".git", "__pycache__"} for part in p.relative_to(SOURCE).parts)),
                  key=lambda p: p.relative_to(SOURCE).as_posix())


def source_hash():
    digest = hashlib.sha256()
    for path in source_files():
        digest.update(path.relative_to(SOURCE).as_posix().encode() + b"\0")
        digest.update(hashlib.sha256(path.read_bytes()).digest())
    return digest.hexdigest()


def verify():
    if hashlib.sha256((SOURCE / "LICENSE").read_bytes()).hexdigest() != LOCK["license_sha256"]:
        raise ValueError("La licence MIT d'origine doit rester intacte dans engine/LICENSE.")
    if not (SOURCE / "crates/rusted-platform/src/lib.rs").is_file():
        raise ValueError("Sources du module Rusted absentes.")


def differences():
    baseline = json.loads(BASELINE.read_text())["sha256"]
    actual = {p.relative_to(SOURCE).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest() for p in source_files()}
    return {"modified": [p for p in actual if p in baseline and actual[p] != baseline[p]],
            "added": [p for p in actual if p not in baseline], "removed": [p for p in baseline if p not in actual]}


def build(target=None):
    verify()
    before = source_hash()
    env = build_environment()
    args = ["cargo", "+" + LOCK["rust_toolchain"], "build", "--manifest-path", SOURCE / "Cargo.toml",
            "--locked", "--release", "--target-dir", TARGET, "-j", "3", "-p", "horizon-client", "--bin", "rusted",
            "-p", "horizon-prepare", "--bin", "horizon-prepare", "--features", "horizon-client/mods"]
    if target:
        args.extend(["--target", target])
    if target == "x86_64-pc-windows-gnu" and os.name != "nt":
        mingw = Path("/workspace/.rusted-tools/mingw/usr/bin")
        if mingw.is_dir():
            env["PATH"] = str(mingw) + os.pathsep + env["PATH"]
            env["CC_x86_64_pc_windows_gnu"] = str(mingw / "x86_64-w64-mingw32-gcc-posix")
            env["AR_x86_64_pc_windows_gnu"] = str(mingw / "x86_64-w64-mingw32-ar")
            env["CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER"] = env["CC_x86_64_pc_windows_gnu"]
    subprocess.run([str(a) for a in args], check=True, env=env, cwd=ROOT)
    if source_hash() != before:
        raise ValueError("Sources changees pendant la compilation ; relancer avant de distribuer.")
    folder = TARGET / target / "release" if target else TARGET / "release"
    extension = ".exe" if (target and "windows" in target) or (not target and os.name == "nt") else ""
    receipt = {"schema": 1, "project": "Rusted", "version": "0.3.6", "source_sha256": before,
               "upstream_revision": LOCK["revision"], "changes": differences(),
               "binaries": {name + extension: hashlib.sha256((folder / (name + extension)).read_bytes()).hexdigest()
                            for name in ["rusted", "horizon-prepare"]}}
    (folder / "rusted-build.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print("Rusted source build verified:", folder)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("changes")
    sub.add_parser("verify")
    build_parser = sub.add_parser("build")
    build_parser.add_argument("--target")
    args = parser.parse_args()
    verify()
    if args.command == "changes":
        print(json.dumps(differences(), indent=2))
    elif args.command == "build":
        build(args.target)
    else:
        print("Rusted sources:", source_hash())


if __name__ == "__main__":
    main()
