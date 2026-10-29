#!/usr/bin/env python3
"""Real process tests using our own protocol peer, never a game/authentication mock server."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
WINE = os.environ.get("RUSTED_TEST_WINE")
SUFFIX = ".exe" if os.name == "nt" or WINE else ""
BINARY_DIR = ROOT / ("target/x86_64-pc-windows-gnu/release" if WINE else "target/debug")
BRIDGE = BINARY_DIR / ("rusted-launch" + SUFFIX)
PEER = BINARY_DIR / "examples" / ("protocol_peer" + SUFFIX)
TOKEN = "TEST_ONLY_NOT_A_REAL_SESSION_TOKEN"
REVISION = json.loads((ROOT / "horizon/upstream.lock.json").read_text())["revision"]


def native_path(path):
    return "Z:" + str(Path(path).resolve()).replace("/", "\\") if WINE else str(path)


class BridgeProcessTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="rusted bridge test ")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.game = self.root / "java instance"
        self.game.mkdir()
        self.resources = self.root / "resources"
        self.resources.mkdir()
        (self.resources / "horizon-resources.json").write_text(json.dumps({"minecraft": "26.3"}))
        self.config = {"schema": 1, "mode": "native", "horizon_revision": REVISION, "minecraft_version": "26.3",
                       "executable": native_path(PEER), "resources": "resources", "data_directory": "native data"}
        self.path = self.root / "config.json"
        self.params = ["--username", "BridgeFixture", "--uuid", "123456781234123412341234567890ab", "--accessToken", TOKEN,
                       "--userType", "msa", "--version", "26.3", "--gameDir", native_path(self.game)]

    def invoke(self, args, text="", auto_config=False):
        self.path.write_text(json.dumps(self.config))
        env = os.environ.copy()
        env["INST_JAVA_ARGS"] = TOKEN
        env.update(getattr(self, "env_extra", {}))
        if not WINE:
            env["TMPDIR"] = str(self.root)
        env["TMP"] = env["TEMP"] = native_path(self.root)
        bridge = getattr(self, "bridge", BRIDGE)
        command = ([WINE, native_path(bridge)] if WINE else [str(bridge)])
        options = [] if auto_config else ["--config", native_path(self.path)]
        # Text-mode pipes rewrite LF to CRLF on Windows, which would invalidate the
        # fixture's byte-for-byte stdin contract before the wrapper even receives it.
        result = subprocess.run([*command, *options, "--", *args], input=text.encode("utf-8"),
                                capture_output=True, env=env, cwd=self.game, timeout=20)
        result.stdout = result.stdout.decode("utf-8")
        result.stderr = result.stderr.decode("utf-8")
        self.assertNotIn(TOKEN, result.stdout + result.stderr)
        self.assertEqual(list(self.root.glob("rusted-session-*")), [])
        return result

    def test_auto_prism_loader_keeps_exact_stdin_and_arguments(self):
        self.config["mode"] = "auto"
        (self.root / "mmc-pack.json").write_text(json.dumps({"components":[{"uid":"net.fabricmc.fabric-loader","version":"FIXTURE"}]}))
        self.env_extra = {"INST_MC_DIR": native_path(self.game), "RUSTED_TEST_JAVA_LOADER": "PRISM_FIXTURE_ONLY"}
        frame = "mainClass net.fabricmc.loader.impl.launch.knot.KnotClient\nlauncher standard\nlaunch\n"
        result = self.invoke([native_path(PEER), "org.prismlauncher.EntryPoint"], frame)
        self.assertEqual(result.returncode, 37, result.stderr)
        self.assertIn("TEST_PEER_JAVA_LOADER_OK", result.stdout)
        self.assertFalse((self.root / "native data").exists())
        self.assertIn("moteur natif Rust est inactif", result.stderr)

    def test_auto_modrinth_fabric_retains_original_jvm(self):
        self.config["mode"] = "auto"
        self.env_extra = {"RUSTED_TEST_JAVA_LOADER": "FABRIC_FIXTURE_ONLY"}
        result = self.invoke([native_path(PEER), "net.fabricmc.loader.impl.launch.knot.KnotClient"], "JAVA_STDIN_UNCHANGED\n")
        self.assertEqual(result.returncode, 37, result.stderr)
        self.assertIn("TEST_PEER_JAVA_LOADER_OK", result.stdout)
        self.assertFalse((self.root / "native data").exists())

    def test_auto_vanilla_still_uses_native_engine(self):
        self.config["mode"] = "auto"
        result = self.invoke(["java", "net.minecraft.client.main.Main", *self.params])
        self.assertEqual(result.returncode, 37, result.stderr)
        self.assertIn("TEST_PEER_NATIVE_OK", result.stdout)

    def test_java_jars_are_not_silently_ignored(self):
        import zipfile
        mods = self.game / "mods"
        mods.mkdir()
        with zipfile.ZipFile(mods / "fabric-test.jar", "w") as archive:
            archive.writestr("fabric.mod.json", '{"id":"fixture","name":"Test Fixture"}')
        result = self.invoke(["java", "net.minecraft.client.main.Main", *self.params])
        self.assertEqual(result.returncode, 1)
        self.assertIn("Mods Java", result.stderr)
        self.assertNotIn("TEST_PEER_NATIVE_OK", result.stdout)

    def test_modrinth_native_handover_and_exit_status(self):
        result = self.invoke(["java", "-Xmx2G", "-cp", "path with spaces", "net.minecraft.client.main.Main", *self.params])
        self.assertEqual(result.returncode, 37, result.stderr)
        self.assertIn("TEST_PEER_NATIVE_OK", result.stdout)
        self.assertEqual(list(self.game.iterdir()), [])

    def test_prism_stdin_handover(self):
        script = "mainClass net.minecraft.client.main.Main\nlauncher modern\n"
        script += "".join("param " + param + "\n" for param in self.params) + "launch\n"
        result = self.invoke(["java", "-cp", "NewLaunch.jar", "org.prismlauncher.EntryPoint"], script)
        self.assertEqual(result.returncode, 37, result.stderr)
        self.assertIn("TEST_PEER_NATIVE_OK", result.stdout)

    def test_prism_standard_without_user_type_and_automatic_config(self):
        # Real Prism calls the current launcher "standard", and the official
        # Minecraft 26.3 arguments no longer include --userType.
        runtime = self.root / "runtime with spaces"
        runtime.mkdir()
        self.bridge = runtime / BRIDGE.name
        shutil.copy2(BRIDGE, self.bridge)
        self.path = runtime / "rusted-runtime.json"
        self.config.update(resources="../resources", data_directory="../native data")
        params = self.params.copy()
        index = params.index("--userType")
        del params[index:index + 2]
        script = "mainClass net.minecraft.client.main.Main\nlauncher standard\n"
        script += "".join("param " + param + "\n" for param in params) + "launch\n"
        result = self.invoke(["java", "-cp", "NewLaunch.jar", "org.prismlauncher.EntryPoint"], script, auto_config=True)
        self.assertEqual(result.returncode, 37, result.stderr)
        self.assertIn("TEST_PEER_NATIVE_OK", result.stdout)
        self.assertEqual(list(self.game.iterdir()), [])

    def test_print_wrapper_needs_no_config_or_account(self):
        command = [WINE, native_path(BRIDGE)] if WINE else [str(BRIDGE)]
        result = subprocess.run([*command, "--print-wrapper"], capture_output=True, text=True, timeout=20)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(result.stdout.strip().startswith('"'))
        self.assertTrue(result.stdout.strip().endswith('" --'))
        self.assertNotIn("\\", result.stdout)
        self.assertNotIn(TOKEN, result.stdout + result.stderr)

    @unittest.skipUnless(WINE or os.name == "nt", "Automatic configuration is Windows-only")
    def test_prism_automatic_setup_preserves_preferences_and_backup(self):
        instance = self.root / "Prism instance with spaces"
        runtime = instance / "minecraft/rusted"
        runtime.mkdir(parents=True)
        bridge = runtime / BRIDGE.name
        shutil.copy2(BRIDGE, bridge)
        original = "[General]\nConfigVersion=1.3\nname=Keep preferences\nJavaPath=C:/Java/javaw.exe\n"
        config = instance / "instance.cfg"
        config.write_text(original)
        (instance / "mmc-pack.json").write_text(json.dumps({"components":[{"uid":"net.minecraft","version":"26.3"}]}))
        command = ([WINE, native_path(bridge)] if WINE else [str(bridge)]) + ["--setup"]
        for _ in range(2):
            result = subprocess.run(command, capture_output=True, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("OverrideCommands=true", config.read_text())
        self.assertIn("name=Keep preferences", config.read_text())
        self.assertIn("JavaPath=C:/Java/javaw.exe", config.read_text())
        backups = list(instance.glob("instance.cfg.rusted-backup-*.ini"))
        self.assertEqual(len(backups), 1)
        self.assertEqual(backups[0].read_text(), original)

    @unittest.skipUnless(WINE or os.name == "nt", "Prism process guard is Windows-only")
    def test_automatic_setup_waits_for_prism_to_exit_before_writing(self):
        instance = self.root / "Prism guarded instance"
        runtime = instance / "minecraft/rusted"
        runtime.mkdir(parents=True)
        bridge = runtime / BRIDGE.name
        shutil.copy2(BRIDGE, bridge)
        fake = self.root / "PrismLauncher.exe"
        shutil.copy2(PEER, fake)
        config = instance / "instance.cfg"
        original = "[General]\nConfigVersion=1.3\nname=Preserved until closed\n"
        config.write_text(original)
        (instance / "mmc-pack.json").write_text(json.dumps({"components":[{"uid":"net.minecraft","version":"26.3"}]}))
        # Our own fixture blocks on stdin, standing in for a running Prism
        # process. It never loads Prism, Minecraft or an actual user account.
        prefix = [WINE] if WINE else []
        peer = subprocess.Popen([*prefix, native_path(fake), "-Xmx2G", "argument with spaces"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        setup = None
        try:
            time.sleep(0.5)
            setup = subprocess.Popen([*prefix, native_path(bridge), "--setup"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            time.sleep(1.5)
            self.assertIsNone(setup.poll())
            self.assertEqual(config.read_text(), original)
            peer.communicate("JAVA_STDIN_UNCHANGED\n", timeout=20)
            stdout, stderr = setup.communicate(timeout=30)
            self.assertEqual(setup.returncode, 0, stderr)
            self.assertIn("fermez Prism", stderr)
            self.assertIn("OverrideCommands=true", config.read_text())
            self.assertNotIn(TOKEN, stdout + stderr)
        finally:
            if peer.poll() is None:
                peer.communicate("JAVA_STDIN_UNCHANGED\n", timeout=20)
            if setup is not None and setup.poll() is None:
                setup.kill()
                setup.communicate(timeout=20)

    def test_prism_abort_never_starts_native_game(self):
        result = self.invoke(["java", "org.prismlauncher.EntryPoint"], "abort\n")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("TEST_PEER", result.stdout)
        self.assertFalse((self.root / "native data").exists())

    def test_java_mode_keeps_command_stdin_and_exit_status(self):
        self.config["mode"] = "java"
        self.config["resources"] = "missing resources are unused in Java mode"
        result = self.invoke([native_path(PEER), "-Xmx2G", "argument with spaces"], "JAVA_STDIN_UNCHANGED\n")
        self.assertEqual(result.returncode, 37, result.stderr)
        self.assertIn("TEST_PEER_JAVA_OK", result.stdout)

    def test_java_mode_keeps_crlf_and_unicode_stdin_exact(self):
        self.config["mode"] = "java"
        script = "JAVA_STDIN_UNCHANGED\r\nCuivre é\r\n"
        self.env_extra = {"RUSTED_TEST_EXPECT_STDIN": script}
        result = self.invoke([native_path(PEER), "-Xmx2G", "argument with spaces"], script)
        self.assertEqual(result.returncode, 37, result.stderr)
        self.assertIn("TEST_PEER_JAVA_OK", result.stdout)

    def test_rejects_java_mods_and_wrong_version(self):
        (self.game / "mods").mkdir()
        (self.game / "mods/sodium.jar").write_bytes(b"test fixture, not a mod")
        result = self.invoke(["java", "net.minecraft.client.main.Main", *self.params])
        self.assertEqual(result.returncode, 1)
        self.assertIn("Mods Java presents", result.stderr)
        shutil.rmtree(self.game / "mods")
        self.params[self.params.index("--version") + 1] = "1.20.4"
        result = self.invoke(["java", "net.minecraft.client.main.Main", *self.params])
        self.assertEqual(result.returncode, 1)
        self.assertIn("26.3", result.stderr)

    def test_same_save_folder_is_rejected(self):
        self.config["data_directory"] = native_path(self.game)
        result = self.invoke(["java", "net.minecraft.client.main.Main", *self.params])
        self.assertEqual(result.returncode, 1)

    def test_existing_unmarked_native_data_is_preserved(self):
        home = self.root / "native data"
        home.mkdir()
        save = home / "personal-save.dat"
        save.write_bytes(b"DO_NOT_CHANGE_TEST_FIXTURE")
        result = self.invoke(["java", "net.minecraft.client.main.Main", *self.params])
        self.assertEqual(result.returncode, 1)
        self.assertEqual(save.read_bytes(), b"DO_NOT_CHANGE_TEST_FIXTURE")
        self.assertEqual(len(list(home.iterdir())), 1)

    def test_failed_exec_leaves_no_session(self):
        # Existing but unexecutable on Unix / invalid PE on Windows.
        invalid = self.root / ("invalid" + SUFFIX)
        invalid.write_text("not an executable")
        self.config["executable"] = native_path(invalid)
        result = self.invoke(["java", "net.minecraft.client.main.Main", *self.params])
        self.assertEqual(result.returncode, 1)
        self.assertIn("Impossible de demarrer", result.stderr)


if __name__ == "__main__":
    if not WINE:
        subprocess.run(["cargo", "build", "--locked", "-p", "rusted-launch", "--bin", "rusted-launch", "--example", "protocol_peer"], cwd=ROOT, check=True)
    unittest.main()
