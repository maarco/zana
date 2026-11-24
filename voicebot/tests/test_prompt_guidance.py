"""Prompt assembly, migration safety, and current-tool guidance regressions."""
import ast
import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
import prompt_guidance
from listening import Transcript


class PromptGuidanceTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="zana-prompt-guidance-")
        self.home = Path(self.temp.name) / "home"
        self.home.mkdir()
        self.resources = ROOT

    def tearDown(self):
        self.temp.cleanup()

    def write_personal_prompt(self, text):
        path = self.home / "system_prompt.md"
        with path.open("w", encoding="utf-8", newline="") as stream:
            stream.write(text)
        return path

    def build(self):
        personal = prompt_guidance.load_personal_prompt(self.home, self.resources)
        guide = prompt_guidance.load_app_tool_guidance(self.resources)
        return prompt_guidance.assemble_instructions("Zana", guide, personal)

    def test_old_prompt_gets_current_guidance_without_editing_it(self):
        old_prompt = "My personal opening.\n\n" + prompt_guidance.LEGACY_TOOL_GUIDANCE + "\n\nKeep my sign-off.\n"
        prompt_path = self.write_personal_prompt(old_prompt)
        result = self.build()
        self.assertIn("check_for_update", result)
        self.assertIn("update_zana", result)
        self.assertIn("interrupt", result)
        self.assertLess(result.index("Current app-owned tool guidance"), result.index("User's personal instructions"))
        self.assertLess(result.index("Current app-owned tool guidance"), result.index("My personal opening"))
        with prompt_path.open("r", encoding="utf-8", newline="") as stream:
            self.assertEqual(stream.read(), old_prompt)

    def test_unreadable_personal_prompt_falls_back_to_shipped_default(self):
        (self.home / "system_prompt.md").mkdir()
        default_prompt = (self.resources / "system_prompt.default.md").read_text(encoding="utf-8")
        self.assertEqual(
            prompt_guidance.load_personal_prompt(self.home, self.resources), default_prompt
        )

    def test_app_guidance_survives_later_personal_prompt_edits(self):
        prompt_path = self.write_personal_prompt("First personal preference.\r\n")
        first = self.build()
        self.assertIn("run_terminal", first)
        second_prompt = "Second personal preference, in my own words.\r\n"
        with prompt_path.open("w", encoding="utf-8", newline="") as stream:
            stream.write(second_prompt)
        second = self.build()
        self.assertIn("Current app-owned tool guidance (version 1", second)
        self.assertIn("run_terminal", second)
        self.assertTrue(second.endswith(second_prompt))
        self.assertNotIn("First personal preference", second)

    def test_legacy_user_gets_one_persisted_offer_without_prompt_mutation(self):
        old_prompt = "Personal text.\n" + prompt_guidance.LEGACY_TOOL_GUIDANCE + "\nUnique addition.\n"
        path = self.write_personal_prompt(old_prompt)
        first = prompt_guidance.take_legacy_prompt_offer(self.home, old_prompt)
        self.assertIn("explicitly agrees", first)
        self.assertEqual(json.loads((self.home / prompt_guidance.MIGRATION_STATE_FILENAME).read_text())["offered"], True)
        with path.open("r", encoding="utf-8", newline="") as stream:
            self.assertEqual(stream.read(), old_prompt)

    def test_failed_offer_turn_retries_until_offer_is_in_transcript(self):
        old_prompt = "Personal text.\n" + prompt_guidance.LEGACY_TOOL_GUIDANCE
        self.write_personal_prompt(old_prompt)
        first = prompt_guidance.take_legacy_prompt_offer(self.home, old_prompt)
        retry = prompt_guidance.take_legacy_prompt_offer(self.home, old_prompt)
        self.assertTrue(first)
        self.assertTrue(retry, "a model failure before speaking must not consume the offer")

        transcript = (
            "[2026-11-24 10:00:00] Voicebot: Would you like me to remove the old tool guidance "
            "from your personal prompt?\n[2026-11-24 10:00:01] User: No\n"
        )
        self.assertEqual(prompt_guidance.take_legacy_prompt_offer(self.home, old_prompt, transcript), "")
        state = json.loads((self.home / prompt_guidance.MIGRATION_STATE_FILENAME).read_text())
        self.assertTrue(state["delivered"])
        self.assertEqual(prompt_guidance.take_legacy_prompt_offer(self.home, old_prompt), "")

    def test_no_legacy_guidance_means_no_offer_or_state_write(self):
        self.assertEqual(prompt_guidance.take_legacy_prompt_offer(self.home, "Only my personal rules."), "")
        self.assertFalse((self.home / prompt_guidance.MIGRATION_STATE_FILENAME).exists())

    def test_cleanup_requires_offer_and_explicit_consent(self):
        old_prompt = "Personal opening.\n" + prompt_guidance.LEGACY_TOOL_GUIDANCE + "\nMy unique ending.\n"
        unchanged, changed, _ = prompt_guidance.remove_legacy_guidance_after_consent(
            self.home, old_prompt, "yes, remove it"
        )
        self.assertFalse(changed)
        self.assertEqual(unchanged, old_prompt)
        prompt_guidance.take_legacy_prompt_offer(self.home, old_prompt)
        for reply in ("no", "not yet", "would that be safe?"):
            unchanged, changed, _ = prompt_guidance.remove_legacy_guidance_after_consent(self.home, old_prompt, reply)
            self.assertFalse(changed, reply)
            self.assertEqual(unchanged, old_prompt)
        for reply in (
            "yes, but don't remove it",
            "yes, I won't remove it",
            "okay, leave it alone",
            "sure, I changed my mind",
        ):
            self.assertFalse(prompt_guidance.explicitly_consents_to_cleanup(reply), reply)

    def test_bare_yes_is_not_consent_without_a_migration_offer_context(self):
        self.assertFalse(prompt_guidance.explicitly_consents_to_cleanup("yes"))

    def test_cleanup_rejects_bare_yes_without_a_recent_spoken_offer(self):
        old_prompt = "Personal opening.\n" + prompt_guidance.LEGACY_TOOL_GUIDANCE + "\nMy unique ending.\n"
        prompt_guidance.take_legacy_prompt_offer(self.home, old_prompt)
        unchanged, changed, _ = prompt_guidance.remove_legacy_guidance_after_consent(
            self.home, old_prompt, "yes"
        )
        self.assertFalse(changed)
        self.assertEqual(unchanged, old_prompt)

    def test_bare_yes_requires_the_exact_cleanup_question(self):
        prompt = "Personal intro.\n" + prompt_guidance.LEGACY_TOOL_GUIDANCE + "\nKeep this note.\n"
        exact_offer = (
            "[2026-11-24 10:00:00] Voicebot: Would you like me to remove the old tool guidance "
            "from your personal prompt?\n[2026-11-24 10:00:01] User: Yes\n"
        )
        prompt_guidance.take_legacy_prompt_offer(self.home, prompt)
        cleaned, changed, _ = prompt_guidance.remove_legacy_guidance_after_consent(
            self.home, prompt, "Yes", exact_offer
        )
        self.assertTrue(changed)
        self.assertNotIn("see_screen: the app and window", cleaned)

        unrelated_home = Path(self.temp.name) / "unrelated-home"
        unrelated_home.mkdir()
        prompt_guidance.take_legacy_prompt_offer(unrelated_home, prompt)
        unrelated_turn = (
            "[2026-11-24 10:00:00] Voicebot: I will not remove the old tool guidance "
            "from your personal prompt unless asked. Would you like me to close the current window?\n"
            "[2026-11-24 10:00:01] User: Yes\n"
        )
        unchanged, changed, _ = prompt_guidance.remove_legacy_guidance_after_consent(
            unrelated_home, prompt, "Yes", unrelated_turn
        )
        self.assertFalse(changed)
        self.assertEqual(unchanged, prompt)

    def test_bare_yes_after_an_example_mention_does_not_authorize_cleanup(self):
        prompt = "Personal note.\n" + prompt_guidance.LEGACY_TOOL_GUIDANCE + "\nKeep this.\n"
        prompt_guidance.take_legacy_prompt_offer(self.home, prompt)
        transcript = (
            "[2026-11-24 10:00:00] Voicebot: The example offer is: "
            "Would you like me to remove the old tool guidance from your personal prompt?\n"
            "[2026-11-24 10:00:01] User: Yes\n"
        )
        unchanged, changed, _ = prompt_guidance.remove_legacy_guidance_after_consent(
            self.home, prompt, "Yes", transcript
        )
        self.assertFalse(changed)
        self.assertEqual(unchanged, prompt)

    def test_bare_yes_after_prefixed_long_question_is_not_cleanup_consent(self):
        prompt = "Personal intro.\n" + prompt_guidance.LEGACY_TOOL_GUIDANCE
        prompt_guidance.take_legacy_prompt_offer(self.home, prompt)
        transcript = Transcript(self.home)
        transcript.record("Voicebot", "🙂" * 3000 + " " + prompt_guidance._MIGRATION_OFFER_QUESTION)
        transcript.record("User", "Yes")
        recent = transcript.tail_lines(2)
        self.assertTrue(recent.startswith("[2026-11-24 "))
        self.assertIn("Voicebot: " + "🙂" * 3000, recent)
        unchanged, changed, _ = prompt_guidance.remove_legacy_guidance_after_consent(
            self.home, prompt, "Yes", recent
        )
        self.assertFalse(changed)
        self.assertEqual(unchanged, prompt)

    def test_explicit_consent_removes_only_exact_stock_lines(self):
        custom_line = "My own tool rule: preserve every original phrase."
        old_prompt = (
            "My personal opening.\n"
            + prompt_guidance.LEGACY_TOOL_GUIDANCE
            + "\n"
            + custom_line
            + "\nKeep this unique note.\n"
        )
        prompt_guidance.take_legacy_prompt_offer(self.home, old_prompt)
        cleaned, changed, message = prompt_guidance.remove_legacy_guidance_after_consent(
            self.home, old_prompt, "Yes, please remove the old tool guidance."
        )
        self.assertTrue(changed)
        self.assertNotIn("see_screen: the app and window", cleaned)
        self.assertIn("My personal opening.", cleaned)
        self.assertIn(custom_line, cleaned)
        self.assertIn("Keep this unique note.", cleaned)
        self.assertIn("all other prompt text was preserved", message)

    def test_unrelated_delete_request_does_not_consent_to_prompt_cleanup(self):
        prompt = prompt_guidance.LEGACY_TOOL_GUIDANCE + "\nKeep this personal note.\n"
        prompt_guidance.take_legacy_prompt_offer(self.home, prompt)
        unchanged, changed, _ = prompt_guidance.remove_legacy_guidance_after_consent(
            self.home, prompt, "Yes, delete the old transcript file."
        )
        self.assertFalse(changed)
        self.assertEqual(unchanged, prompt)

    def test_unrelated_delete_request_after_offer_does_not_consent(self):
        prompt = prompt_guidance.LEGACY_TOOL_GUIDANCE + "\nKeep this personal note.\n"
        prompt_guidance.take_legacy_prompt_offer(self.home, prompt)
        transcript = (
            "[2026-11-24 10:00:00] Voicebot: "
            + prompt_guidance._MIGRATION_OFFER_QUESTION
            + "\n[2026-11-24 10:00:01] User: Yes, delete the old transcript file.\n"
        )
        unchanged, changed, _ = prompt_guidance.remove_legacy_guidance_after_consent(
            self.home, prompt, "Yes, delete the old transcript file.", transcript
        )
        self.assertFalse(changed)
        self.assertEqual(unchanged, prompt)

    def test_corrupt_migration_state_fails_closed(self):
        old_prompt = prompt_guidance.LEGACY_TOOL_GUIDANCE
        prompt_guidance.take_legacy_prompt_offer(self.home, old_prompt)
        state_path = self.home / prompt_guidance.MIGRATION_STATE_FILENAME
        state_path.write_text("{broken", encoding="utf-8")
        self.assertEqual(prompt_guidance.take_legacy_prompt_offer(self.home, old_prompt), "")
        cleaned, changed, _ = prompt_guidance.remove_legacy_guidance_after_consent(
            self.home, old_prompt, "yes"
        )
        self.assertFalse(changed)
        self.assertEqual(cleaned, old_prompt)

    def test_every_current_builtin_tool_is_named_in_shipped_guidance(self):
        tree = ast.parse((ROOT / "agent.py").read_text(encoding="utf-8"))
        voicebot = next(node for node in tree.body if isinstance(node, ast.ClassDef) and node.name == "Voicebot")
        tools = [
            node.name for node in voicebot.body
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
            and any(isinstance(dec, ast.Call) and getattr(dec.func, "id", None) == "function_tool"
                    for dec in node.decorator_list)
        ]
        guidance = prompt_guidance.load_app_tool_guidance(ROOT)
        self.assertEqual(len(tools), 28)
        for name in tools:
            with self.subTest(tool=name):
                self.assertIn(f"`{name}(", guidance)

    def test_default_personal_prompt_contains_no_builtin_tool_names(self):
        tree = ast.parse((ROOT / "agent.py").read_text(encoding="utf-8"))
        voicebot = next(node for node in tree.body if isinstance(node, ast.ClassDef) and node.name == "Voicebot")
        tools = [
            node.name for node in voicebot.body
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
            and any(isinstance(dec, ast.Call) and getattr(dec.func, "id", None) == "function_tool"
                    for dec in node.decorator_list)
        ]
        default_prompt = (ROOT / "system_prompt.default.md").read_text(encoding="utf-8")
        for name in tools:
            with self.subTest(tool=name):
                self.assertNotIn(name, default_prompt)

    def test_missing_app_guidance_fails_instead_of_silently_dropping_tools(self):
        with self.assertRaises(FileNotFoundError):
            prompt_guidance.load_app_tool_guidance(self.home)


if __name__ == "__main__":
    unittest.main()
