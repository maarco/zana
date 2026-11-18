"""Regression tests for tool calls across Voicebot conversation compaction."""
import asyncio
import importlib.util
import json
import os
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))

try:
    from livekit.agents import Agent, AgentSession, function_tool, llm
    from livekit.agents.types import DEFAULT_API_CONNECT_OPTIONS
except ModuleNotFoundError as exc:
    if exc.name != "livekit":
        raise
    DEPENDENCY_ERROR = str(exc)
else:
    TEST_HOME = tempfile.TemporaryDirectory(prefix="zana-compaction-test-")
    spec = importlib.util.spec_from_file_location(
        "_zana_compaction_agent", os.path.join(os.path.dirname(os.path.dirname(__file__)), "agent.py"),
    )
    agent_module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = agent_module
    with patch.dict(os.environ, {"VOICEBOT_HOME": TEST_HOME.name}):
        spec.loader.exec_module(agent_module)
    Voicebot = agent_module.Voicebot
    DEPENDENCY_ERROR = None


def tearDownModule():
    if DEPENDENCY_ERROR is None:
        sys.modules.pop(spec.name, None)
        TEST_HOME.cleanup()


if DEPENDENCY_ERROR is None:
    class ScriptedLLM(llm.LLM):
        """A local LLM that needs the retained tool result to make its next call."""

        def __init__(self):
            super().__init__()
            self.followup_requests = []
            self.summary_started = asyncio.Event()
            self.summary_release = asyncio.Event()
            self.summary_release.set()

        def chat(self, *, chat_ctx, tools=None, **kwargs):
            return ScriptedStream(
                self, chat_ctx=chat_ctx, tools=tools or [],
                conn_options=DEFAULT_API_CONNECT_OPTIONS,
            )


    class ScriptedStream(llm.LLMStream):
        async def _run(self):
            if not self._tools:
                self._llm.summary_started.set()
                await self._llm.summary_release.wait()
                delta = llm.ChoiceDelta(role="assistant", content="Earlier conversation summary.")
            else:
                messages, _ = self._chat_ctx.to_provider_format("openai")
                self._llm.followup_requests.append(messages)
                if any(item.type == "function_call_output" and item.call_id == "followup"
                       for item in self._chat_ctx.items):
                    delta = llm.ChoiceDelta(role="assistant", content="The switch is on.")
                elif any(message.get("role") == "tool" and message.get("tool_call_id") == "old"
                         and message.get("content") == "switch-42" for message in messages):
                    delta = llm.ChoiceDelta(role="assistant", tool_calls=[llm.FunctionToolCall(
                        name="check_status", call_id="followup",
                        arguments=json.dumps({"say": "Checking it again", "target": "switch-42"}),
                    )])
                else:
                    delta = llm.ChoiceDelta(role="assistant", content="I cannot find the switch.")
            self._event_ch.send_nowait(llm.ChatChunk(id="scripted", delta=delta))


    class TestVoicebot(Voicebot):
        def __init__(self, chat_ctx):
            # Avoid starting voiceprint/model background work; exercise the real
            # compaction and llm_node with a real LiveKit session below.
            Agent.__init__(self, instructions="Test instructions", chat_ctx=chat_ctx)
            self.base_instructions = "Test instructions"
            self.summary = ""
            self.compact_chars = 1000
            self.compact_at = 1000
            self.compacting = True
            self.fillers_on = False
            self.checked = []

        async def on_user_turn_completed(self, turn_ctx, new_message):
            pass

        @function_tool()
        async def check_status(self, target: str) -> str:
            """Check the named switch."""
            self.checked.append(target)
            return "on"


def history_with_split_tool_result():
    """The last six messages start at an interleaved message after the tool call."""
    ctx = llm.ChatContext.empty()
    ctx.add_message(role="user", content="Hello")
    ctx.add_message(role="assistant", content="Hello")
    ctx.add_message(role="user", content="Find the switch")
    ctx.items.append(llm.FunctionCall(
        name="check_status", call_id="old", arguments='{"target":"discovery"}',
    ))
    ctx.add_message(role="assistant", content="A background job has finished.")
    ctx.items.append(llm.FunctionCallOutput(
        name="check_status", call_id="old", output="switch-42", is_error=False,
    ))
    ctx.add_message(role="assistant", content="Found it.")
    ctx.add_message(role="user", content="Thanks")
    ctx.add_message(role="assistant", content="Sure")
    ctx.add_message(role="user", content="Remember it")
    ctx.add_message(role="assistant", content="Okay")
    return ctx


@unittest.skipIf(DEPENDENCY_ERROR is not None, f"Voicebot dependencies unavailable: {DEPENDENCY_ERROR}")
class CompactionTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.debug = patch.object(agent_module, "debug")
        self.debug_mock = self.debug.start()
        self.addCleanup(self.debug.stop)

    async def start_bot(self, ctx, model=None):
        model = model or ScriptedLLM()
        bot = TestVoicebot(ctx)
        session = AgentSession(llm=model)
        self.addAsyncCleanup(session.aclose)
        await session.start(bot, record=False)
        return bot, session, model

    async def assert_followup_runs(self, bot, session, model):
        handle = session.generate_reply(user_input="Check the switch again")
        await asyncio.wait_for(handle, timeout=5)
        self.assertIsNone(handle.exception())
        self.assertEqual(bot.checked, ["switch-42"])
        self.assertTrue(any(
            message.get("role") == "tool" and message.get("tool_call_id") == "old"
            for message in model.followup_requests[0]
        ))
        self.assertGreaterEqual(len(model.followup_requests), 2)
        self.assertTrue(any(item.type == "message" and item.role == "assistant"
                            and "The switch is on." in (item.text_content or "")
                            for item in bot.chat_ctx.items))

    async def test_followup_tool_runs_after_compaction_splits_call_and_output(self):
        bot, session, model = await self.start_bot(history_with_split_tool_result())
        await bot.compact()
        self.assertFalse(bot.compacting)
        self.assertEqual(bot.summary, "Earlier conversation summary.")
        messages, _ = bot.chat_ctx.to_provider_format("openai")
        self.assertEqual(messages[0]["role"], "system")
        items = bot.chat_ctx.items
        call = next(i for i, item in enumerate(items)
                    if item.type == "function_call" and item.call_id == "old")
        interleaved = next(i for i, item in enumerate(items)
                           if item.type == "message" and item.text_content == "A background job has finished.")
        result = next(i for i, item in enumerate(items)
                      if item.type == "function_call_output" and item.call_id == "old")
        self.assertLess(call, interleaved)
        self.assertLess(interleaved, result)
        await self.assert_followup_runs(bot, session, model)

    async def test_result_arriving_during_summarization_keeps_its_call(self):
        ctx = history_with_split_tool_result()
        ctx.items = [item for item in ctx.items if item.type != "function_call_output"]
        model = ScriptedLLM()
        model.summary_release.clear()
        bot, session, model = await self.start_bot(ctx, model)
        task = asyncio.create_task(bot.compact())
        try:
            await asyncio.wait_for(model.summary_started.wait(), timeout=5)
            current = bot.chat_ctx.copy()
            current.items.append(llm.FunctionCallOutput(
                name="check_status", call_id="old", output="switch-42", is_error=False,
            ))
            await bot.update_chat_ctx(current)
        finally:
            model.summary_release.set()
            await asyncio.wait_for(task, timeout=5)
        await self.assert_followup_runs(bot, session, model)

    async def test_new_tool_exchange_arriving_during_summarization_is_kept(self):
        ctx = history_with_split_tool_result()
        ctx.items = [item for item in ctx.items
                     if item.type not in ("function_call", "function_call_output")]
        model = ScriptedLLM()
        model.summary_release.clear()
        bot, session, model = await self.start_bot(ctx, model)
        task = asyncio.create_task(bot.compact())
        try:
            await asyncio.wait_for(model.summary_started.wait(), timeout=5)
            current = bot.chat_ctx.copy()
            current.items.append(llm.FunctionCall(
                name="check_status", call_id="old", arguments='{"target":"discovery"}',
            ))
            current.items.append(llm.FunctionCallOutput(
                name="check_status", call_id="old", output="switch-42", is_error=False,
            ))
            await bot.update_chat_ctx(current)
        finally:
            model.summary_release.set()
            await asyncio.wait_for(task, timeout=5)
        await self.assert_followup_runs(bot, session, model)

    async def test_result_arriving_after_summarization_keeps_pending_call(self):
        ctx = history_with_split_tool_result()
        ctx.items = [item for item in ctx.items if item.type != "function_call_output"]
        bot, session, model = await self.start_bot(ctx)
        await bot.compact()
        self.assertTrue(any(item.type == "function_call" and item.call_id == "old"
                            for item in bot.chat_ctx.items))
        current = bot.chat_ctx.copy()
        current.items.append(llm.FunctionCallOutput(
            name="check_status", call_id="old", output="switch-42", is_error=False,
        ))
        await bot.update_chat_ctx(current)
        await self.assert_followup_runs(bot, session, model)

    async def test_completed_tool_exchange_fully_in_head_is_summarized(self):
        ctx = llm.ChatContext.empty()
        ctx.add_message(role="user", content="Find the switch")
        ctx.items.append(llm.FunctionCall(name="check_status", call_id="old", arguments="{}"))
        ctx.items.append(llm.FunctionCallOutput(
            name="check_status", call_id="old", output="switch-42", is_error=False,
        ))
        for role, text in [("assistant", "Found it"), ("user", "Thanks"), ("assistant", "Sure"),
                           ("user", "Remember it"), ("assistant", "Okay"), ("user", "All right")]:
            ctx.add_message(role=role, content=text)
        bot, _, _ = await self.start_bot(ctx)
        await bot.compact()
        self.assertEqual(bot.summary, "Earlier conversation summary.")
        self.assertFalse(any(item.type in ("function_call", "function_call_output")
                             for item in bot.chat_ctx.items))

    async def test_parallel_tool_results_keep_both_matching_calls(self):
        ctx = history_with_split_tool_result()
        first = next(item for item in ctx.items if item.type == "function_call")
        first.group_id = "parallel"
        ctx.insert(llm.FunctionCall(
            name="check_status", call_id="second", group_id="parallel",
            arguments='{"target":"other"}', created_at=first.created_at,
        ))
        result = next(item for item in ctx.items if item.type == "function_call_output")
        ctx.insert(llm.FunctionCallOutput(
            name="check_status", call_id="second", output="other-switch", is_error=False,
            created_at=result.created_at,
        ))
        bot, _, _ = await self.start_bot(ctx)
        await bot.compact()
        messages, _ = bot.chat_ctx.to_provider_format("openai")
        self.assertEqual({message.get("tool_call_id") for message in messages
                          if message.get("role") == "tool"}, {"old", "second"})
        self.assertEqual(sum(item.type == "function_call" for item in bot.chat_ctx.items), 2)

    async def test_not_enough_messages_leaves_context_unchanged(self):
        ctx = llm.ChatContext.empty()
        ctx.add_message(role="user", content="Find the switch")
        ctx.items.append(llm.FunctionCall(name="check_status", call_id="old", arguments="{}"))
        ctx.items.append(llm.FunctionCallOutput(
            name="check_status", call_id="old", output="switch-42", is_error=False,
        ))
        ctx.add_message(role="assistant", content="Found it")
        bot, _, _ = await self.start_bot(ctx)
        before = [(item.type, item.id) for item in bot.chat_ctx.items]
        await bot.compact()
        self.assertEqual(bot.summary, "")
        self.assertFalse(bot.compacting)
        self.assertEqual([(item.type, item.id) for item in bot.chat_ctx.items], before)

    async def test_summarizer_failure_keeps_conversation_usable(self):
        bot, session, model = await self.start_bot(history_with_split_tool_result())
        before = [(item.type, item.id) for item in bot.chat_ctx.items]
        with patch.object(model, "chat", side_effect=RuntimeError("summary failed")):
            await bot.compact()
        self.assertFalse(bot.compacting)
        self.assertEqual([(item.type, item.id) for item in bot.chat_ctx.items], before)
        self.debug_mock.assert_any_call("error", source="compaction", error="summary failed")
        await self.assert_followup_runs(bot, session, model)

    async def test_restored_call_is_summarized_when_result_ages_out(self):
        bot, _, _ = await self.start_bot(history_with_split_tool_result())
        await bot.compact()
        current = bot.chat_ctx.copy()
        for number in range(3):
            current.add_message(role="user", content=f"New question {number}")
            current.add_message(role="assistant", content=f"New answer {number}")
        await bot.update_chat_ctx(current)
        bot.compacting = True
        await bot.compact()
        self.assertFalse(any(item.type in ("function_call", "function_call_output")
                             for item in bot.chat_ctx.items))

    async def test_repeated_compaction_does_not_duplicate_the_call(self):
        bot, session, model = await self.start_bot(history_with_split_tool_result())
        await bot.compact()
        bot.compacting = True
        await bot.compact()
        self.assertEqual(sum(item.type == "function_call" and item.call_id == "old"
                             for item in bot.chat_ctx.items), 1)
        await self.assert_followup_runs(bot, session, model)


if __name__ == "__main__":
    unittest.main()
