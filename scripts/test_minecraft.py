#!/usr/bin/env python3
"""Run real Minecraft network handlers under Fabric with our production mixin.

This is a codec integration test, not a game session or authenticated multiplayer.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import time
import zipfile

from prepare_minecraft import ROOT, prepare

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prepared", action="store_true", help="Reuse already verified local runtime")
    parser.add_argument("--mode", choices=("enabled", "disabled", "missing", "all"), default="all")
    args = parser.parse_args()
    runtime = json.loads((ROOT / "build/minecraft/runtime.json").read_text()) if args.prepared else prepare()
    production = ROOT / "build/rusted-backend-0.1.0-mc1.20.4.jar"
    if not production.is_file():
        parser.error("Build the production mod first (scripts/build_mod.py)")
    classes = ROOT / "build/integration-classes"
    classes.mkdir(parents=True, exist_ok=True)
    compiler = ["javac"] if shutil.which("javac") else ["java", "com.sun.tools.javac.Main"]
    subprocess.run([*compiler, "--release", "17", "-proc:none", "-cp", os.pathsep.join([str(production), *runtime["classpath"]]),
                    "-d", str(classes), str(ROOT / "java/src/integration/java/dev/rusted/integration/MinecraftNetworkTest.java")], check=True)
    test_mod = ROOT / "build/rusted-integration-test.jar"
    with zipfile.ZipFile(test_mod, "w", zipfile.ZIP_DEFLATED) as archive:
        for item in classes.rglob("*.class"):
            archive.write(item, item.relative_to(classes).as_posix())
        archive.writestr("fabric.mod.json", json.dumps({
            "schemaVersion": 1, "id": "rusted_integration_test", "version": "0.1.0", "environment": "server",
            "entrypoints": {"preLaunch": ["dev.rusted.integration.MinecraftNetworkTest"]},
            "depends": {"rusted_backend": "*", "minecraft": "=1.20.4", "fabricloader": ">=0.15.11"}
        }))
    run_root = ROOT / "build/minecraft-runs" / str(time.time_ns())
    run_root.mkdir(parents=True)
    for mode in (("enabled", "disabled", "missing") if args.mode == "all" else (args.mode,)):
        directory = run_root / mode
        mods = directory / "mods"
        mods.mkdir(parents=True)
        shutil.copyfile(production, mods / production.name)
        shutil.copyfile(test_mod, mods / test_mod.name)
        report = directory / "report.json"
        command = ["java", "-Xmx1536m", "-Xcheck:jni", "-Dio.netty.leakDetection.level=paranoid",
                   "-Dfabric.gameVersion=1.20.4", "-Dfabric.side=server", "-Drusted.enabled=" + str(mode != "disabled").lower(),
                   "-Drusted.integration.mode=" + mode, "-Drusted.integration.report=" + str(report),
                   "-Dmixin.debug.export=true"]
        if mode == "missing":
            command.append("-Drusted.native.path=" + str(directory / "intentionally-absent-native"))
        command += ["-cp", os.pathsep.join(runtime["classpath"]), runtime["main_class"], "nogui"]
        print("Running actual Minecraft/Fabric codec integration:", mode, "logs:", directory, flush=True)
        with (directory / "console.log").open("w") as output:
            result = subprocess.run(command, cwd=directory, stdout=output, stderr=subprocess.STDOUT, timeout=240)
        text = (directory / "console.log").read_text()
        if result.returncode == 0:
            print("\n".join(line for line in text.splitlines() if "RUSTED_MINECRAFT_NETWORK_PASS" in line))
        else:
            print(text[-12000:])
        if result.returncode != 0 or not report.exists() or not json.loads(report.read_text()).get("passed"):
            raise SystemExit(f"Real Minecraft integration failed ({mode}), exit {result.returncode}; see {directory / 'console.log'}")
        if (directory / "world").exists() or (directory / "eula.txt").exists():
            raise SystemExit("Test unexpectedly reached world/EULA setup")
    print("Real Minecraft codec integration passed. Reports:", run_root)

if __name__ == "__main__":
    main()
