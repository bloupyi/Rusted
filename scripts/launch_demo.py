#!/usr/bin/env python3
"""Launch the ORIGINAL Minecraft/Fabric client in official demo mode.

Only for isolated development tests; no account credentials or authenticated
multiplayer, and no removal of the original demo restrictions.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

from prepare_minecraft import ROOT

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--instance", type=Path, help="Reuse only an instance previously created by this script")
    parser.add_argument("--disabled", action="store_true", help="Keep the Java backend for comparison")
    parser.add_argument("--width", type=int, default=1280)
    parser.add_argument("--height", type=int, default=720)
    args = parser.parse_args()
    if not (640 <= args.width <= 3840 and 480 <= args.height <= 2160):
        parser.error("Unsupported test resolution")
    metadata = ROOT / "build/minecraft/client-runtime.json"
    if not metadata.is_file():
        parser.error("Run scripts/prepare_client.py first")
    runtime = json.loads(metadata.read_text())
    instance = (args.instance or ROOT / "build/client-runs" / str(time.time_ns())).resolve()
    if instance.exists():
        if not (instance / "rusted-demo-instance.json").is_file():
            parser.error("Refuse to reuse a directory not created as a disposable Rusted demo instance")
    else:
        instance.mkdir(parents=True)
        (instance / "rusted-demo-instance.json").write_text(json.dumps({"minecraft": "1.20.4", "demo": True, "personal_world": False}))
        # Only local test settings; no changes to any installed Minecraft profile.
        (instance / "options.txt").write_text("autoJump:false\nrenderDistance:4\nsimulationDistance:5\nmaxFps:60\nguiScale:2\nsoundCategory_master:0.0\n")
    mods = instance / "mods"
    mods.mkdir(exist_ok=True)
    production = ROOT / "build/rusted-backend-0.1.0-mc1.20.4.jar"
    shutil.copyfile(production, mods / production.name)
    command = ["java", "-Xmx2g", "-Djava.library.path=" + runtime["natives"],
               "-Drusted.enabled=" + str(not args.disabled).lower(), "-Dfabric.side=client",
               "-Dfabric.gameVersion=1.20.4", "-cp", os.pathsep.join(runtime["classpath"]), runtime["main_class"],
               "--version", "1.20.4", "--gameDir", str(instance), "--assetsDir", runtime["assets"],
               "--assetIndex", runtime["asset_index"], "--width", str(args.width), "--height", str(args.height), "--demo"]
    # Fabric's documented development argument defaults are used. We do not
    # fabricate an account token or request an authenticated server session.
    environment = dict(os.environ)
    if "XDG_CACHE_HOME" not in environment:
        environment["XDG_CACHE_HOME"] = str(ROOT / "build/runtime-cache")
    print("Original Minecraft demo instance:", instance, flush=True)
    run = instance / "rusted-runs" / str(time.time_ns())
    run.mkdir(parents=True)
    log_path = run / "console.log"
    started = time.monotonic()
    with log_path.open("x") as log:
        process = subprocess.Popen(command, cwd=instance, env=environment, stdout=log, stderr=subprocess.STDOUT)
        (instance / "process.json").write_text(json.dumps({"pid": process.pid, "demo": True, "native_requested": not args.disabled, "console_log": str(log_path)}))
        code = process.wait()
    (run / "exit.json").write_text(json.dumps({"code": code, "elapsed_seconds": time.monotonic() - started,
                                              "demo": True, "native_requested": not args.disabled}))
    print("Original Minecraft demo exit:", code, "logs:", log_path)
    raise SystemExit(code)

if __name__ == "__main__":
    main()
