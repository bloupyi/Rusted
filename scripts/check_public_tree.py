#!/usr/bin/env python3
"""Audit Git source paths, including the vendored engine, before distribution."""
import importlib.util
from pathlib import Path, PurePosixPath
import subprocess

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("engine_audit", ROOT / "engine/tools/check_public_tree.py")
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)
paths = subprocess.check_output(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=ROOT).decode().split("\0")
rejected = []
for path in filter(None, paths):
    if path.startswith("engine/"):
        bad = audit.prohibited(path.removeprefix("engine/"))
    else:
        parsed = PurePosixPath(path)
        bad = parsed.parts[0] in {"build", "target", ".local", "logs", "saves"} or parsed.suffix.lower() in {".jar", ".class", ".jfr", ".hprof"}
    if bad:
        rejected.append(path)
if rejected:
    raise SystemExit("Private files in public tree:\n" + "\n".join(rejected))
print(f"Public source audit passed ({len(paths) - 1} paths).")
