#!/usr/bin/env python3
"""Compile only Rusted's original Java handover, against Java's standard library."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def build(output, javac="javac"):
    source = ROOT / "launcher/java/dev/rusted/launcher/Entrypoint.java"
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="rusted-entry-") as temp:
        classes = Path(temp) / "classes"
        classes.mkdir()
        subprocess.run([javac, "--release", "17", "-proc:none", "-d", str(classes), str(source)], check=True)
        with zipfile.ZipFile(output / "launcher-entry.jar", "w", compression=zipfile.ZIP_DEFLATED) as archive:
            for path in sorted(classes.rglob("*.class")):
                name = path.relative_to(classes).as_posix()
                if not name.startswith("dev/rusted/launcher/"):
                    raise ValueError("Unexpected class in original launcher entry")
                archive.writestr(zipfile.ZipInfo(name, (2026, 1, 1, 0, 0, 0)), path.read_bytes())
    record = {"schema": 1, "source_sha256": {source.relative_to(ROOT).as_posix(): hashlib.sha256(source.read_bytes()).hexdigest()},
              "jar_sha256": hashlib.sha256((output / "launcher-entry.jar").read_bytes()).hexdigest(),
              "contains_minecraft_classes": False}
    (output / "launcher-entry.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    print(output / "launcher-entry.jar")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=ROOT / "build")
    parser.add_argument("--javac", default="javac")
    args = parser.parse_args()
    build(args.output_dir, args.javac)
