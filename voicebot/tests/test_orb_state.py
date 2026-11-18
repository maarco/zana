"""Contract tests for the retained Voicebot orb participant attribute."""
import asyncio
import os
import sys
import unittest
from types import SimpleNamespace

sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))
from orb_state import OrbState  # noqa: E402

try:
    from livekit.agents import AgentSession, llm
    from livekit.agents.voice.events import (ToolCallEnded, ToolCallStarted, ToolCallUpdated,
                                              ToolExecutionUpdatedEvent)
except ImportError:
    AgentSession = None


class OrbStateTests(unittest.IsolatedAsyncioTestCase):
    @unittest.skipIf(AgentSession is None, "LiveKit is not installed")
    async def test_livekit_session_emits_typed_tool_lifecycle(self):
        async def publish(_attributes):
            pass

        orb = OrbState(publish)
        session = AgentSession()
        session.on("tool_execution_updated", orb.tool_execution_updated)
        orb.agent_changed("thinking")

        def emit(update):
            session.emit("tool_execution_updated", ToolExecutionUpdatedEvent(update=update))

        for call_id in ("a", "b", "c"):
            emit(ToolCallStarted(function_call=llm.FunctionCall(
                call_id=call_id, name="test", arguments="{}")))
        emit(ToolCallUpdated(id="a_update_1", call_id="a", message="progress"))
        for call_id, status in (("a", "done"), ("b", "error"), ("c", "cancelled")):
            self.assertEqual(orb.state, "working")
            emit(ToolCallEnded(id=f"{call_id}_final", call_id=call_id, status=status))
        self.assertEqual(orb.state, "thinking")
        await orb.close()

    async def test_tool_execution_events_track_distinct_calls_and_terminal_states(self):
        async def publish(_attributes):
            pass

        orb = OrbState(publish)
        def event(kind, call_id):
            return SimpleNamespace(update=SimpleNamespace(
                type=kind, call_id=call_id,
                function_call=SimpleNamespace(call_id=call_id)))

        orb.agent_changed("thinking")
        orb.tool_execution_updated(event("tool_call_started", "a"))
        orb.tool_execution_updated(event("tool_call_started", "a"))
        orb.tool_execution_updated(event("tool_call_started", "b"))
        orb.tool_execution_updated(event("tool_call_updated", "a"))
        orb.tool_execution_updated(event("tool_call_ended", "a"))
        orb.tool_execution_updated(event("tool_call_ended", "a"))
        self.assertEqual(orb.state, "working")
        orb.tool_execution_updated(event("tool_call_ended", "b"))
        self.assertEqual(orb.state, "thinking")
        await orb.close()
        orb.tool_execution_updated(event("tool_call_started", "c"))
        self.assertEqual(orb.state, "idle")

    async def test_activity_precedence_and_balanced_tool_counts(self):
        async def publish(_attributes):
            self.fail("state changes must not publish before start")

        orb = OrbState(publish)
        self.assertEqual(orb.state, "idle")
        orb.user_changed("speaking")
        self.assertEqual(orb.state, "idle")
        orb.agent_changed("listening")
        self.assertEqual(orb.state, "listening")
        orb.agent_changed("thinking")
        self.assertEqual(orb.state, "thinking")
        orb.tool_started()
        orb.tool_started()
        self.assertEqual(orb.state, "working")
        orb.agent_changed("speaking")
        self.assertEqual(orb.state, "talking")
        orb.agent_changed("listening")
        orb.tool_finished()
        self.assertEqual(orb.state, "working", "the second tool is still active")
        orb.tool_finished()
        self.assertEqual(orb.state, "listening")
        orb.user_changed("listening")
        orb.tool_finished()  # an extra finish must not make a later start disappear
        self.assertEqual(orb.state, "idle")
        orb.tool_started()
        self.assertEqual(orb.state, "working")
        await orb.close()
        self.assertEqual(orb.state, "idle")

    async def test_publisher_coalesces_changes_serially_and_resets_on_close(self):
        calls = []
        first_started, release_first = asyncio.Event(), asyncio.Event()
        in_flight = 0
        max_in_flight = 0

        async def publish(attributes):
            nonlocal in_flight, max_in_flight
            in_flight += 1
            max_in_flight = max(max_in_flight, in_flight)
            try:
                calls.append(attributes)
                if len(calls) == 1:
                    first_started.set()
                    await release_first.wait()
            finally:
                in_flight -= 1

        orb = OrbState(publish)
        orb.start()
        await asyncio.wait_for(first_started.wait(), 1)
        orb.agent_changed("thinking")
        orb.tool_started()
        orb.agent_changed("speaking")
        release_first.set()

        async def wait_for_latest():
            while not calls or calls[-1] != {"zana.orb.state": "talking"}:
                await asyncio.sleep(0)

        await asyncio.wait_for(wait_for_latest(), 1)
        self.assertEqual(calls, [
            {"zana.orb.state": "idle"}, {"zana.orb.state": "talking"},
        ])
        self.assertEqual(max_in_flight, 1)
        await asyncio.wait_for(orb.close(), 1)
        self.assertEqual(calls[-1], {"zana.orb.state": "idle"})
        orb.tool_started()
        orb.agent_changed("speaking")
        await asyncio.sleep(0)
        self.assertEqual(calls[-1], {"zana.orb.state": "idle"})

    async def test_all_five_wire_values_are_published_and_close_resets(self):
        values = []

        async def publish(attributes):
            self.assertEqual(list(attributes), ["zana.orb.state"])
            values.append(attributes["zana.orb.state"])

        async def expect_count(count):
            async def wait():
                while len(values) < count:
                    await asyncio.sleep(0)
            await asyncio.wait_for(wait(), 1)

        orb = OrbState(publish)
        orb.start()
        await expect_count(1)
        orb.agent_changed("listening")
        orb.user_changed("speaking")
        await expect_count(2)
        orb.agent_changed("thinking")
        await expect_count(3)
        orb.tool_started()
        await expect_count(4)
        orb.agent_changed("speaking")
        await expect_count(5)
        await asyncio.wait_for(orb.close(), 1)
        self.assertEqual(values, ["idle", "listening", "thinking", "working", "talking", "idle"])

    async def test_transient_publish_failure_retries_without_a_new_transition(self):
        attempts = []
        delivered = asyncio.Event()

        async def publish(attributes):
            attempts.append(attributes["zana.orb.state"])
            if len(attempts) == 1:
                raise ConnectionError("temporary failure")
            delivered.set()

        orb = OrbState(publish, retry_delay=0.001, publish_timeout=0.1)
        with self.assertLogs("voicebot.orb_state", level="WARNING"):
            orb.start()
            await asyncio.wait_for(delivered.wait(), 1)
        self.assertEqual(attempts[:2], ["idle", "idle"])
        await asyncio.wait_for(orb.close(), 1)

    async def test_close_times_out_a_stuck_write_then_attempts_final_idle(self):
        calls = []
        blocked, cancelled = asyncio.Event(), asyncio.Event()

        async def publish(attributes):
            value = attributes["zana.orb.state"]
            calls.append(value)
            if value == "thinking":
                blocked.set()
                try:
                    await asyncio.Event().wait()
                finally:
                    cancelled.set()

        orb = OrbState(publish, retry_delay=0.001, publish_timeout=0.01)
        orb.start()
        while not calls:
            await asyncio.sleep(0)
        orb.agent_changed("thinking")
        await asyncio.wait_for(blocked.wait(), 1)
        with self.assertLogs("voicebot.orb_state", level="WARNING"):
            await asyncio.wait_for(orb.close(), 0.5)
        self.assertTrue(cancelled.is_set())
        self.assertEqual(calls, ["idle", "thinking", "idle"])
