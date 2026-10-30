"""Runtime logs must never become source-archive or build-receipt inputs."""
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
import rusted


class SourceFingerprintTest(unittest.TestCase):
    def test_runtime_logs_do_not_change_receipts_but_engine_sources_do(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory)
            (source / 'src').mkdir()
            engine = source / 'src/engine.rs'
            engine.write_text('pub const VALUE: u32 = 1;')
            with patch.object(rusted, 'SOURCE', source):
                before = rusted.source_hash()
                (source / 'logs').mkdir()
                (source / 'logs/latest.log').write_text('OWN_TEST_RUNTIME_LOG')
                self.assertEqual(rusted.source_hash(), before)
                self.assertEqual(rusted.source_files(), [engine])
                engine.write_text('pub const VALUE: u32 = 2;')
                self.assertNotEqual(rusted.source_hash(), before)


if __name__ == '__main__':
    unittest.main()
