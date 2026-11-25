import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class SecurityPolicyContractTests(unittest.TestCase):
    def test_voicebot_risk_categories_and_privacy_contact_are_kept(self):
        security = (ROOT / "SECURITY.md").read_text()
        privacy = (ROOT / "PRIVACY.md").read_text()
        areas = security.split("## Privacy-Sensitive Areas", 1)[1]

        for phrase in (
            "Terminal Tool shell commands",
            "AppleScript app automation",
            "key presses sent to the active app",
            "configured MCP servers and their tools",
            "optional rewrite provider requests",
        ):
            with self.subTest(phrase=phrase):
                self.assertIn(phrase, areas)

        for phrase in ("global hotkeys",):
            with self.subTest(phrase=phrase):
                self.assertNotIn(phrase, areas)

        profile = "https://github.com/maarco"
        self.assertIn(profile, security)
        self.assertIn(profile, privacy)
        self.assertIn("not currently enabled for this\nrepository", security)
        self.assertIn("not currently enabled for this repository", privacy)


if __name__ == "__main__":
    unittest.main()
