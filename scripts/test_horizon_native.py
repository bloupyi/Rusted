#!/usr/bin/env python3
"""Drive an isolated native developer game: menus, world, edits, save/reload. No account."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]


class NativeGame:
    def __init__(self, binary, resources, scratch, home, offscreen):
        self.scratch, self.home = scratch, home
        self.endpoint = scratch / "ctl.sock"
        self.log = (scratch / "console.log").open("w")
        env = os.environ.copy()
        for key, folder in [("HORIZON_HOME", home), ("HORIZON_LAUNCHER_HOME", scratch / "launcher"),
                            ("XDG_DATA_HOME", scratch / "xdg-data"), ("XDG_CONFIG_HOME", scratch / "xdg-config"),
                            ("XDG_RUNTIME_DIR", scratch / "runtime"), ("XDG_CACHE_HOME", scratch / "cache")]:
            folder.mkdir(parents=True, exist_ok=True)
            env[key] = str(folder)
        (scratch / "runtime").chmod(0o700)
        env.update(HORIZON_LAUNCHER_NO_SELF_UPDATE="1", HORIZON_NO_AUDIO="1")
        args = [str(binary), "--resources", str(resources), "--control-socket", str(self.endpoint), "--render-distance", "2", "--width", "960", "--height", "540"]
        if offscreen:
            args.append("--offscreen")
        self.process = subprocess.Popen(args, cwd=home, env=env, stdout=self.log, stderr=subprocess.STDOUT)

    def command(self, line):
        if self.process.poll() is not None:
            raise RuntimeError("Native game stopped: " + (self.scratch / "console.log").read_text()[-3000:])
        with socket.socket(socket.AF_UNIX) as connection:
            connection.settimeout(120)
            connection.connect(str(self.endpoint))
            connection.sendall((line + "\n").encode())
            with connection.makefile("r") as stream:
                reply = json.loads(stream.readline())
        if "error" in reply:
            raise RuntimeError(f"Native control {line!r}: {reply['error']}")
        return reply

    def wait(self, predicate, seconds=120):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise RuntimeError("Native game stopped: " + (self.scratch / "console.log").read_text()[-3000:])
            if self.endpoint.exists():
                state = self.command("state")
                if predicate(state):
                    return state
            time.sleep(0.2)
        raise RuntimeError("Native scenario timed out")

    def screenshot(self, name):
        path = self.scratch / (name + ".png")
        self.command("screenshot " + str(path))
        if not path.is_file() or path.stat().st_size < 1000:
            raise RuntimeError("Native screenshot missing or empty")
        return {"path": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}

    def wait_block(self, position, expected=None):
        deadline = time.monotonic() + 120
        while time.monotonic() < deadline:
            block = self.command("block " + " ".join(map(str, position)))
            if block.get("loaded") and (expected is None or block.get("block") == expected):
                return block
            time.sleep(0.2)
        raise RuntimeError("Native block did not load with the expected state: " + json.dumps(block))

    def close(self):
        if self.process.poll() is None:
            try:
                self.command("quit")
                self.process.wait(timeout=60)
            except Exception:
                self.process.kill()
                self.process.wait()
                raise
        self.log.close()
        if self.process.returncode != 0:
            raise RuntimeError(f"Native game exit {self.process.returncode}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "build/horizon/target/release/rusted")
    parser.add_argument("--resources", type=Path, default=ROOT / "build/horizon/source/.local/horizon-client/26.3")
    parser.add_argument("--offscreen", action="store_true")
    args = parser.parse_args()
    scratch = Path(tempfile.mkdtemp(prefix="visual-", dir=ROOT / "build/horizon"))
    home = scratch / "game"
    report = {"kind": "native-developer-local-scenario", "home": str(home), "resources": str(args.resources),
              "account_authentication_tested": False, "screenshots": {}, "steps": []}
    (ROOT / "build/horizon/latest-visual-test.json").write_text(json.dumps({"scratch": str(scratch)}))
    game = NativeGame(args.binary.resolve(), args.resources.resolve(), scratch, home, args.offscreen)
    try:
        title = game.wait(lambda state: state.get("menu") is not None)
        report["title_state"] = title
        report["screenshots"]["title"] = game.screenshot("title")
        print("Native title rendered", flush=True)
        for page in ["main", "video", "sound", "controls", "mods", "java-mods", "credits"]:
            game.command("options " + page)
            report["screenshots"][page] = game.screenshot("options-" + page)
        game.wait(lambda state: state.get("menu") is not None)
        game.command("open RustedNativeTest 1 creative")
        state = game.wait(lambda state: state.get("world") is not None and state.get("player") is not None)
        report["initial_world"] = state
        report["steps"].append("world_loaded_with_integrated_server")
        print("Native world rendered", flush=True)
        x, y, z = state["player"]["block_pos"]
        # A unique marker near the player exercises actual world edits and persistence.
        marker = [x + 2, y, z + 2]
        game.wait_block(marker)
        report["screenshots"]["world"] = game.screenshot("world")
        game.command("command setblock " + " ".join(map(str, marker)) + " minecraft:gold_block")
        report["written_marker"] = game.wait_block(marker, "minecraft:gold_block")
        game.command("save")
        report["marker"] = marker
        report["screenshots"]["saved_world"] = game.screenshot("world-saved")
        report["steps"].append("command_and_world_save")
        game.close()
        report["steps"].append("clean_process_exit")
        print("Native world saved; restarting", flush=True)
        second = scratch / "restart"
        second.mkdir()
        game = NativeGame(args.binary.resolve(), args.resources.resolve(), second, home, args.offscreen)
        game.wait(lambda state: state.get("menu") is not None)
        game.command("open RustedNativeTest")
        reloaded = game.wait(lambda state: state.get("world") is not None and state.get("player") is not None)
        report["reloaded_world"] = reloaded
        block = game.wait_block(marker, "minecraft:gold_block")
        report["reloaded_marker"] = block
        if "minecraft:gold_block" not in json.dumps(block):
            raise RuntimeError("Native world marker was not preserved")
        report["screenshots"]["reloaded_world"] = game.screenshot("world-reloaded")
        report["steps"].append("world_and_block_persisted_after_process_restart")
        game.close()
        report["passed"] = True
        print("Native world reloaded and block persisted", flush=True)
    except Exception as exc:
        report["passed"] = False
        report["error"] = str(exc)
        if game.process.poll() is None:
            game.process.kill()
            game.process.wait()
        raise
    finally:
        (scratch / "validation.json").write_text(json.dumps(report, indent=2) + "\n")
        print("Native validation:", scratch / "validation.json", flush=True)


if __name__ == "__main__":
    main()
