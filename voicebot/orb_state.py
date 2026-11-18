"""Retained participant state for the Voicebot orb."""
import asyncio
import logging


log = logging.getLogger("voicebot.orb_state")


class OrbState:
    """Resolve talking > working > thinking > listening > idle for one session."""

    def __init__(self, publish, *, retry_delay=1.0, publish_timeout=2.0):
        self._publish = publish
        self._retry_delay = retry_delay
        self._publish_timeout = publish_timeout
        self._agent = "initializing"
        self._user = "listening"
        self._tools = 0
        self._tool_calls = set()
        self._closed = False
        self._changed = asyncio.Event()
        self._task = None

    @property
    def state(self):
        if self._closed:
            return "idle"
        if self._agent == "speaking":
            return "talking"
        if self._tools:
            return "working"
        if self._agent == "thinking":
            return "thinking"
        if self._agent == "listening" and self._user == "speaking":
            return "listening"
        return "idle"

    def _change(self, change):
        if self._closed:
            return
        before = self.state
        change()
        if self.state != before:
            self._changed.set()

    def agent_changed(self, state):
        self._change(lambda: setattr(self, "_agent", state))

    def user_changed(self, state):
        self._change(lambda: setattr(self, "_user", state))

    def tool_started(self):
        self._change(lambda: setattr(self, "_tools", self._tools + 1))

    def tool_finished(self):
        self._change(lambda: setattr(self, "_tools", max(0, self._tools - 1)))

    def tool_execution_updated(self, event):
        """Track actual SDK execution events, deduplicated by function call ID."""
        if self._closed:
            return
        update = getattr(event, "update", None)
        kind = getattr(update, "type", None)
        if kind == "tool_call_started":
            call_id = getattr(getattr(update, "function_call", None), "call_id", None)
            if call_id and call_id not in self._tool_calls:
                self._tool_calls.add(call_id)
                self.tool_started()
        elif kind == "tool_call_ended":
            call_id = getattr(update, "call_id", None)
            if call_id in self._tool_calls:
                self._tool_calls.remove(call_id)
                self.tool_finished()

    def start(self):
        if self._closed or self._task is not None:
            return
        self._changed.set()
        self._task = asyncio.create_task(self._run())

    async def _run(self):
        while True:
            await self._changed.wait()
            self._changed.clear()
            state = self.state
            try:
                await asyncio.wait_for(
                    self._publish({"zana.orb.state": state}), self._publish_timeout,
                )
            except Exception as exc:  # noqa: BLE001 - state display must not stop the agent
                log.warning("could not publish orb state: %s", exc)
                if self._closed:
                    if state == "idle":
                        return
                    self._changed.set()  # try the final idle even if the earlier update failed
                    continue
                try:
                    await asyncio.wait_for(self._changed.wait(), self._retry_delay)
                except asyncio.TimeoutError:
                    pass
                self._changed.set()
                continue
            if self._closed and state == "idle":
                return
            if state != self.state:
                self._changed.set()

    async def close(self):
        self._closed = True
        self._tool_calls.clear()
        if self._task is not None:
            self._changed.set()
            await self._task
