#!/usr/bin/env python3
"""Package our launcher adapter only; no game, Minecraft resources or Java mods."""
import argparse
import hashlib
import json
from pathlib import Path
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def package(binary, output):
    binary = binary.resolve()
    data = binary.read_bytes()
    if data[:2] != b"MZ":
        raise ValueError("Le binaire doit etre un executable Windows x86_64.")
    pe = int.from_bytes(data[60:64], "little")
    if data[pe:pe + 4] != b"PE\0\0" or int.from_bytes(data[pe + 4:pe + 6], "little") != 0x8664:
        raise ValueError("Le binaire doit etre un PE Windows AMD64.")
    config = json.loads((ROOT / "horizon/rusted-horizon.example.json").read_text())
    config["mode"] = "java"  # usable without a native game; switch explicitly after preparation
    payloads = {
        "rusted-launch.exe": data,
        "rusted-horizon.json": (json.dumps(config, indent=2) + "\n").encode(),
        "rusted-horizon.native.example.json": (ROOT / "horizon/rusted-horizon.example.json").read_bytes(),
        "LISEZ-MOI.md": (ROOT / "docs/HORIZON-WINDOWS.md").read_bytes(),
        "COMPATIBILITE.md": (ROOT / "docs/HORIZON-COMPATIBILITY.md").read_bytes(),
        "LICENSE": (ROOT / "LICENSE").read_bytes(),
        "THIRD_PARTY.md": (ROOT / "THIRD_PARTY.md").read_bytes(),
        "upstream.lock.json": (ROOT / "horizon/upstream.lock.json").read_bytes(),
    }
    # Notices of every Cargo registry dependency, without redistributing their code.
    import subprocess
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1"], cwd=ROOT))
    for dep in metadata["packages"]:
        if dep["source"] is None:
            continue
        folder = Path(dep["manifest_path"]).parent
        for path in folder.iterdir():
            if path.is_file() and path.name.upper().startswith(("LICENSE", "COPYING", "NOTICE")):
                payloads[f"licenses/{dep['name']}-{dep['version']}/{path.name}"] = path.read_bytes()
    manifest = {"schema": 1, "kind": "launcher-adapter-development-kit", "default_mode": "java", "includes_native_game": False,
                "sha256": {name: hashlib.sha256(body).hexdigest() for name, body in payloads.items()}}
    payloads["package.json"] = (json.dumps(manifest, indent=2) + "\n").encode()
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, "x", compression=zipfile.ZIP_DEFLATED, strict_timestamps=False) as archive:
        for name, body in payloads.items():
            archive.writestr(name, body)
    with zipfile.ZipFile(output) as archive:
        if archive.testzip() is not None:
            raise ValueError("Archive corrompue.")
        for name, expected in manifest["sha256"].items():
            if hashlib.sha256(archive.read(name)).hexdigest() != expected:
                raise ValueError("Empreinte d'archive incorrecte.")
    print(f"{output} : passerelle Windows ; jeu natif et essais Windows non inclus.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/x86_64-pc-windows-gnu/release/rusted-launch.exe")
    parser.add_argument("--output", type=Path, default=ROOT / "build/Rusted-Horizon-Bridge-Windows-0.1.0.zip")
    args = parser.parse_args()
    package(args.binary, args.output)
