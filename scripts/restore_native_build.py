#!/usr/bin/env python3
"""Reuse only pinned, source-matching native binaries from our validated Windows build."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import zipfile

from rusted import ROOT, source_hash


def restore(archive, expected_sha256, output_dir):
    if not archive.is_file():
        print("No previous native build available; compile from source.")
        return False
    body = archive.read_bytes()
    if hashlib.sha256(body).hexdigest() != expected_sha256:
        print("Native cache integrity mismatch; compile from source.")
        return False
    with zipfile.ZipFile(io.BytesIO(body)) as bundle:
        receipt_bytes = bundle.read("Rusted/rusted-build.json")
        receipt = json.loads(receipt_bytes)
        if receipt.get("source_sha256") != source_hash():
            print("Engine sources changed; compile from source.")
            return False
        files = {}
        for name, member in [("rusted.exe", "Rusted/runtime/rusted.exe"),
                             ("horizon-prepare.exe", "Rusted/runtime/rusted-prepare.exe")]:
            payload = bundle.read(member)
            if hashlib.sha256(payload).hexdigest() != receipt["binaries"][name]:
                print("Native build receipt mismatch; compile from source.")
                return False
            files[name] = payload
    # Copy only these three verified files; packaging rebuilds wrappers/resources/notices.
    files["rusted-build.json"] = receipt_bytes
    output_dir.mkdir(parents=True, exist_ok=True)
    for name, payload in files.items():
        (output_dir / name).write_bytes(payload)
    print("Reused source-matching MSVC engine binaries from the pinned validated build.")
    return True


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--expected-sha256", required=True)
    parser.add_argument("--output-dir", type=Path, default=ROOT / "build/horizon/target/release")
    args = parser.parse_args()
    restored = restore(args.archive, args.expected_sha256, args.output_dir)
    if os.environ.get("GITHUB_OUTPUT"):
        with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as stream:
            stream.write(f"restored={str(restored).lower()}\n")
