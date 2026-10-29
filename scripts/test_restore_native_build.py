#!/usr/bin/env python3
"""Native artifact reuse must preserve the source/receipt/archive identity checks."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

from restore_native_build import restore


class RestoreNativeTests(unittest.TestCase):
    def fixture(self, folder, source="OWN_SOURCE", corrupt_receipt=False):
        archive = folder / "native.zip"
        engine, preparer = b"OWN_TEST_ENGINE", b"OWN_TEST_PREPARER"
        receipt = {"source_sha256": source, "binaries": {
            "rusted.exe": hashlib.sha256(engine).hexdigest(),
            "horizon-prepare.exe": "BAD_HASH" if corrupt_receipt else hashlib.sha256(preparer).hexdigest(),
        }}
        with zipfile.ZipFile(archive, "w") as bundle:
            bundle.writestr("Rusted/rusted-build.json", json.dumps(receipt))
            bundle.writestr("Rusted/runtime/rusted.exe", engine)
            bundle.writestr("Rusted/runtime/rusted-prepare.exe", preparer)
            bundle.writestr("Rusted/not-restored", b"UNRELATED_FILE")
        return archive, hashlib.sha256(archive.read_bytes()).hexdigest()

    def test_only_verified_binaries_and_receipt_are_restored(self):
        with tempfile.TemporaryDirectory() as owned, patch("restore_native_build.source_hash", return_value="OWN_SOURCE"):
            folder = Path(owned); archive, digest = self.fixture(folder); out = folder / "output"
            self.assertTrue(restore(archive, digest, out))
            self.assertEqual(set(p.name for p in out.iterdir()), {"rusted.exe", "horizon-prepare.exe", "rusted-build.json"})
            self.assertEqual((out / "rusted.exe").read_bytes(), b"OWN_TEST_ENGINE")

    def test_mismatch_never_writes_partial_binaries(self):
        with tempfile.TemporaryDirectory() as owned, patch("restore_native_build.source_hash", return_value="OWN_SOURCE"):
            folder = Path(owned); out = folder / "output"
            self.assertFalse(restore(folder / "missing.zip", "", out))
            archive, digest = self.fixture(folder)
            self.assertFalse(restore(archive, "BAD_ARCHIVE_HASH", out))
            archive, digest = self.fixture(folder, source="STALE_SOURCE")
            self.assertFalse(restore(archive, digest, out))
            archive, digest = self.fixture(folder, corrupt_receipt=True)
            self.assertFalse(restore(archive, digest, out))
            self.assertFalse(out.exists())


if __name__ == "__main__":
    unittest.main()
