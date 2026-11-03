#!/usr/bin/env python3
"""Reject local game artifacts even when somebody uses git add -f."""

import argparse
from pathlib import PurePosixPath
import subprocess
import sys

PRIVATE_ROOTS = {".local", "decompiled", "minecraft", "assets", "run", "logs"}
PRIVATE_EXTENSIONS = {".jar", ".class", ".jfr", ".hprof", ".java"}
ORIGINAL_JDK_TOOLS = {"tools/JfrSummary.java", "tests/java/JfrFixture.java"}

# Original JNI facades authored from public API signatures, not recovered game code.
# Keep this an exact list: arbitrary Java under this directory stays prohibited.
ORIGINAL_RUSTED_FACADES = {
    'crates/rusted-java/java/dev/rusted/compat/Host.java',
    'crates/rusted-java/java/dev/rusted/compat/Versions.java',
    'crates/rusted-java/java/net/fabricmc/api/ClientModInitializer.java',
    'crates/rusted-java/java/net/fabricmc/api/DedicatedServerModInitializer.java',
    'crates/rusted-java/java/net/fabricmc/api/EnvType.java',
    'crates/rusted-java/java/net/fabricmc/api/ModInitializer.java',
    'crates/rusted-java/java/net/fabricmc/fabric/api/client/event/lifecycle/v1/ClientTickEvents.java',
    'crates/rusted-java/java/net/fabricmc/fabric/api/event/Event.java',
    'crates/rusted-java/java/net/fabricmc/loader/api/FabricLoader.java',
    'crates/rusted-java/java/net/fabricmc/loader/api/LanguageAdapter.java',
    'crates/rusted-java/java/net/fabricmc/loader/api/LanguageAdapterException.java',
    'crates/rusted-java/java/net/fabricmc/loader/api/ModContainer.java',
    'crates/rusted-java/java/net/fabricmc/loader/api/Version.java',
    'crates/rusted-java/java/net/fabricmc/loader/api/SemanticVersion.java',
    'crates/rusted-java/java/net/fabricmc/loader/api/VersionParsingException.java',
    'crates/rusted-java/java/net/fabricmc/loader/api/entrypoint/PreLaunchEntrypoint.java',
    'crates/rusted-java/java/net/fabricmc/loader/api/metadata/ModMetadata.java',
    'crates/rusted-java/java/net/minecraft/client/Minecraft.java',
    'crates/rusted-java/java/net/minecraft/IdentifierException.java',
    'crates/rusted-java/java/net/minecraft/resources/Identifier.java',
    'crates/rusted-java/java/net/minecraft/client/multiplayer/ClientLevel.java',
    'crates/rusted-java/java/net/minecraft/client/player/LocalPlayer.java',
    'crates/rusted-java/java/net/minecraft/network/chat/Component.java',
    'crates/rusted-java/java/net/minecraft/network/chat/MutableComponent.java',
    'crates/rusted-java/java/net/minecraft/world/level/Level.java',
    'crates/rusted-java/java/net/minecraft/world/level/storage/LevelData.java',
}


def prohibited(path):
    parsed = PurePosixPath(path)
    # Original JDK-only tools are an exact allowlist; recovered game Java stays private.
    if path in ORIGINAL_JDK_TOOLS or path in ORIGINAL_RUSTED_FACADES:
        return False
    # Original Horizon mod sources may also be tracked.
    if parsed.suffix == ".java" and path.startswith(("mods/horizon/src/", "mods/horizon-lab/src/")):
        return not ("/dev/horizonlab/" in path)
    return bool(parsed.parts and parsed.parts[0] in PRIVATE_ROOTS) or parsed.suffix.lower() in PRIVATE_EXTENSIONS


def paths_in_repository(history=False, revisions=None):
    if not history:
        result = subprocess.check_output(["git", "ls-files", "-z"])
        return result.decode("utf-8").split("\0")
    # An empty push (including deletions only) must not fall back to HEAD or --all.
    if revisions is not None and not revisions:
        return []
    roots = ["--all"] if revisions is None else revisions
    # Include diffs against merge parents so merge-resolution-only artifacts are caught.
    # Git pushes the stored objects, not the history presented by local replace refs.
    result = subprocess.check_output(["git", "--no-replace-objects", "log", "--format=", "--name-only",
                                      "-z", "-m", *roots, "--"])
    return [path.lstrip("\n") for path in result.decode("utf-8").split("\0")]


def pushed_revisions(stream):
    """Read Git's pre-push ref updates and retain the non-deleted local object IDs."""
    revisions = []
    for line in stream:
        _local_ref, local_oid, _remote_ref, _remote_oid = line.split()
        if local_oid.strip("0"):
            revisions.append(local_oid)
    return revisions


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--history", action="store_true", help="Inspect paths in all local Git history")
    mode.add_argument("--pre-push", action="store_true", help="Inspect history reachable from Git's ref updates on stdin")
    args = parser.parse_args()
    revisions = pushed_revisions(sys.stdin) if args.pre_push else None
    rejected = sorted({path for path in paths_in_repository(args.history or args.pre_push, revisions)
                       if path and prohibited(path)})
    if rejected:
        print("Private game/profiling artifacts must stay outside Git:", file=sys.stderr)
        for path in rejected:
            print(f"  {path}", file=sys.stderr)
        return 1
    print("Public tree check passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
