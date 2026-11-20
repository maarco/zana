"""Speech filter: what reaches text-to-speech. Standard library only (run in CI)."""
import asyncio
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from speech import (  # noqa: E402
    TOOL_SAY,
    join_sentences,
    number_words,
    one_sentence,
    speakable,
    speakable_stream,
)


async def collect(transform, chunks):
    async def gen():
        for c in chunks:
            yield c
    return "".join([x async for x in transform(gen())])


class SpeakableTest(unittest.TestCase):
    def test_links_paths_markup_and_hashes_are_never_spoken(self):
        s = speakable("**Done.** See https://github.com/x/y/pull/3 and `~/code/app/agent.py` at 75be6e7.\\n# Head")
        for bad in ("http", "/", "*", "\\", "75be6e7", "#"):
            self.assertNotIn(bad, s)
        for good in ("Done.", "a link", "a file path"):
            self.assertIn(good, s)

    def test_numbers_are_said_as_words(self):
        self.assertEqual(
            speakable("PR 350 is 80% done, v0.1.3, 3.5 GB, 1,200 files"),
            "PR three hundred fifty is eighty percent done, version zero point one point three, "
            "three point five GB, one thousand two hundred files")
        self.assertEqual(number_words(1234567),
                         "one million two hundred thirty four thousand five hundred sixty seven")
        self.assertEqual(number_words(0), "zero")

    def test_json_reads_as_words(self):
        s = speakable('{"open_prs": 3}')
        self.assertNotIn("{", s)
        self.assertNotIn('"', s)
        self.assertIn("open prs", s)
        self.assertIn("three", s)

    def test_stream_catches_links_split_across_chunks_and_drops_code(self):
        out = asyncio.run(collect(speakable_stream, ["Open https://exa", "mple.com/a.b now. ", "```py\nprint(1)\n", "``` ok!"]))
        self.assertNotIn("exa", out)
        self.assertNotIn("print", out)
        self.assertIn("ok!", out)
        self.assertIn("a link", out)

    def test_a_tool_calls_description_is_always_spoken(self):
        said = asyncio.run(collect(one_sentence, ["It's running as job one, wrapping up soon.",
                                                  " " + TOOL_SAY + "Checking if that job finished "]))
        self.assertEqual(" ".join(said.split()), "It's running as job one, wrapping up soon. Checking if that job finished")
        said = asyncio.run(collect(one_sentence, ["First real sentence right here. A second one gets cut. ",
                                                  " " + TOOL_SAY + "Peeking at job one "]))
        self.assertEqual(" ".join(said.split()), "First real sentence right here. Peeking at job one")
        said = asyncio.run(collect(one_sentence, [" " + TOOL_SAY + "Checking the skills ", "folder real quick "]))
        self.assertEqual(" ".join(said.split()), "Checking the skills folder real quick")

    def test_a_fixed_line_marked_TOOL_SAY_is_spoken_in_full(self):
        line = "Going into voice print mode. Talk naturally for about eight seconds."
        said = asyncio.run(collect(one_sentence, [TOOL_SAY + line]))
        self.assertEqual(" ".join(said.split()), line)
        # the marker also keeps a line whose first sentence would otherwise be dropped as a short opener
        said = asyncio.run(collect(one_sentence, [TOOL_SAY + "Goodbye. Say Zana when you need me."]))
        self.assertEqual(" ".join(said.split()), "Goodbye. Say Zana when you need me.")

    def test_an_unmarked_fixed_line_is_clipped_to_its_first_sentence(self):
        said = asyncio.run(collect(one_sentence, ["Going into voice print mode. Talk naturally for about eight seconds."]))
        self.assertEqual(" ".join(said.split()), "Going into voice print mode.")

    def test_step_descriptions_end_in_exactly_one_stop(self):
        self.assertEqual(join_sentences(["Checking your calendar!", "Reading the log", "Done.", "Ready? ", " "]),
                         "Checking your calendar! Reading the log. Done. Ready?")
        self.assertEqual(join_sentences(["Let me think..."]), "Let me think...")

    def test_one_sentence_per_reply(self):
        out = asyncio.run(collect(one_sentence, ["Checking your pull ", "requests now. Then I will ", "read the logs."]))
        self.assertEqual(out, "Checking your pull requests now.")
        self.assertEqual(asyncio.run(collect(one_sentence, ["No period at all"])), "No period at all")
        # a short filler opener is skipped so the real answer is what gets spoken
        self.assertEqual(
            asyncio.run(collect(one_sentence, ["I'm listening. You were just talking about ", "the full disk. Anything else?"])),
            "You were just talking about the full disk.")
        self.assertEqual(asyncio.run(collect(one_sentence, ["Sent."])), "Sent.")
        self.assertEqual(asyncio.run(collect(one_sentence, ["Okay. Done."])), "Done.")


if __name__ == "__main__":
    unittest.main()
