#!/usr/bin/env python3
"""Publisher release gate tests; synthetic public GUIDs only, no login/network."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from package_launcher import publisher_auth_config


class PublisherAuthConfigTests(unittest.TestCase):
    def test_missing_identity_blocks_distribution_before_creating_files(self):
        with patch.dict(os.environ, {"RUSTED_MICROSOFT_CLIENT_ID": ""}):
            with self.assertRaisesRegex(ValueError, "publisher must register"):
                publisher_auth_config()
            with tempfile.TemporaryDirectory() as owned:
                output = Path(owned) / "must-not-exist"
                result = subprocess.run(
                    [sys.executable, str(Path(__file__).with_name("package_launcher.py")), "--output-dir", str(output)],
                    capture_output=True, text=True, check=False,
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("Microsoft login is not configured", result.stderr)
                self.assertFalse(output.exists())

    def test_publisher_identity_is_bundled_and_preview_is_explicit(self):
        client = "11111111-2222-4333-8444-555555555555"
        with patch.dict(os.environ, {"RUSTED_MICROSOFT_CLIENT_ID": client}):
            self.assertEqual(publisher_auth_config(), {"schema": 1, "microsoft_client_id": client})
        with patch.dict(os.environ, {"RUSTED_MICROSOFT_CLIENT_ID": ""}):
            self.assertEqual(publisher_auth_config(allow_unconfigured=True)["microsoft_client_id"], "")

    def test_invalid_identity_is_never_shipped_even_in_preview(self):
        for client in ["00000000-0000-0000-0000-000000000000", "------------------------------------", "11111111222243338444555555555555", "not-an-app-id"]:
            with self.subTest(client=client), patch.dict(os.environ, {"RUSTED_MICROSOFT_CLIENT_ID": client}):
                with self.assertRaisesRegex(ValueError, "Invalid publisher"):
                    publisher_auth_config(allow_unconfigured=True)


if __name__ == "__main__":
    unittest.main()
