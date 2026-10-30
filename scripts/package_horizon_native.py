#!/usr/bin/env python3
"""Build native Windows runtime, Prism instance and local Modrinth import without Minecraft files."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import zipfile

from horizon import ROOT, LOCK
from rusted import SOURCE, verify, source_hash
from prepare_horizon_exporters import ASSET, DESTINATION, prepare
from build_bootstrap import DEST as BOOTSTRAP, LOCK as FABRIC

VERSION = "0.3.6"
# Prism 10.0.5 expands INST_MC_DIR to native Windows separators before splitting.
# Its splitter consumes backslashes inside either kind of quotes. Leave this
# variable unquoted for standard paths; paths with spaces need the absolute
# forward-slash command documented in NATIVE-WINDOWS.md.
PRISM_WRAPPER = "$INST_MC_DIR/rusted/rusted-launch.exe --"


def pe_amd64(path):
    body = path.read_bytes()
    offset = int.from_bytes(body[60:64], "little")
    if body[:2] != b"MZ" or body[offset:offset + 4] != b"PE\0\0" or int.from_bytes(body[offset + 4:offset + 6], "little") != 0x8664:
        raise ValueError(f"Pas un executable Windows AMD64 : {path.name}")
    return body


def notices(manifest):
    metadata = json.loads(subprocess.check_output(["cargo", "+" + LOCK["rust_toolchain"], "metadata", "--manifest-path", str(manifest), "--locked", "--format-version", "1"], cwd=ROOT))
    result = {}
    for dependency in metadata["packages"]:
        folder = Path(dependency["manifest_path"]).parent
        for path in folder.iterdir():
            if path.is_file() and path.name.upper().startswith(("LICENSE", "COPYING", "NOTICE")):
                result[f"licenses/crates/{dependency['name']}-{dependency['version']}/{path.name}"] = path.read_bytes()
    return result


def payloads(binary_dir, bridge_binary, setup_binary):
    verify()
    receipt_path = binary_dir / "rusted-build.json"
    receipt = json.loads(receipt_path.read_text())
    if receipt.get("project") != "Rusted" or receipt.get("source_sha256") != source_hash():
        raise ValueError("Compiler les sources Rusted actuelles avant l'empaquetage.")
    for name, expected in receipt["binaries"].items():
        if hashlib.sha256((binary_dir / name).read_bytes()).hexdigest() != expected:
            raise ValueError("Executable different de la compilation Rusted verifiee : " + name)
    prepare()
    bootstrap = BOOTSTRAP / "rusted-bootstrap.jar"
    record = json.loads((BOOTSTRAP / "bootstrap.json").read_text())
    if hashlib.sha256(bootstrap.read_bytes()).hexdigest() != record["jar_sha256"]:
        raise ValueError("Bootstrap different de sa compilation verifiee.")
    for name, expected in record["source_sha256"].items():
        if hashlib.sha256((ROOT / name).read_bytes()).hexdigest() != expected:
            raise ValueError("Recompiler le bootstrap apres modification de ses sources.")
    with zipfile.ZipFile(bootstrap) as own:
        if any(n.endswith(".class") and not n.startswith("dev/rusted/bootstrap/") for n in own.namelist()):
            raise ValueError("Classe tierce interdite dans le bootstrap original.")
    files = {
        "bootstrap/rusted-bootstrap.jar": bootstrap.read_bytes(),
        "bootstrap.json": (BOOTSTRAP / "bootstrap.json").read_bytes(),
        "rusted-launch.exe": pe_amd64(bridge_binary),
        "runtime/rusted-icon.png": (ROOT / "artwork/rusted-icon.png").read_bytes(),
        "runtime/rusted.exe": pe_amd64(binary_dir / "rusted.exe"),
        "runtime/rusted-title.png": (ROOT / "artwork/rusted-title.png").read_bytes(),
        "licenses/Rusted-title-artwork.txt": (ROOT / "artwork/README.md").read_bytes(),
        "runtime/rusted-prepare.exe": pe_amd64(binary_dir / "horizon-prepare.exe"),
        "rusted-build.json": receipt_path.read_bytes(),
        "PROVENANCE.md": (ROOT / "engine/RUSTED.md").read_bytes(),
        "LICENSE": (ROOT / "LICENSE").read_bytes(),
        "licenses/Horizon-MIT.txt": (SOURCE / "LICENSE").read_bytes(),
        "licenses/Horizon-THIRD-PARTY.md": (SOURCE / "THIRD_PARTY_NOTICES.md").read_bytes(),
        "LISEZ-MOI.md": (ROOT / "docs/NATIVE-WINDOWS.md").read_bytes(),
        "COMPATIBILITE.md": (ROOT / "docs/HORIZON-COMPATIBILITY.md").read_bytes(),
        "upstream.lock.json": (ROOT / "horizon/upstream.lock.json").read_bytes(),
        "release-assets.lock.json": (ROOT / "horizon/release-assets.lock.json").read_bytes(),
    }
    # Original MIT preparation code/sources only, from tracked upstream paths.
    selected = [SOURCE / "research-lock.json"]
    for folder in ["mods/horizon/src/test/java", "crates/horizon-prepare"]:
        selected.extend(p for p in (SOURCE / folder).rglob("*") if p.is_file())
    for path in selected:
        files["runtime/preparation/" + path.relative_to(SOURCE).as_posix()] = path.read_bytes()
    manifest = json.loads((DESTINATION / "manifest.json").read_text())
    for name, expected in manifest["classes"].items():
        body = (DESTINATION / name).read_bytes()
        if hashlib.sha256(body).hexdigest() != expected:
            raise ValueError("Exporter modifie depuis sa verification.")
        files["runtime/preparation/exporters/" + name] = body
    files["runtime/preparation/exporters/manifest.json"] = (DESTINATION / "manifest.json").read_bytes()
    for path in (SOURCE / "crates/horizon-theme").rglob("*"):
        if path.is_file() and path.name.startswith(("OFL", "LICENSE")):
            files["licenses/" + path.relative_to(SOURCE).as_posix()] = path.read_bytes()
    files.update(notices(ROOT / "Cargo.toml"))
    files.update(notices(SOURCE / "Cargo.toml"))
    shader = ROOT / "build/Rusted-Soft-Light.hshader"
    if not shader.is_file():
        raise ValueError("Construire et verifier Rusted Soft Light avant le pack complet.")
    files["native-data/shaderpacks/Rusted-Soft-Light.hshader"] = shader.read_bytes()
    files["SHADERS.md"] = (ROOT / "docs/SHADERS.md").read_bytes()
    native_mods = ROOT / "build/horizon/native-mods"
    for path in native_mods.glob("*.hmod"):
        files["native-data/horizon-mods/" + path.name] = path.read_bytes()
    # This is a new native data folder we own. Mods remain off in Horizon's UI.
    if any(name.startswith("native-data/") for name in files):
        files["native-data/.rusted-native-instance.json"] = json.dumps({"schema": 1, "horizon_revision": LOCK["revision"]}).encode()
    config = {"schema": 1, "mode": "auto", "horizon_revision": LOCK["revision"], "minecraft_version": LOCK["minecraft_version"],
              "executable": "runtime/rusted.exe", "resources": "runtime/preparation/.local/horizon-client/26.3", "data_directory": "native-data",
              "integrity_manifest": "runtime-manifest.json",
              "preparation": {"root": "runtime/preparation", "executable": "runtime/rusted-prepare.exe", "download_server": True, "import_launcher": True}}
    files["rusted-runtime.json"] = (json.dumps(config, indent=2) + "\n").encode()
    integrity = {"schema": 1, "files": {name: hashlib.sha256(body).hexdigest() for name, body in files.items() if not name.startswith("native-data/") and name != "rusted-runtime.json"}}
    files["runtime-manifest.json"] = (json.dumps(integrity, indent=2) + "\n").encode()
    files["package.json"] = (json.dumps({"schema": 1, "version": VERSION, "native_engine_version": receipt["version"], "kind": "native-windows-runtime", "includes_native_game": True,
        "includes_minecraft_files": False, "minecraft_version": LOCK["minecraft_version"], "upstream_revision": LOCK["revision"],
        "exporter_source_key": ASSET["source_key"], "windows_hardware_gameplay_verified": False,
        "sha256": {name: hashlib.sha256(body).hexdigest() for name, body in files.items()}}, indent=2) + "\n").encode()
    return files


def write_zip(path, files):
    path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(path, "x", compression=zipfile.ZIP_DEFLATED) as archive:
        for name, body in files.items():
            if ".." in Path(name).parts or Path(name).is_absolute() or "/.local/" in name or (name.endswith(".jar") and not name.endswith(("/rusted-bootstrap.jar", "mods/rusted-bootstrap.jar"))):
                raise ValueError("Fichier interdit dans une distribution native : " + name)
            archive.writestr(name, body)
    with zipfile.ZipFile(path) as archive:
        if archive.testzip() is not None:
            raise ValueError("Archive corrompue.")
    print(path, path.stat().st_size, "bytes")


def write_patch(path, files, previous):
    # Overlay only changed distributed files. Keep the player's configuration,
    # prepared resources, native mod choices and saves in the existing instance.
    changed = {}
    with zipfile.ZipFile(previous) as archive:
        for name, body in files.items():
            if name.startswith("native-data/") or name == "rusted-runtime.json":
                continue
            old = "Rusted/" + name
            if old not in archive.namelist() or hashlib.sha256(archive.read(old)).digest() != hashlib.sha256(body).digest():
                changed[name] = body
    write_zip(path, changed)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary-dir", type=Path, default=ROOT / "build/horizon/target/x86_64-pc-windows-gnu/release")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "build")
    parser.add_argument("--bridge-binary", type=Path, default=ROOT / "target/x86_64-pc-windows-gnu/release/rusted-launch.exe")
    parser.add_argument("--setup-binary", type=Path, default=ROOT / "target/x86_64-pc-windows-gnu/release/rusted-setup.exe")
    parser.add_argument("--patch-from", type=Path, help="Previous standalone Windows ZIP; also build a data-preserving runtime overlay")
    args = parser.parse_args()
    files = payloads(args.binary_dir, args.bridge_binary, args.setup_binary)
    write_zip(args.output_dir / f"Rusted-Native-Windows-{VERSION}.zip", {"Rusted/" + name: body for name, body in files.items()})
    prism = {"minecraft/rusted/" + name: body for name, body in files.items()}
    prism["minecraft/mods/rusted-bootstrap.jar"] = files["bootstrap/rusted-bootstrap.jar"]
    prism["mmc-pack.json"] = json.dumps({"formatVersion": 1, "components": [{"uid": "net.minecraft", "version": LOCK["minecraft_version"]}, {"uid":"net.fabricmc.fabric-loader","version":FABRIC["version"]}]}).encode()
    prism["instance.cfg"] = b"[General]\nname=Rusted 0.3.6\niconKey=rusted_0_3_5\nInstanceType=OneSix\nOverrideCommands=true\nWrapperCommand=\nPreLaunchCommand=\nPostExitCommand=\n"
    prism["icon.png"] = (ROOT / "artwork/rusted-icon.png").read_bytes()
    prism["LISEZ-MOI.md"] = files["LISEZ-MOI.md"]
    write_zip(args.output_dir / f"Rusted-Native-Prism-{VERSION}.zip", prism)
    modrinth = {"overrides/rusted/" + name: body for name, body in files.items()}
    modrinth["overrides/mods/rusted-bootstrap.jar"] = files["bootstrap/rusted-bootstrap.jar"]
    modrinth["modrinth.index.json"] = json.dumps({"formatVersion": 1, "game": "minecraft", "versionId": VERSION, "name": "Rusted Native 26.3 (Windows)",
        "summary": "Rusted native client derived from Horizon; native engine for vanilla, original JVM for installed Java loaders in auto mode. See the included setup guide.",
        "dependencies": {"minecraft": LOCK["minecraft_version"], "fabric-loader":FABRIC["version"]}, "files": []}, indent=2).encode()
    write_zip(args.output_dir / f"Rusted-Native-Modrinth-{VERSION}.mrpack", modrinth)
    if args.patch_from:
        write_patch(args.output_dir / f"Rusted-Correctif-Windows-{VERSION}.zip", files, args.patch_from)


if __name__ == "__main__":
    main()
