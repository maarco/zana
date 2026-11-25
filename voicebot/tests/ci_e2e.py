"""Run the real Voicebot agent-mode probe entirely on loopback."""
from __future__ import annotations

import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
LIVEKIT_KEY = "ci-key"
LIVEKIT_SECRET = "ci-secret"
EXPECTED_REPLY = "loopback voicebot smoke passed"


class FakeOpenAIHandler(BaseHTTPRequestHandler):
    """Minimal OpenAI-compatible chat, speech, and transcription endpoints."""

    counts: dict[str, int] = {}
    protocol_version = "HTTP/1.1"

    def log_message(self, _format, *_args):
        pass

    def _body(self) -> bytes:
        return self.rfile.read(int(self.headers.get("Content-Length", "0")))

    def _send(self, status: int, content_type: str, body: bytes) -> None:
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:
        if self.path == "/v1/models":
            self._send(200, "application/json", b'{"object":"list","data":[]}')
            return
        self._send(404, "text/plain", b"not found")

    def do_POST(self) -> None:
        body = self._body()
        if self.path == "/v1/chat/completions":
            self.counts["chat"] = self.counts.get("chat", 0) + 1
            request = json.loads(body or b"{}")
            if request.get("stream"):
                self._stream_chat()
            else:
                response = {
                    "id": "chatcmpl-zana-loopback",
                    "object": "chat.completion",
                    "created": 0,
                    "model": "loopback",
                    "choices": [{
                        "index": 0,
                        "message": {"role": "assistant", "content": EXPECTED_REPLY},
                        "finish_reason": "stop",
                    }],
                    "usage": {"prompt_tokens": 1, "completion_tokens": 5, "total_tokens": 6},
                }
                self._send(200, "application/json", json.dumps(response).encode())
            return
        if self.path == "/v1/audio/speech":
            self.counts["speech"] = self.counts.get("speech", 0) + 1
            self._send(200, "audio/pcm", b"\x00\x00" * 2400)
            return
        if self.path == "/v1/audio/transcriptions":
            self.counts["transcription"] = self.counts.get("transcription", 0) + 1
            self._send(200, "application/json", b'{"text":""}')
            return
        self._send(404, "text/plain", b"not found")

    def _stream_chat(self) -> None:
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "close")
        self.end_headers()
        for content in ("loopback ", "voicebot ", "smoke passed"):
            chunk = {
                "id": "chatcmpl-zana-loopback",
                "object": "chat.completion.chunk",
                "created": 0,
                "model": "loopback",
                "choices": [{"index": 0, "delta": {"content": content}, "finish_reason": None}],
            }
            self.wfile.write(f"data: {json.dumps(chunk)}\n\n".encode())
            self.wfile.flush()
        finish = {
            "id": "chatcmpl-zana-loopback",
            "object": "chat.completion.chunk",
            "created": 0,
            "model": "loopback",
            "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
        }
        self.wfile.write(f"data: {json.dumps(finish)}\n\ndata: [DONE]\n\n".encode())
        self.wfile.flush()


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def _port_is_free(port: int) -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        try:
            sock.bind(("127.0.0.1", port))
        except OSError:
            return False
    return True


def _livekit_port() -> int:
    for _ in range(20):
        port = _free_port()
        if all(_port_is_free(port + offset) for offset in (1, 2)):
            return port
    raise RuntimeError("could not find three free loopback ports for LiveKit")


def _wait_for_port(host: str, port: int, timeout: float = 15) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            with socket.create_connection((host, port), timeout=0.5):
                return
        except OSError:
            time.sleep(0.1)
    raise RuntimeError(f"LiveKit did not listen on {host}:{port}")


def _tail(path: Path, lines: int = 80) -> str:
    if not path.exists():
        return "(no log)"
    return "".join(path.read_text(errors="replace").splitlines(keepends=True)[-lines:])


def run() -> None:
    livekit = shutil.which("livekit-server")
    if not livekit:
        raise SystemExit("livekit-server is required for the loopback Voicebot probe")

    counts: dict[str, int] = {}
    FakeOpenAIHandler.counts = counts
    fake = ThreadingHTTPServer(("127.0.0.1", 0), FakeOpenAIHandler)
    fake_port = int(fake.server_address[1])
    fake_thread = threading.Thread(target=fake.serve_forever, name="fake-openai", daemon=True)
    fake_thread.start()

    livekit_host = os.getenv("ZANA_CI_LIVEKIT_HOST", "127.0.0.1")
    livekit_port = _livekit_port()
    processes: list[tuple[str, subprocess.Popen, object, Path]] = []
    temp_dir = tempfile.TemporaryDirectory(prefix="zana-voicebot-ci-")
    try:
        temp_root = Path(temp_dir.name)
        home = temp_root / "voicebot-home"
        home.mkdir()
        (home / "config.json").write_text(json.dumps({
            "fillers": {"on": False},
            "terminal": False,
            "wake_word": "hey voicebot",
        }))
        logs = temp_root / "logs"
        logs.mkdir()
        fake_url = f"http://127.0.0.1:{fake_port}/v1"
        env = os.environ.copy()
        no_proxy = ",".join(filter(None, [env.get("NO_PROXY"), env.get("no_proxy"), "127.0.0.1", "localhost"]))
        env.update({
            "LIVEKIT_URL": f"ws://{livekit_host}:{livekit_port}",
            "LIVEKIT_HTTP_URL": f"http://{livekit_host}:{livekit_port}",
            "LIVEKIT_API_KEY": LIVEKIT_KEY,
            "LIVEKIT_API_SECRET": LIVEKIT_SECRET,
            "VOICEBOT_HOME": str(home),
            "VOICEBOT_TEST_MODE": "1",
            "VOICE_SERVER_URL": fake_url,
            "LLM_BASE_URL": fake_url,
            "LLM_API_KEY": "loopback",
            "LLM_MODEL": "loopback",
            "TTS_BASE_URL": fake_url,
            "TTS_API_KEY": "loopback",
            "TTS_MODEL": "loopback",
            "NO_PROXY": no_proxy,
            "no_proxy": no_proxy,
            "PYTHONUNBUFFERED": "1",
        })

        livekit_process = _start(
            "livekit",
            [livekit, "--dev", "--bind", livekit_host, "--node-ip", livekit_host,
             "--port", str(livekit_port), "--rtc.tcp_port", str(livekit_port + 1),
             "--rtc.port_range_start", str(livekit_port + 2), "--rtc.port_range_end", str(livekit_port + 2),
             "--rtc.udp_port.start", str(livekit_port + 2), "--rtc.udp_port.end", str(livekit_port + 2),
             "--keys", f"{LIVEKIT_KEY}: {LIVEKIT_SECRET}"],
            env, logs, processes,
        )
        _wait_for_port(livekit_host, livekit_port)
        time.sleep(0.5)
        agent_process = _start("agent", [sys.executable, "voicebot/agent.py", "start"], env, logs, processes)
        probe_env = env | {
            "PROBE_STEPS": "2",
            "PROBE_REQUIRE_SUCCESS": "1",
            "PROBE_EXPECTED_REPLY": EXPECTED_REPLY,
            "PROBE_AGENT_WAIT_SECONDS": "90",
            "PROBE_DEBUG": "1",
        }
        probe = subprocess.run(
            [sys.executable, "voicebot/tests/e2e_probe.py", "agent"],
            cwd=ROOT, env=probe_env, check=False, capture_output=True, text=True, timeout=120,
        )
        print(probe.stdout, end="")
        print(probe.stderr, end="", file=sys.stderr)
        if probe.returncode:
            raise RuntimeError(f"agent-mode probe failed with exit {probe.returncode}")
        if counts.get("chat", 0) < 1 or counts.get("speech", 0) < 1:
            raise RuntimeError(f"fake model was not exercised: {counts}")
        if livekit_process.poll() is not None or agent_process.poll() is not None:
            raise RuntimeError("LiveKit or the agent exited before the probe completed")
        print(f"loopback Voicebot e2e passed: {counts}")
    except BaseException:
        for name, _process, _handle, log_path in processes:
            print(f"{name} log before cleanup:\n{_tail(log_path)}", file=sys.stderr)
        raise
    finally:
        fake.shutdown()
        fake.server_close()
        for name, process, handle, log_path in reversed(processes):
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=10)
            handle.close()
            if process.returncode not in (0, -15):
                print(f"{name} log:\n{_tail(log_path)}", file=sys.stderr)
        temp_dir.cleanup()


def _start(name: str, command: list[str], env: dict[str, str], logs: Path,
           processes: list[tuple[str, subprocess.Popen, object, Path]]) -> subprocess.Popen:
    log_path = logs / f"{name}.log"
    handle = log_path.open("w")
    process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=handle, stderr=subprocess.STDOUT, text=True)
    processes.append((name, process, handle, log_path))
    return process


if __name__ == "__main__":
    run()
