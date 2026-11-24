"""Contract checks for the user-facing default Voicebot response style."""

from pathlib import Path
import unittest


PROMPT = Path(__file__).parents[1] / "system_prompt.default.md"


class DefaultPromptStyleTests(unittest.TestCase):
    def test_keeps_replies_concise_without_fixed_sentence_count(self):
        prompt = PROMPT.read_text(encoding="utf-8")
        self.assertIn("usually one or two short sentences", prompt)
        self.assertIn("clarity and conversation flow matter more than a rigid count", prompt)
        self.assertNotIn("Exactly one short sentence per reply", prompt)
        self.assertNotIn("never two", prompt)

    def test_allows_natural_casual_phrasing_without_forcing_slang(self):
        prompt = PROMPT.read_text(encoding="utf-8")
        self.assertIn("Match the user's conversational vibe", prompt)
        self.assertIn("without forcing slang", prompt)


if __name__ == "__main__":
    unittest.main()
