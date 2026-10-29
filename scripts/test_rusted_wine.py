#!/usr/bin/env python3
"""Run the Windows Rusted game under Wine in an isolated developer world. No account."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wine", type=Path, required=True)
    parser.add_argument("--prefix", type=Path, required=True)
    parser.add_argument("--binary", type=Path, default=ROOT / "build/horizon/target/x86_64-pc-windows-gnu/release/rusted.exe")
    parser.add_argument("--resources", type=Path, default=ROOT / "build/horizon/source/.local/horizon-client/26.3")
    args = parser.parse_args()
    scratch = Path(tempfile.mkdtemp(prefix="rusted-wine-world-", dir=ROOT / "build/horizon"))
    home = scratch / "game"
    home.mkdir()
    windows = lambda path: "Z:" + str(Path(path).resolve()).replace("/", "\\")
    env = os.environ.copy()
    env.update(WINEPREFIX=str(args.prefix.resolve()), WINEDEBUG="-all", HORIZON_NO_AUDIO="1", HORIZON_LAUNCHER_NO_SELF_UPDATE="1")
    for key, folder in [("HORIZON_HOME", home), ("RUSTED_HOME", home), ("HORIZON_LAUNCHER_HOME", scratch / "launcher")]:
        folder.mkdir(exist_ok=True)
        env[key] = windows(folder)
    for key, name in [("XDG_DATA_HOME", "xdg-data"), ("XDG_CONFIG_HOME", "xdg-config"), ("XDG_RUNTIME_DIR", "runtime")]:
        folder = scratch / name
        folder.mkdir(mode=0o700)
        env[key] = str(folder)
    result = {"kind": "windows-native-game-under-wine", "native_windows_hardware": False,
              "account_authentication_tested": False, "binary": str(args.binary.resolve()), "scratch": str(scratch)}
    (ROOT / "build/horizon/latest-rusted-wine-test.json").write_text(json.dumps({"scratch": str(scratch)}))
    command = [str(args.wine.resolve()), windows(args.binary), "--resources", windows(args.resources),
               "--world", "RustedWindowsTest", "--seed", "1", "--render-distance", "2",
               "--world-frames", "10", "--offscreen", "--width", "960", "--height", "540",
               "--smoke-report", windows(scratch / "report.json")]
    with (scratch / "console.log").open("w") as log:
        process = subprocess.Popen(command, env=env, cwd=home, stdout=log, stderr=subprocess.STDOUT)
        try:
            result["exit_code"] = process.wait(timeout=220)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
            result["timed_out"] = True
            result["exit_code"] = process.returncode
    if (scratch / "report.json").is_file():
        result["world"] = json.loads((scratch / "report.json").read_text())
    world = result.get("world", {})
    result["passed"] = result["exit_code"] == 0 and not world.get("error") and world.get("chunks", 0) > 0 and world.get("frames", 0) >= 10
    (scratch / "validation.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))
    if not result["passed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
