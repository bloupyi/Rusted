"""Only our explicitly listed Java API facades may enter the public tree."""
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('public_tree', ROOT / 'tools/check_public_tree.py')
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)

class JavaFacadeAudit(unittest.TestCase):
    def test_original_sources_are_explicit_and_present(self):
        self.assertTrue(audit.ORIGINAL_RUSTED_FACADES)
        for source in audit.ORIGINAL_RUSTED_FACADES:
            self.assertTrue((ROOT / source).is_file())
            self.assertFalse(audit.prohibited(source))

    def test_unknown_java_and_all_game_artifacts_stay_private(self):
        for source in ['crates/rusted-java/java/net/minecraft/world/Block.java',
                       'crates/rusted-java/java/net/minecraft/client/Minecraft.class',
                       'crates/rusted-java/java/third-party.jar',
                       '.local/minecraft.jar', 'decompiled/net/minecraft/Client.java']:
            self.assertTrue(audit.prohibited(source), source)

if __name__ == '__main__':
    unittest.main()
