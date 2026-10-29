#!/usr/bin/env python3
"""Create an isolated Prism/MultiMC test-instance import ZIP, with only our mod.
The user's launcher obtains the game and handles authentication normally.
"""
import argparse
import json
from pathlib import Path
import zipfile

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mod", required=True, type=Path)
    parser.add_argument("--output", type=Path, default=Path("build/Rusted-1.20.4-test.zip"))
    args = parser.parse_args()
    if not args.mod.is_file():
        parser.error("Build the mod JAR first; the native bridge test JAR is not a game mod")
    with zipfile.ZipFile(args.mod) as jar:
        meta = json.loads(jar.read("fabric.mod.json"))
        if meta["id"] != "rusted_backend":
            parser.error("Not the Rusted mod")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(args.output, "x", zipfile.ZIP_DEFLATED) as out:
        out.writestr("mmc-pack.json", json.dumps({"formatVersion": 1, "components": [
            {"uid": "net.minecraft", "version": "1.20.4"},
            {"uid": "net.fabricmc.fabric-loader", "version": "0.15.11"}
        ]}, indent=2))
        out.writestr("instance.cfg", "[General]\nname=Rusted 1.20.4 isolated test\nInstanceType=OneSix\nOverrideJavaArgs=true\nJvmArgs=-Drusted.enabled=true\n")
        out.write(args.mod, "minecraft/mods/" + args.mod.name)
        out.writestr("RUSTED-TEST-NOTES.txt", "Use your own authorized Minecraft account. Create a disposable world.\nExpected: original Minecraft menus; Rust encodes outgoing compressed packets only.\nThis package is a test configuration, not a compatibility certification.\n")
        windows_guide = Path(__file__).resolve().parents[1] / "docs/WINDOWS.md"
        if windows_guide.is_file():
            out.write(windows_guide, "LISEZ-MOI-WINDOWS.txt")
    print("Created", args.output, "— import into Prism/MultiMC; original game and authentication remain in that launcher")

if __name__ == "__main__":
    main()
