"""Live end-to-end check for Zana's Voicebot. Not part of CI: it needs a running
Voicebot (Voicebot Mode on in Zana) and calls the real chat model.

Run with Voicebot's own Python:
  "<Voicebot folder>/venv/bin/python" voicebot/tests/e2e_probe.py zana   # Zana's RPC handlers
  "<Voicebot folder>/venv/bin/python" voicebot/tests/e2e_probe.py agent  # the agent's behavior

It reads LiveKit's port and keys from the running livekit-server's environment
(same user only), then:
  zana:  joins Zana's room and calls its screen and screenshot handlers (never
         paste: that would type into whatever app you are using);
  agent: plays Zana in a fresh room (attribute client=zana) and drives the real
         agent with typed turns through the wake word, unread transcript, tools,
         MCP, vision and mute, printing replies, tool events and RPC calls;
  voiceprint: speaks with VibeVoice voices: enrolls one voice, then checks that
         voice is answered and a different one is ignored (only without a
         voice print set up; it removes the one it creates).
"""
import asyncio
import json
import os
import re
import subprocess
import sys
import time
from datetime import timedelta

from livekit import api, rtc

MODE = sys.argv[1] if len(sys.argv) > 1 else "agent"
HOME = os.path.join(os.path.expanduser("~/Library/Application Support") if sys.platform == "darwin"
                    else os.path.expanduser("~/.config"), "Zana", "voicebot")


def livekit_conn():
    pid = subprocess.run(["pgrep", "-f", "livekit-server --node-ip"], capture_output=True, text=True).stdout.split()[0]
    env = subprocess.run(["ps", "eww", "-o", "command=", "-p", pid], capture_output=True, text=True).stdout
    key, secret = re.search(r"LIVEKIT_KEYS=(\S+): (\S+)", env).groups()
    port = re.search(r"LIVEKIT_CONFIG=port: (\d+)", env).group(1)
    return f"ws://127.0.0.1:{port}", f"http://127.0.0.1:{port}", key, secret


URL, HTTP, KEY, SECRET = livekit_conn()


def token(room, identity, attrs=None):
    t = api.AccessToken(KEY, SECRET).with_identity(identity).with_ttl(timedelta(hours=1)).with_grants(
        api.VideoGrants(room_join=True, room=room, can_publish_data=True))
    if attrs:
        t = t.with_attributes(attrs)
    return t.to_jwt()


async def mode_zana():
    lk = api.LiveKitAPI(HTTP, KEY, SECRET)
    target = None
    for r in (await lk.room.list_rooms(api.ListRoomsRequest())).rooms:
        ps = (await lk.room.list_participants(api.ListParticipantsRequest(room=r.name))).participants
        if any(p.identity == "zana-user" for p in ps):
            target = r.name
            print("zana room:", r.name, [(p.identity, dict(p.attributes).get("client"), dict(p.attributes).get("lk.agent.state")) for p in ps])
    await lk.aclose()
    room = rtc.Room()
    await room.connect(URL, token(target, "probe-a"))
    call = lambda m, p="": room.local_participant.perform_rpc(destination_identity="zana-user", method=m, payload=p, response_timeout=15)
    shot = await call("zana.screenshot")
    print("screenshot:", shot, os.path.getsize(shot) if os.path.isfile(shot) else "MISSING")
    print("screen:", (await call("zana.screen"))[:200])
    await room.disconnect()


async def mode_agent():
    room_name = "probe-" + str(int(time.time()))
    room = rtc.Room()
    replies, events, rpcs = [], [], []

    async def on_stream(reader, participant_identity):
        text = await reader.read_all()
        if os.getenv("PROBE_DEBUG"):
            print("   [text]", participant_identity, reader.info.attributes, repr(text[:120]))
        if reader.info.attributes.get("lk.transcription_final") in (None, "true") and text.strip():
            replies.append((time.time(), text.strip()))

    tasks = set()  # keep stream readers alive until they finish

    def on_text_stream(reader, participant_identity):
        task = asyncio.ensure_future(on_stream(reader, participant_identity))
        tasks.add(task)
        task.add_done_callback(tasks.discard)

    def on_data(pkt):
        if os.getenv("PROBE_DEBUG"):
            print("   [data]", pkt.topic, pkt.data[:120])
        if pkt.topic == "events":
            events.append(json.loads(pkt.data.decode()))

    room.register_text_stream_handler("lk.transcription", on_text_stream)
    room.on("data_received", on_data)

    def handler(name, answer):
        async def h(data: rtc.RpcInvocationData):
            rpcs.append((name, data.payload))
            return answer
        return h

    shot = os.path.join(HOME, "screen.jpg")
    await room.connect(URL, token(room_name, "probe-user", {"client": "zana"}))
    for method, answer in [("zana.screen", "Front app and window now: TextEdit | Untitled"),
                           ("zana.paste", "pasted at the cursor"), ("zana.screenshot", shot),
                           ("zana.mute", "microphone off")]:
        room.local_participant.register_rpc_method(method)(handler(method, answer))

    for _ in range(60):  # wait for the agent to join
        if room.remote_participants:
            break
        await asyncio.sleep(0.5)
    print("agent joined:", list(room.remote_participants))
    await asyncio.sleep(2)

    steps = [
        ("asleep chatter", "the deploy failed last night because the disk was full", 10),
        ("wake + unread", "hey voicebot, what was I just talking about?", 40),
        ("type up", "type up a one line thank you note to Dana for the review", 45),
        ("mcp time", "what time is it in Tokyo right now?", 45),
        ("terminal", "run the terminal command: echo voicebot-ok", 45),
        ("mcp list", "which MCP servers do you have?", 40),
        ("vision", "look at my screen and tell me what app is open", 60),
        ("stop listening", "please stop listening for now", 40),
        ("asleep again", "this should be ignored now", 10),
    ]
    steps = steps[: int(os.getenv("PROBE_STEPS", len(steps)))]
    for label, text, wait in steps:
        n_r, n_e, n_rpc = len(replies), len(events), len(rpcs)
        await room.local_participant.send_text(text, topic="lk.chat")
        deadline = time.time() + wait
        while time.time() < deadline:
            await asyncio.sleep(0.5)
            settled = len(replies) > n_r and time.time() - replies[-1][0] > 4
            if settled and label not in ("asleep chatter", "asleep again"):
                break
        new_r = [r for _, r in replies[n_r:]]
        new_e = [(e.get("kind"), e.get("name") or e.get("asleep"), str(e.get("args") or "")[:80], str(e.get("output") or "")[:120]) for e in events[n_e:]]
        print(f"\n== {label}: {text}")
        print("   replies:", new_r)
        print("   events:", new_e)
        print("   rpcs:", rpcs[n_rpc:])
    await room.disconnect()


def voice_port():
    pid = subprocess.run(["pgrep", "-f", "voicebot/venv/bin/python voice_server.py"], capture_output=True, text=True).stdout.split()[0]
    env = subprocess.run(["/bin/ps", "eww", "-o", "command=", "-p", pid], capture_output=True, text=True).stdout
    return re.search(r"VOICE_SERVER_PORT=(\d+)", env).group(1)


def tts(voice, text):
    """24 kHz mono PCM from Voicebot's own speech server."""
    import urllib.request
    body = json.dumps({"model": "tts-1", "voice": voice, "input": text, "response_format": "pcm"}).encode()
    req = urllib.request.Request(f"http://127.0.0.1:{voice_port()}/v1/audio/speech", data=body,
                                 headers={"Content-Type": "application/json"})
    return urllib.request.urlopen(req, timeout=180).read()


async def mode_voiceprint():
    """Enroll one voice with spoken audio, then check that voice is answered and another is ignored.
    Removes the voice print it created (only run when you have not set one up)."""
    vp_file = os.path.join(HOME, "voiceprint.npy")
    if os.path.exists(vp_file):
        sys.exit("A voice print is already set up; this test would replace it. Clear it first.")
    room_name = "probe-vp-" + str(int(time.time()))
    room = rtc.Room()
    await room.connect(URL, token(room_name, "probe-user", {"client": "zana"}))
    for method in ("zana.screen", "zana.paste", "zana.screenshot", "zana.mute"):
        room.local_participant.register_rpc_method(method)(lambda data: asyncio.sleep(0, "ok"))
    source = rtc.AudioSource(24000, 1)
    await room.local_participant.publish_track(rtc.LocalAudioTrack.create_audio_track("mic", source))
    for _ in range(60):
        if room.remote_participants:
            break
        await asyncio.sleep(0.5)
    await asyncio.sleep(2)
    debug_log = os.path.join(HOME, "logs", "debug.jsonl")
    start = os.path.getsize(debug_log) if os.path.exists(debug_log) else 0

    async def speak(voice, text, pause=2.5):
        pcm = tts(voice, text)
        for i in range(0, len(pcm), 480):  # 10 ms frames
            chunk = pcm[i:i + 480].ljust(480, b"\0")
            await source.capture_frame(rtc.AudioFrame(chunk, 24000, 1, 240))
        silence = bytes(480)
        for _ in range(int(pause * 100)):
            await source.capture_frame(rtc.AudioFrame(silence, 24000, 1, 240))

    await room.local_participant.send_text("hey voicebot, set up my voice print", topic="lk.chat")
    await asyncio.sleep(12)
    for line in ["Okay, here is my voice. I build software all day and I like short answers.",
                 "Most mornings I check the release pipeline and the open pull requests first.",
                 "Then I review what the agents did overnight and decide what ships next."]:
        await speak("en-Carter_man", line)
    await asyncio.sleep(8)
    import hashlib
    ours = hashlib.sha256(open(vp_file, "rb").read()).hexdigest() if os.path.exists(vp_file) else None
    print("voice print created by the test:", bool(ours))
    await speak("en-Carter_man", "What is two plus two?", pause=12)
    await speak("en-Emma_woman", "Hey, can you tell me a joke about cats?", pause=12)
    events = []
    with open(debug_log) as f:
        f.seek(start)
        events = [json.loads(l) for l in f if l.strip()]
    for ev in events:
        if ev["kind"] in ("voiceprint", "heard", "said", "tool"):
            print(ev["kind"], {k: v for k, v in ev.items() if k not in ("t", "kind", "output")})
    await room.disconnect()
    # Remove only the print this test made; if someone enrolled for real meanwhile, keep theirs.
    if ours and os.path.exists(vp_file) and hashlib.sha256(open(vp_file, "rb").read()).hexdigest() == ours:
        os.remove(vp_file)
        print("test voice print removed")


async def mode_selfedit():
    """Drive set_name, edit_system_prompt and reload through the real agent, check the debug log,
    then restore the prompt file and the name exactly as they were."""
    prompt_file = os.path.join(HOME, "system_prompt.md")
    config_file = os.path.join(HOME, "config.json")
    saved_prompt = open(prompt_file).read()
    saved_config = open(config_file).read()
    room = rtc.Room()
    await room.connect(URL, token("probe-edit-" + str(int(time.time())), "probe-user", {"client": "zana"}))
    for _ in range(60):
        if room.remote_participants:
            break
        await asyncio.sleep(0.5)
    await asyncio.sleep(2)
    debug_log = os.path.join(HOME, "logs", "debug.jsonl")
    start = os.path.getsize(debug_log)
    try:
        for text in ["hey voicebot, change your name to Jarvis",
                     "add a rule to your system prompt: always end with the word over",
                     "reload your system prompt"]:
            await room.local_participant.send_text(text, topic="lk.chat")
            await asyncio.sleep(14)
        with open(debug_log) as f:
            f.seek(start)
            for ev in (json.loads(l) for l in f if l.strip()):
                if ev["kind"] in ("tool", "settings", "prompt"):
                    print(ev["kind"], {k: (str(v)[:300]) for k, v in ev.items() if k not in ("t", "kind")})
        print("prompt changed on disk:", open(prompt_file).read() != saved_prompt)
        print("name in config:", json.loads(open(config_file).read()).get("name"))
    finally:
        with open(prompt_file, "w") as f:
            f.write(saved_prompt)
        with open(config_file, "w") as f:
            f.write(saved_config)
        print("restored prompt and config")
        await room.disconnect()


async def mode_jobs():
    """A slow terminal command becomes a job: the agent gets its number, process id and output file,
    then is told when it finishes; check_job reads it back."""
    wake = json.load(open(os.path.join(HOME, "config.json"))).get("wake_word") or ""
    room = rtc.Room()
    await room.connect(URL, token("probe-jobs-" + str(int(time.time())), "probe-user", {"client": "zana"}))
    for _ in range(60):
        if room.remote_participants:
            break
        await asyncio.sleep(0.5)
    await asyncio.sleep(2)
    debug_log = os.path.join(HOME, "logs", "debug.jsonl")
    start = os.path.getsize(debug_log)
    await room.local_participant.send_text(f"{wake}, run the terminal command: sleep 8; echo job-done-xyz", topic="lk.chat")
    await asyncio.sleep(22)
    await room.local_participant.send_text("check job one", topic="lk.chat")
    await asyncio.sleep(10)
    with open(debug_log) as f:
        f.seek(start)
        for ev in (json.loads(l) for l in f if l.strip()):
            if ev["kind"] in ("tool", "job", "said"):
                print(ev["kind"], {k: str(v)[:220] for k, v in ev.items() if k not in ("t", "kind")})
    await room.disconnect()


async def mode_memory():
    """Check compaction, conversation search, and warned job cancellation when the conversation closes.
    config.json is restored afterwards."""
    cfg_path = os.path.join(HOME, "config.json")
    original = open(cfg_path).read()
    cfg = json.loads(original)
    wake = cfg.get("wake_word") or ""
    debug_log = os.path.join(HOME, "logs", "debug.jsonl")
    start = os.path.getsize(debug_log)
    try:
        with open(cfg_path, "w") as f:
            json.dump({**cfg, "compact_chars": 1500}, f)
        room = rtc.Room()
        await room.connect(URL, token("probe-memory-" + str(int(time.time())), "probe-user", {"client": "zana"}))
        for _ in range(60):
            if room.remote_participants:
                break
            await asyncio.sleep(0.5)
        await asyncio.sleep(2)

        def events(since):
            with open(debug_log) as f:
                f.seek(since)
                return [json.loads(l) for l in f if l.strip()]

        async def turn(text, wait=45):
            mark = os.path.getsize(debug_log)
            await room.local_participant.send_text(text, topic="lk.chat")
            for _ in range(wait * 2):
                await asyncio.sleep(0.5)
                said = [e for e in events(mark) if e["kind"] == "said" and not e["text"].startswith("On it")]
                if said:
                    await asyncio.sleep(1.5)
                    return said[-1]["text"]
            return "(no answer)"

        print("1", await turn(f"{wake}, run the terminal command: seq 1 150"))
        print("2", await turn("run the terminal command: echo compaction-marker-abc"))
        print("3", await turn("run the terminal command: seq 151 300"))
        print("4", await turn("run the terminal command: seq 301 420"))
        print("5", await turn("say ok"))
        for _ in range(60):
            if any(e["kind"] == "compact" for e in events(start)):
                break
            await asyncio.sleep(0.5)
        print("6", await turn("what was the very first terminal command I asked you to run in this conversation?"))
        print("7", await turn("search your conversation logs for compaction marker and tell me what it was"))
        print("8", await turn("run the terminal command: sleep 40"))
        print("9", await turn("stop that job"))
        mark = os.path.getsize(debug_log)
        await asyncio.sleep(8)
        late = [e["text"] for e in events(mark) if e["kind"] == "said"]
        print("after stop, extra replies:", late or "none")
        job_reply = await turn("run the terminal command: sleep 120")
        assert "conversation ends" in job_reply.lower() and "stopped" in job_reply.lower(), (
            f"long-job reply did not disclose conversation-close cancellation: {job_reply!r}")
        print("long job warning names conversation-close behavior: true")
        await room.disconnect()
        gone = False
        for _ in range(40):
            await asyncio.sleep(1)
            ps = subprocess.run(["/bin/ps", "-ax", "-o", "command"], capture_output=True, text=True).stdout
            if "sleep 120" not in ps:
                gone = True
                break
        print("job stopped when the conversation closed:", gone)
    finally:
        with open(cfg_path, "w") as f:
            f.write(original)
        print("config.json restored:", open(cfg_path).read() == original)
    for e in events(start):
        if e["kind"] == "compact":
            print("COMPACT", {k: (str(v)[:400]) for k, v in e.items() if k not in ("t", "kind")})
        elif e["kind"] == "said" and "context_chars" in e:
            print("said", e["context_chars"], "|", e["text"][:90])
        elif e["kind"] == "tool":
            print("tool", e.get("name"), str(e.get("args"))[:80])
        elif e["kind"] == "error":
            print("ERROR", e)


async def mode_modelerror():
    """A model that fails must not leave Voicebot silent: point this test's conversation at a model
    that does not exist and expect the spoken explanation. config.json is restored afterwards."""
    cfg_path = os.path.join(HOME, "config.json")
    original = open(cfg_path).read()
    cfg = json.loads(original)
    debug_log = os.path.join(HOME, "logs", "debug.jsonl")
    start = os.path.getsize(debug_log)
    try:
        with open(cfg_path, "w") as f:
            json.dump({**cfg, "llm": {**cfg.get("llm", {}), "model": "zana-test/no-such-model"}}, f)
        room = rtc.Room()
        await room.connect(URL, token("probe-modelerror-" + str(int(time.time())), "probe-user", {"client": "zana"}))
        for _ in range(60):
            if room.remote_participants:
                break
            await asyncio.sleep(0.5)
        await asyncio.sleep(2)
    finally:
        with open(cfg_path, "w") as f:
            f.write(original)  # the test conversation already read its model
    await room.local_participant.send_text(f"{cfg.get('wake_word') or ''}, what time is it?", topic="lk.chat")
    await asyncio.sleep(15)
    await room.disconnect()
    with open(debug_log) as f:
        f.seek(start)
        for ev in (json.loads(l) for l in f if l.strip()):
            if ev["kind"] in ("said", "error"):
                print(ev["kind"], str(ev.get("text") or ev.get("error"))[:160])
    print("config.json restored:", open(cfg_path).read() == original)


async def mode_sayline():
    """An answer sentence plus a tool call in one reply: both must be spoken, spaced, with the
    tool call's own description. Prints what was said, what went to speech, and the transcript."""
    wake = json.load(open(os.path.join(HOME, "config.json"))).get("wake_word") or ""
    marks = {p: os.path.getsize(p) for p in (os.path.join(HOME, "logs", "debug.jsonl"),
                                             os.path.join(HOME, "logs", "agent.log"),
                                             os.path.join(HOME, "transcript.md"))}
    room = rtc.Room()
    await room.connect(URL, token("probe-sayline-" + str(int(time.time())), "probe-user", {"client": "zana"}))
    for _ in range(60):
        if room.remote_participants:
            break
        await asyncio.sleep(0.5)
    await asyncio.sleep(2)
    await room.local_participant.send_text(
        f"{wake}, in the same reply write one full sentence telling me you will look up the date, and call "
        "run_terminal with the command date", topic="lk.chat")
    await asyncio.sleep(20)
    await room.disconnect()
    for path, start in marks.items():
        with open(path, errors="replace") as f:
            f.seek(start)
            for line in f:
                if path.endswith("debug.jsonl"):
                    ev = json.loads(line)
                    if ev["kind"] in ("said", "tool"):
                        print("debug", ev["kind"], repr(str(ev.get("text") or ev.get("args")))[:150])
                elif "spoken:" in line:
                    print("agent.log", line.split("spoken:", 1)[1].strip()[:150].rstrip('"}'))
                elif path.endswith("transcript.md"):
                    print("transcript", line.strip()[:150])


async def mode_chain():
    """Three tool calls in a row: the first step gets the cached thinking sound, later steps open with
    a thinking word and are spaced out. Prints the timeline."""
    wake = json.load(open(os.path.join(HOME, "config.json"))).get("wake_word") or ""
    debug_log = os.path.join(HOME, "logs", "debug.jsonl")
    start = os.path.getsize(debug_log)
    room = rtc.Room()
    await room.connect(URL, token("probe-chain-" + str(int(time.time())), "probe-user", {"client": "zana"}))
    for _ in range(60):
        if room.remote_participants:
            break
        await asyncio.sleep(0.5)
    await asyncio.sleep(2)
    await room.local_participant.send_text(
        f"{wake}, run the terminal command echo one, then once you see its output run echo two, then once you "
        "see that run echo three, one call at a time, then tell me what they printed", topic="lk.chat")
    await asyncio.sleep(40)
    await room.disconnect()
    with open(debug_log) as f:
        f.seek(start)
        events = [json.loads(l) for l in f if l.strip()]
    t0 = events[0]["t"] if events else 0
    for ev in events:
        if ev["kind"] in ("filler", "said", "tool"):
            print(f"{ev['t'] - t0:6.1f}s", ev["kind"], str(ev.get("text") or ev.get("args"))[:110])


async def mode_orb():
    """Set a Voicebot setting through the orb (the path Voicebot's change_setting takes), then show a
    state for five seconds: e2e_probe.py orb orb_design fur talking"""
    lk = api.LiveKitAPI(HTTP, KEY, SECRET)
    target = None
    for r in (await lk.room.list_rooms(api.ListRoomsRequest())).rooms:
        ps = (await lk.room.list_participants(api.ListParticipantsRequest(room=r.name))).participants
        if any(p.identity == "zana-user" for p in ps):
            target = r.name
    await lk.aclose()
    room = rtc.Room()
    await room.connect(URL, token(target, "probe-orb"))
    setting, value, state = sys.argv[2], sys.argv[3], (sys.argv[4] if len(sys.argv) > 4 else "")
    print(await room.local_participant.perform_rpc(destination_identity="zana-user", method="zana.set_setting",
                                                   payload=json.dumps({"name": setting, "value": value}), response_timeout=10))
    if state:
        print(await room.local_participant.perform_rpc(destination_identity="zana-user", method="zana.preview_state",
                                                       payload=state, response_timeout=10))
    await room.disconnect()


async def mode_preview():
    """Ask the real orb to show a state for five seconds (argv[2], e.g. listening or speaking)."""
    lk = api.LiveKitAPI(HTTP, KEY, SECRET)
    target = None
    for r in (await lk.room.list_rooms(api.ListRoomsRequest())).rooms:
        ps = (await lk.room.list_participants(api.ListParticipantsRequest(room=r.name))).participants
        if any(p.identity == "zana-user" for p in ps):
            target = r.name
    await lk.aclose()
    room = rtc.Room()
    await room.connect(URL, token(target, "probe-preview"))
    print(await room.local_participant.perform_rpc(destination_identity="zana-user", method="zana.preview_state",
                                                   payload=sys.argv[2], response_timeout=10))
    await room.disconnect()


asyncio.run({"zana": mode_zana, "voiceprint": mode_voiceprint, "selfedit": mode_selfedit,
             "preview": mode_preview, "jobs": mode_jobs, "memory": mode_memory,
             "modelerror": mode_modelerror, "sayline": mode_sayline,
             "chain": mode_chain, "orb": mode_orb}.get(MODE, mode_agent)())
