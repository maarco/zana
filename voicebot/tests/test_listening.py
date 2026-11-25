"""Wake word, always-on transcript and unread mark. Standard library only (run in CI)."""
import asyncio
import datetime
import os
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from listening import (ToolCallText, Transcript, clean, conversation_screenshot_path, heard_wake_word,
                       is_goodbye, normalize_retention_days, parse_text_tool_calls, remove_conversation_screenshot,
                       repeats, split_say, topics, wants_redo,  # noqa: E402
                       wants_voiceprint)


class WakeWordTest(unittest.TestCase):
    def test_matches_inside_a_sentence_and_small_mishearings(self):
        self.assertTrue(heard_wake_word("okay, hey voicebot what's up", "hey voicebot"))
        self.assertTrue(heard_wake_word("Hey voice bot, check this", "hey voicebot"))
        self.assertTrue(heard_wake_word("hey voicebut are you there", "hey voicebot"))

    def test_ignores_other_speech(self):
        self.assertFalse(heard_wake_word("the deploy failed because the disk was full", "hey voicebot"))
        self.assertFalse(heard_wake_word("", "hey voicebot"))

    def test_empty_wake_word_always_answers(self):
        self.assertTrue(heard_wake_word("anything at all", ""))


class GoodbyeTest(unittest.TestCase):
    def test_short_sign_offs_count(self):
        for text in ("Goodbye.", "ok bye", "Stop listening!", "that's all, thanks", "good night voicebot"):
            self.assertTrue(is_goodbye(text), text)

    def test_longer_sentences_and_other_words_do_not(self):
        for text in ("I said goodbye to her yesterday at the airport", "bypass the cache", "", "maybe later"):
            self.assertFalse(is_goodbye(text), text)


class VoicePrintIntentTest(unittest.TestCase):
    def test_enrollment_requests(self):
        for text in ("Let's enroll my voice in voice print", "set up my voice print", "voice print mode",
                     "redo my voiceprint please", "hey voicebot, enroll my voice", "Roll my voice."):
            self.assertTrue(wants_voiceprint(text), text)

    def test_redo_and_enable(self):
        self.assertTrue(wants_voiceprint("can you enable voice print"))
        self.assertTrue(wants_redo("redo my voice print"))
        self.assertTrue(wants_redo("record a new voice print"))
        self.assertFalse(wants_redo("enroll my voice"))

    def test_topics_pick_the_features_mentioned(self):
        self.assertEqual(topics("is my voice print set up?"), ["voiceprint"])
        self.assertIn("mcp", topics("which MCP servers do you have"))
        self.assertIn("wake", topics("what is your wake word"))
        self.assertEqual(topics("what's the weather like"), [])
        self.assertIn("prompt", topics("open the system prompt file"))

    def test_other_mentions(self):
        for text in ("what is a voice print", "my voice is tired", "print the report"):
            self.assertFalse(wants_voiceprint(text), text)


class TranscriptTest(unittest.TestCase):
    def test_unread_is_what_came_after_the_last_read(self):
        with tempfile.TemporaryDirectory() as home:
            t = Transcript(home)
            t.record("User", "first, already read")
            t.mark_read()
            t.record("User", "said while asleep")
            t.record("User", "  spaced   out\nline ")
            unread = t.take_unread()
            self.assertNotIn("already read", unread)
            self.assertIn("User: said while asleep", unread)
            self.assertIn("User: spaced out line", unread)
            self.assertEqual(t.take_unread(), "")  # reading moves the mark
            t.record("Voicebot", "")  # blank lines are not recorded
            self.assertEqual(t.take_unread(), "")
            with open(os.path.join(home, "transcript.md")) as f:
                self.assertEqual(len(f.read().splitlines()), 3)  # retention is age-based, not per-write

    def test_retention_drops_only_old_dated_lines_and_preserves_unknown_lines(self):
        now = datetime.datetime(2026, 11, 24, 12, 0, 0)
        old = b"[2026-10-24 11:59:59] User: synthetic old line\n"
        boundary = b"[2026-10-25 12:00:00] User: boundary line\n"
        recent = b"[2026-11-24 11:00:00] Voicebot: recent line\n"
        unknown = b"legacy line without a timestamp\n"
        with tempfile.TemporaryDirectory() as home:
            t = Transcript(home, retention_days=30, now=now)
            with open(t.path, "wb") as stream:
                stream.write(old + boundary + recent + unknown)
            with open(t.mark, "w") as stream:
                stream.write(str(len(old)))

            self.assertEqual(t.prune_old(now=now), 1)
            with open(t.path, "rb") as stream:
                content = stream.read()
            self.assertNotIn(old, content)
            self.assertIn(boundary, content)
            self.assertIn(recent, content)
            self.assertIn(unknown, content)
            self.assertEqual(t.take_unread(), (boundary + recent + unknown).decode())

    def test_clear_resets_transcript_and_unread_marker_without_touching_neighbors(self):
        with tempfile.TemporaryDirectory() as home:
            t = Transcript(home)
            t.record("User", "synthetic private line")
            t.mark_read()
            unrelated = os.path.join(home, "config.json")
            with open(unrelated, "w") as stream:
                stream.write("keep")

            t.clear()

            with open(t.path) as stream:
                self.assertEqual(stream.read(), "")
            with open(t.mark) as stream:
                self.assertEqual(stream.read(), "0")
            with open(unrelated) as stream:
                self.assertEqual(stream.read(), "keep")

    def test_screenshot_cleanup_requires_a_scoped_id(self):
        conversation_id = "a" * 32
        with tempfile.TemporaryDirectory() as home:
            scoped = conversation_screenshot_path(home, conversation_id)
            unrelated = os.path.join(home, "screen-other.jpg")
            with open(scoped, "wb") as stream:
                stream.write(b"synthetic screenshot")
            with open(unrelated, "wb") as stream:
                stream.write(b"keep")

            remove_conversation_screenshot(home, conversation_id)

            self.assertFalse(os.path.exists(scoped))
            self.assertTrue(os.path.exists(unrelated))
            with self.assertRaises(ValueError):
                conversation_screenshot_path(home, "../outside")

    def test_retention_setting_is_bounded(self):
        self.assertEqual(normalize_retention_days(None), 30)
        self.assertEqual(normalize_retention_days(0), 30)
        self.assertEqual(normalize_retention_days(99999), 3650)

    def test_tail_keeps_whole_newest_lines(self):
        with tempfile.TemporaryDirectory() as home:
            t = Transcript(home)
            for i in range(30):
                t.record("User", f"line number {i}")
            tail = t.tail(120)
            self.assertIn("line number 29", tail)
            self.assertTrue(tail.startswith("["))  # no partial first line
            self.assertLessEqual(len(tail), 120)

    def test_tail_lines_keeps_complete_long_turns(self):
        with tempfile.TemporaryDirectory() as home:
            t = Transcript(home)
            t.record("Voicebot", "Would you like me to remove the old tool guidance?")
            long_reply = "Yes, I agree. " + ("More context. " * 400)
            t.record("User", long_reply)
            tail = t.tail_lines(2)
            self.assertIn("Voicebot: Would you like me to remove the old tool guidance?", tail)
            self.assertTrue(tail.endswith("User: " + long_reply.strip()))

    def test_unread_is_capped_to_the_newest_text(self):
        with tempfile.TemporaryDirectory() as home:
            t = Transcript(home, unread_limit=50)
            for i in range(20):
                t.record("User", f"line {i}")
            unread = t.take_unread()
            self.assertLessEqual(len(unread), 50)
            self.assertIn("line 19", unread)


class CleanTest(unittest.TestCase):
    def test_hides_secrets_and_colors_and_trims(self):
        out = clean("\x1b[31mred\x1b[0m API_KEY=abc123 token: zzz " + "x" * 40)
        self.assertNotIn("\x1b", out)
        self.assertNotIn("abc123", out)
        self.assertNotIn("zzz", out)
        long = clean("a" * 10 + " " + "b " * 3000, limit=100)
        self.assertLessEqual(len(long), 110)



class TranscriptSearchTest(unittest.TestCase):
    def test_finds_lines_with_every_word_and_the_reply_after(self):
        with tempfile.TemporaryDirectory() as home:
            t = Transcript(home)
            for who, text in [("User", "find the PTY manager skill"), ("Voicebot", "It lives in your skills folder."),
                              ("User", "what time is it"), ("Voicebot", "Ten past eleven."),
                              ("User", "open the manager again")]:
                t.record(who, text)
            found = t.search("pty managers")  # word starts match, so plurals still find it
            self.assertEqual(len(found), 2)
            self.assertIn("PTY manager", found[0])
            self.assertIn("skills folder", found[1])

    def test_falls_back_to_the_lines_with_the_most_words(self):
        with tempfile.TemporaryDirectory() as home:
            t = Transcript(home)
            t.record("User", "open the manager again")
            t.record("User", "nothing here")
            self.assertIn("manager", t.search("pty manager")[0])
            self.assertEqual(t.search("zebra"), [])
            self.assertEqual(t.search(""), [])

    def test_search_matches_terms_split_across_bounded_reads(self):
        with tempfile.TemporaryDirectory() as home:
            t = Transcript(home)
            header = "[2026-11-24 00:00:00] User: "
            padding = "x" * (16 * 1024 - len(header) - 4) + " "
            with open(t.path, "w") as stream:
                stream.write(header + padding + "needle\n")
            self.assertTrue(any("needle" in line for line in t.search("needle")))


class TranscriptSearchResponsivenessTest(unittest.TestCase):
    def test_search_runs_off_the_event_loop(self):
        with tempfile.TemporaryDirectory() as home:
            transcript = Transcript(home)

            def slow_search(_words, _limit=12):
                time.sleep(0.2)
                return ["found"]

            async def run():
                started = time.monotonic()
                with patch.object(transcript, "search", slow_search):
                    result, heartbeat = await asyncio.gather(
                        transcript.search_async("query"),
                        self._heartbeat(started),
                    )
                return result, heartbeat

            result, heartbeat = asyncio.run(run())
            self.assertEqual(result, ["found"])
            self.assertLess(heartbeat, 0.1)

    @staticmethod
    async def _heartbeat(started):
        await asyncio.sleep(0.01)
        return time.monotonic() - started

    def test_search_on_50mb_transcript_finishes_within_two_seconds(self):
        with tempfile.TemporaryDirectory() as home:
            transcript = Transcript(home)
            line_size = 16 * 1024
            prefix = "[2026-11-24 00:00:00] User: "
            ordinary = prefix + "x" * (line_size - len(prefix) - 1) + "\n"
            final = "[2026-11-24 00:00:00] User: needle at the end\n"
            repetitions = (50 * 1024 * 1024) // len(ordinary)
            with open(transcript.path, "w") as f:
                f.write(ordinary * repetitions)
                f.write(final)
            self.assertGreaterEqual(os.path.getsize(transcript.path), 50 * 1024 * 1024)

            started = time.monotonic()
            result = asyncio.run(transcript.search_async("needle"))
            elapsed = time.monotonic() - started
            self.assertTrue(any("needle at the end" in line for line in result))
            self.assertLess(elapsed, 2.0, f"search took {elapsed:.2f}s")


class TextToolCallTest(unittest.TestCase):
    def test_reads_both_written_out_formats(self):
        said = ('Let me check. <|tool_call_begin|>tool_name: search_conversations, arguments: {"words": "a, {b}"}, '
                'reasoning: None, call_id: None, type: tool_call<|tool_call_end|>'
                '<|tool_call_begin|>functions.list_jobs:0<|tool_call_argument_begin|>{}<|tool_call_end|>')
        self.assertEqual(parse_text_tool_calls(said),
                         [("search_conversations", '{"words": "a, {b}"}'), ("list_jobs", "{}")])
        self.assertEqual(parse_text_tool_calls("<|tool_call_begin|>nonsense<|tool_call_end|>"), [])
        self.assertEqual(parse_text_tool_calls("plain words"), [])


class MoreTextToolCallTest(unittest.TestCase):
    def test_reads_glm_hermes_and_function_formats(self):
        glm = ('<tool_call>run_terminal<arg_key>command</arg_key><arg_value>grep -l todo ~/x | head -5'
               '</arg_value></tool_call>')
        self.assertEqual(parse_text_tool_calls(glm), [("run_terminal", '{"command": "grep -l todo ~/x | head -5"}')])
        self.assertEqual(parse_text_tool_calls('<tool_call><arg_key>x</arg_key></tool_call>'), [])
        hermes = '<tool_call>\n{"name": "check_job", "arguments": {"job": 1}}\n</tool_call>'
        self.assertEqual(parse_text_tool_calls(hermes), [("check_job", '{"job": 1}')])
        qwen = '<function=check_job><parameter=job>2</parameter></function>'
        self.assertEqual(parse_text_tool_calls(qwen), [("check_job", '{"job": 2}')])

    def test_holds_back_tool_call_text_even_when_a_marker_is_cut_off(self):
        split = ToolCallText()
        spoken = split.feed("Sure, let me look. <tool") + split.feed("_call>run_terminal<arg_key>c</arg_key>")
        spoken += split.feed("<arg_value>ls</arg_value></tool_call>") + split.finish()
        self.assertEqual(spoken, "Sure, let me look. ")
        self.assertEqual(parse_text_tool_calls(split.held), [("run_terminal", '{"c": "ls"}')])

    def test_plain_angle_brackets_are_still_spoken(self):
        split = ToolCallText()
        spoken = split.feed("Two < three, and <") + split.feed("b> is bold.") + split.finish()
        self.assertEqual(spoken, "Two < three, and <b> is bold.")
        self.assertEqual(split.held, "")


class SplitSayTest(unittest.TestCase):
    def test_takes_the_spoken_field_out_of_the_arguments(self):
        self.assertEqual(split_say('{"say": "Let me check your notes folder", "command": "ls ~/notes"}'),
                         ("Let me check your notes folder", '{"command": "ls ~/notes"}'))
        self.assertEqual(split_say('{"job": 1}'), ("", '{"job": 1}'))
        self.assertEqual(split_say("not json"), ("", "not json"))


class RepeatsTest(unittest.TestCase):
    def test_spots_a_description_already_said_in_the_step(self):
        self.assertTrue(repeats("Hm, first one printed one, on to the next",
                                'Hm, first one printed "one", on to the next.'))
        self.assertFalse(repeats("Running the third and final echo",
                                 "Alright, that printed two, last one coming up."))
        self.assertFalse(repeats("Checking the logs", ""))

if __name__ == "__main__":
    unittest.main()
