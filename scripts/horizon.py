#!/usr/bin/env python3
"""Developer entry point for the pinned MIT Horizon source. Never downloads Minecraft."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
LOCK = json.loads((ROOT / "horizon/upstream.lock.json").read_text())
SOURCE = ROOT / "build/horizon/source"
TARGET = ROOT / "build/horizon/target"


def run(args, **kwargs):
    return subprocess.run([str(a) for a in args], check=True, **kwargs)


def output(*args):
    return subprocess.check_output(args, cwd=SOURCE, text=True).strip()


def verify():
    if not (SOURCE / ".git").exists():
        raise ValueError("Source absente : python scripts/horizon.py fetch")
    if output("git", "rev-parse", "HEAD") != LOCK["revision"]:
        raise ValueError("Revision Horizon differente de horizon/upstream.lock.json ; aucun ecrasement effectue.")
    if output("git", "status", "--porcelain", "--untracked-files=no"):
        raise ValueError("Source Horizon modifiee : conserver les changements avant une compilation reproductible.")
    for file, key in [("Cargo.lock", "cargo_lock_sha256"), ("LICENSE", "license_sha256")]:
        if hashlib.sha256((SOURCE / file).read_bytes()).hexdigest() != LOCK[key]:
            raise ValueError(f"Empreinte Horizon incorrecte : {file}")


def fetch():
    if not SOURCE.exists():
        SOURCE.parent.mkdir(parents=True, exist_ok=True)
        run(["git", "init", SOURCE])
        run(["git", "-C", SOURCE, "remote", "add", "origin", LOCK["repository"]])
        run(["git", "-C", SOURCE, "fetch", "--depth", "1", "origin", LOCK["revision"]])
        run(["git", "-C", SOURCE, "checkout", "--detach", "FETCH_HEAD"])
    verify()
    print(f"Horizon epingle : {LOCK['revision']} (MIT). Ressources Minecraft non incluses.")


def cargo(action, packages, release=False):
    verify()
    args = ["cargo", "+" + LOCK["rust_toolchain"], action, "--manifest-path", SOURCE / "Cargo.toml",
            "--locked", "--target-dir", TARGET, "-j", "4"]
    for package in packages:
        args.extend(["-p", package])
    if release:
        args.append("--release")
    return args


def build_environment():
    environment = os.environ.copy()
    # This cloud workspace keeps verified Debian development packages local.
    # Respect installed system packages elsewhere; no elevated install is needed.
    local = Path("/workspace/.rusted-tools/horizon-linux/usr")
    if sys.platform.startswith("linux") and local.is_dir():
        for key, path in [("PKG_CONFIG_PATH", local / "lib/x86_64-linux-gnu/pkgconfig"), ("CPATH", local / "include")]:
            environment[key] = str(path) + (os.pathsep + environment[key] if environment.get(key) else "")
    return environment


def tool(name):
    path = TARGET / "debug" / (name + (".exe" if os.name == "nt" else ""))
    if not path.is_file():
        raise ValueError("Compiler d'abord : python scripts/horizon.py build-tools")
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    for name in ["fetch", "verify", "build-tools", "test-tools", "build-game"]:
        sub.add_parser(name)
    p = sub.add_parser("prepare")
    p.add_argument("--minecraft-dir", required=True, type=Path)
    p.add_argument("--server-jar", required=True, type=Path)
    p = sub.add_parser("inventory")
    p.add_argument("pack", type=Path)
    p.add_argument("--work", type=Path, required=True)
    p = sub.add_parser("check-mod")
    p.add_argument("jar", type=Path)
    p.add_argument("--work", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "fetch":
        fetch()
    elif args.command == "verify":
        verify()
        print("Source Horizon et empreintes verifiees.")
    elif args.command in ["build-tools", "test-tools"]:
        run(cargo("test" if args.command == "test-tools" else "build", ["horizon-port-pack", "horizon-prepare"]), env=build_environment())
    elif args.command == "build-game":
        # Full player build, including sandboxed Rust/WASM code mods.
        run(cargo("build", ["horizon-client"], release=True) + ["--bin", "horizon-client", "--features", "horizon-client/mods"], env=build_environment())
    else:
        verify()
        if args.command == "prepare":
            run([tool("horizon-prepare"), "--root", SOURCE, "--prepare", "--minecraft-dir", args.minecraft_dir.resolve(), "--server-jar", args.server_jar.resolve()])
        else:
            # A fresh work directory prevents accidental mutation of an old import.
            work = args.work.resolve()
            if work.exists():
                raise ValueError("Le dossier --work doit etre nouveau ; les anciens imports sont conserves.")
            if args.command == "inventory":
                run([tool("horizon-port-pack"), "import", args.pack.resolve(), work])
                run([tool("horizon-port-pack"), "inventory", work, "--offline"])
                run([tool("horizon-port-pack"), "report", work])
            else:
                run([tool("horizon-port-pack"), "check", args.jar.resolve(), work])
            print("Analyse statique seulement : aucun code Java charge. Un verdict favorable ne prouve pas la compatibilite en jeu.")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, subprocess.CalledProcessError) as exc:
        print(f"Horizon : {exc}", file=sys.stderr)
        sys.exit(1)
