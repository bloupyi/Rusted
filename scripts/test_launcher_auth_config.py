#!/usr/bin/env python3
"""Distribution identity regression tests; no login/network or user data."""
import os
import unittest
from unittest.mock import patch
from package_launcher import publisher_auth_config, DEFAULT_MICROSOFT_CLIENT_ID


class PublisherAuthConfigTests(unittest.TestCase):
    def test_standard_distribution_needs_no_publisher_variable(self):
        with patch.dict(os.environ, {"RUSTED_MICROSOFT_CLIENT_ID": ""}):
            self.assertEqual(publisher_auth_config(), {"schema": 1, "microsoft_client_id": DEFAULT_MICROSOFT_CLIENT_ID})

    def test_custom_registered_identity_can_override_the_default(self):
        for client in [DEFAULT_MICROSOFT_CLIENT_ID, "11111111-2222-4333-8444-555555555555"]:
            with self.subTest(client=client), patch.dict(os.environ, {"RUSTED_MICROSOFT_CLIENT_ID": client}):
                self.assertEqual(publisher_auth_config()["microsoft_client_id"], client)

    def test_invalid_override_does_not_silently_switch_identity(self):
        for client in ["00000000-0000-0000-0000-000000000000", "------------------------------------", "11111111222243338444555555555555", "not-an-app-id", "00000000402b5329"]:
            with self.subTest(client=client), patch.dict(os.environ, {"RUSTED_MICROSOFT_CLIENT_ID": client}):
                with self.assertRaisesRegex(ValueError, "Invalid publisher"):
                    publisher_auth_config()


if __name__ == "__main__":
    unittest.main()
