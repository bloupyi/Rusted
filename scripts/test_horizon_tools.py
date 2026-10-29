#!/usr/bin/env python3
"""Exercise the pinned upstream CLI with original metadata/data fixtures only."""
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[1]
DRIVER = ROOT / "scripts/horizon.py"


class HorizonToolTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="rusted metadata ")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

    def invoke(self, *args, ok=True):
        result = subprocess.run([os.sys.executable, str(DRIVER), *map(str, args)], cwd=ROOT, text=True, capture_output=True, timeout=30)
        self.assertEqual(result.returncode == 0, ok, result.stdout + result.stderr)
        return result

    def test_fabric_forge_neoforge_metadata_and_data(self):
        for loader, path in [("fabric", "fabric.mod.json"), ("forge", "META-INF/mods.toml"), ("neoforge", "META-INF/neoforge.mods.toml")]:
            with self.subTest(loader=loader):
                jar = self.root / (loader + ".jar")
                if loader == "fabric":
                    text = json.dumps({"schemaVersion": 1, "id": "rusted_fixture", "name": "Original test fixture", "version": "0.1.0", "license": "MIT"})
                else:
                    text = 'license="MIT"\n[[mods]]\nmodId="rusted_fixture"\nversion="0.1.0"\ndisplayName="Original test fixture"\n'
                with zipfile.ZipFile(jar, "w") as archive:
                    archive.writestr(path, text)
                    archive.writestr("data/rusted_fixture/recipe/test.json", json.dumps({"type": "minecraft:crafting_shapeless", "ingredients": ["minecraft:stone"], "result": {"id": "minecraft:stone", "count": 1}}))
                work = self.root / (loader + "-report")
                self.invoke("check-mod", jar, "--work", work)
                report = json.loads((work / "check.json").read_text())
                self.assertEqual((report["loader"], report["mod_id"], report["license"], report["code"]["classes"]), (loader, "rusted_fixture", "MIT", 0))
                self.assertTrue((work / "carry-report.json").is_file())
                # Re-running must leave the existing analysis intact.
                before = (work / "check.json").read_bytes()
                self.invoke("check-mod", jar, "--work", work, ok=False)
                self.assertEqual((work / "check.json").read_bytes(), before)

    def test_modrinth_import_is_offline_and_keeps_loader_metadata(self):
        pack = self.root / "test.mrpack"
        with zipfile.ZipFile(pack, "w") as archive:
            archive.writestr("modrinth.index.json", json.dumps({"formatVersion": 1, "game": "minecraft", "versionId": "fixture-1", "name": "Original empty fixture", "dependencies": {"minecraft": "26.3", "fabric-loader": "0.16.0"}, "files": []}))
        work = self.root / "pack-report"
        self.invoke("inventory", pack, "--work", work)
        report = json.loads((work / "pack.json").read_text())
        self.assertEqual((report["format"], report["minecraft"], report["loader"]), ("mrpack", "26.3", "fabric"))
        self.assertTrue((work / "coverage.json").is_file())
        self.assertTrue((work / "coverage.md").is_file())


if __name__ == "__main__":
    unittest.main()
