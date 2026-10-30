#!/usr/bin/env python3
"""Assemble the complete original Windows launcher and verified Rusted runtime."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import zipfile
import uuid

from build_launcher_entry import build as build_entry
from package_horizon_native import pe_amd64
from rusted import ROOT, source_hash

VERSION = "0.2.5"


def digest(body):
    return hashlib.sha256(body).hexdigest()


DEFAULT_MICROSOFT_CLIENT_ID = "00000000402b5328"  # msmc 5.0.5 public Live client.


def publisher_auth_config():
    config = json.loads((ROOT / "launcher/default-config.json").read_text())
    client = (os.environ.get("RUSTED_MICROSOFT_CLIENT_ID", "").strip()
              or config.get("microsoft_client_id", "").strip() or DEFAULT_MICROSOFT_CLIENT_ID)
    if client != DEFAULT_MICROSOFT_CLIENT_ID:
        try:
            parsed = uuid.UUID(client)
            if str(parsed) != client.lower() or parsed.int == 0:
                raise ValueError("Noncanonical or empty client id")
        except ValueError as exc:
            raise ValueError("Invalid publisher Microsoft application ID; provide the Rusted application's public client GUID.") from exc
    return {"schema": 1, "microsoft_client_id": client}


def write_zip(path, files):
    with zipfile.ZipFile(path, "x", compression=zipfile.ZIP_DEFLATED, compresslevel=7) as archive:
        for name, body in sorted(files.items()):
            archive.writestr(name, body)
    with zipfile.ZipFile(path) as archive:
        if archive.testzip():
            raise ValueError("ZIP integrity failure")
    print(path, path.stat().st_size, "bytes")


def package(args):
    auth_config = publisher_auth_config()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(args.runtime) as archive:
        files = {}
        for name in archive.namelist():
            if not name.startswith("Rusted/") or ".." in PurePosixPath(name).parts:
                raise ValueError("Unexpected runtime ZIP member")
            if not name.endswith("/"):
                files[name.removeprefix("Rusted/")] = archive.read(name)
    runtime_manifest = json.loads(files["runtime-manifest.json"])
    for name, expected in runtime_manifest["files"].items():
        if digest(files[name]) != expected:
            raise ValueError("Runtime checksum mismatch: " + name)
    runtime_receipt = json.loads(files["rusted-build.json"])
    if runtime_receipt["source_sha256"] != source_hash():
        raise ValueError("Rebuild the native runtime for current engine sources")
    for name in files:
        if "/.local/" in name or name.endswith((".secret", "accounts.json")):
            raise ValueError("Private runtime member: " + name)
        if name.endswith(".jar") and name != "bootstrap/rusted-bootstrap.jar":
            raise ValueError("Third-party Java archive in runtime: " + name)
    launcher = pe_amd64(args.launcher)
    pe = int.from_bytes(launcher[60:64], "little")
    if int.from_bytes(launcher[pe + 24 + 68:pe + 24 + 70], "little") != 2:
        raise ValueError("Launcher must use the Windows GUI subsystem")
    build_entry(args.output_dir)
    files["launcher-entry.jar"] = (args.output_dir / "launcher-entry.jar").read_bytes()
    files["launcher-entry.json"] = (args.output_dir / "launcher-entry.json").read_bytes()
    payload = {"rusted/" + name: body for name, body in files.items()}
    payload["Rusted Launcher.exe"] = launcher
    payload["launcher-config.json"] = (json.dumps(auth_config, indent=2) + "\n").encode()
    payload["LICENSE"] = (ROOT / "LICENSE").read_bytes()
    payload["LISEZ-MOI.md"] = (ROOT / "docs/RUSTED-LAUNCHER.md").read_bytes()
    # Include licences of the launcher's extra crates, in addition to the runtime notices.
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--offline", "--format-version", "1"], cwd=ROOT))
    for dependency in metadata["packages"]:
        for path in Path(dependency["manifest_path"]).parent.iterdir():
            if path.is_file() and path.name.upper().startswith(("LICENSE", "COPYING", "NOTICE")):
                payload[f"licenses/{dependency['name']}-{dependency['version']}/{path.name}"] = path.read_bytes()
    public = subprocess.check_output(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=ROOT).decode().split("\0")
    launcher_sources = {name: digest((ROOT / name).read_bytes()) for name in public if name and
        (name.startswith(("launcher/", "launcher-bridge/")) or name in ("Cargo.toml", "Cargo.lock", "artwork/rusted-icon.ico"))}
    payload["launcher-manifest.json"] = (json.dumps({"schema": 1, "name": "Rusted Launcher", "version": VERSION,
        "native_engine_version": runtime_receipt["version"], "launcher_original_implementation": True,
        "native_engine_origin": "Horizon MIT fork; see rusted/PROVENANCE.md",
        "includes_minecraft_files": False, "includes_accounts": False, "includes_java_runtime": False,
        "microsoft_login_configured": bool(auth_config["microsoft_client_id"]),
        "authentication_flow": "microsoft-live-device" if auth_config["microsoft_client_id"] == DEFAULT_MICROSOFT_CLIENT_ID else "entra-device",
        "oauth_client_identity": "shared-public-client (msmc compatibility)" if auth_config["microsoft_client_id"] == DEFAULT_MICROSOFT_CLIENT_ID else "publisher-managed",
        "distribution_kind": "release-candidate",
        "windows_hardware_verified": False,
        "source_sha256": launcher_sources, "files": {name: digest(body) for name, body in payload.items()}}, indent=2) + "\n").encode()
    target = args.output_dir / f"Rusted-Launcher-Windows-{VERSION}.zip"
    write_zip(target, {"Rusted Launcher/" + name: body for name, body in payload.items()})
    if args.source:
        subprocess.run(["python3" if __import__('os').name != 'nt' else "python", str(ROOT / "scripts/check_public_tree.py")], cwd=ROOT, check=True)
        source_zip = args.output_dir / f"Rusted-Launcher-Source-{VERSION}.zip"
        write_zip(source_zip, {"Rusted/" + name: (ROOT / name).read_bytes() for name in public if name and (ROOT / name).is_file()})
    sums = [target] + ([source_zip] if args.source else [])
    (args.output_dir / f"Rusted-Launcher-{VERSION}-SHA256SUMS.txt").write_text(
        "".join(digest(path.read_bytes()) + "  " + path.name + "\n" for path in sums), encoding="ascii")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runtime", type=Path, default=ROOT / "build/Rusted-Native-Windows-0.3.6.zip")
    parser.add_argument("--launcher", type=Path, default=ROOT / "target/release/rusted-launcher.exe")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "build")
    parser.add_argument("--source", action="store_true")
    package(parser.parse_args())
