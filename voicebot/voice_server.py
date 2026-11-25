"""Voicebot's local OpenAI-compatible speech server (started by Zana).

POST /v1/audio/transcriptions  -> NVIDIA Parakeet (parakeet-mlx, Apple Silicon)
POST /v1/audio/speech          -> Microsoft VibeVoice-Realtime-0.5B (streams raw 24 kHz s16le PCM)
"""
import asyncio, copy, os, re, tempfile, threading, time, logging
from concurrent.futures import ThreadPoolExecutor

import numpy as np
import torch
from fastapi import FastAPI, File, Form, UploadFile
from fastapi.responses import JSONResponse, StreamingResponse, PlainTextResponse
from pydantic import BaseModel

from port_reporting import report_bound_port

HERE = os.path.dirname(os.path.abspath(__file__))
VOICEBOT_HOME = os.path.expanduser(os.getenv("VOICEBOT_HOME", os.path.join(HERE, ".home")))
# Model ids resolve through the Hugging Face cache; a local folder path also works.
VIBEVOICE_MODEL = os.path.expanduser(os.getenv("VIBEVOICE_MODEL", "microsoft/VibeVoice-Realtime-0.5B"))
VIBEVOICE_VOICE = os.getenv("VIBEVOICE_VOICE", "en-Emma_woman")
PARAKEET_MODEL = os.getenv("PARAKEET_MODEL", "mlx-community/parakeet-tdt-0.6b-v3")
STOCK_VOICES = os.path.join(VOICEBOT_HOME, "voices", "stock")  # downloaded by Zana on first start
CUSTOM_VOICES = os.path.join(VOICEBOT_HOME, "voices", "custom")  # extra .pt voice prompts you add

from vibevoice.modular.modeling_vibevoice_streaming_inference import VibeVoiceStreamingForConditionalGenerationInference
from vibevoice.processor.vibevoice_streaming_processor import VibeVoiceStreamingProcessor
from vibevoice.modular.streamer import AudioStreamer
from generation_health import GenerationHealth

log = logging.getLogger("voice_server")
logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")

# ---------- STT: Parakeet (MLX). All MLX work stays on one dedicated thread. ----------
mlx_pool = ThreadPoolExecutor(max_workers=1)
_stt = {}

# Both models keep GPU scratch buffers sized to each request and never hand them back on their own:
# without releasing them the server's footprint grew about 8 MB per spoken or heard turn (9.6 GB after
# 80 test rounds). MLX's cache is capped and both caches are emptied after every request.
MLX_CACHE_LIMIT = 256 * 1024 * 1024

def _load_parakeet():
    import mlx.core as mx
    from parakeet_mlx import from_pretrained
    mx.set_cache_limit(MLX_CACHE_LIMIT)
    _stt["model"] = from_pretrained(PARAKEET_MODEL)
    log.info("Parakeet loaded: %s", PARAKEET_MODEL)

def _transcribe(path):
    import mlx.core as mx
    try:
        return _stt["model"].transcribe(path).text.strip()
    finally:
        mx.clear_cache()

# ---------- TTS: VibeVoice Realtime 0.5B (PyTorch MPS) ----------
DEVICE = "mps" if torch.backends.mps.is_available() else "cpu"
tts_lock = threading.Lock()
tts_health = GenerationHealth()
# live-tunable from the page's Voice settings tab: cfg = how tightly it follows the voice sample,
# steps = diffusion steps (cleaner, slower), seed = same noise every sentence (-1 = random each time)
TTS_SETTINGS = {"cfg": float(os.getenv("VIBEVOICE_CFG", "1.5")), "steps": int(os.getenv("VIBEVOICE_STEPS", "5")),
                "seed": int(os.getenv("VIBEVOICE_SEED", "-1"))}
_tts = {}

def _load_vibevoice():
    proc = VibeVoiceStreamingProcessor.from_pretrained(VIBEVOICE_MODEL)
    model = VibeVoiceStreamingForConditionalGenerationInference.from_pretrained(
        VIBEVOICE_MODEL, torch_dtype=torch.float32, attn_implementation="sdpa", device_map=None)
    model.to(DEVICE).eval()
    model.set_ddpm_inference_steps(num_steps=int(os.getenv("VIBEVOICE_STEPS", "5")))
    _tts.update(proc=proc, model=model, voices={})
    log.info("VibeVoice loaded on %s from %s", DEVICE, VIBEVOICE_MODEL)

def _list_voices(d):
    return sorted(f[:-3] for f in os.listdir(d) if f.endswith(".pt")) if os.path.isdir(d) else []

def _voice_paths():
    return {n: os.path.join(d, n + ".pt") for d in (STOCK_VOICES, CUSTOM_VOICES) for n in _list_voices(d)}

def _voice(name):
    avail = _voice_paths()
    key = name if name in avail else next((k for k in avail if name.lower() in k.lower()), VIBEVOICE_VOICE)
    if key not in _tts["voices"]:
        _tts["voices"][key] = torch.load(avail[key], map_location=DEVICE, weights_only=False)
    return _tts["voices"][key]

def _tts_generate(text, voice, streamer, stop=None):
    try:
        with tts_lock:
            with tts_health.generation():
                if stop is not None and stop.is_set():
                    return
                prefill = _voice(voice)
                model_steps, seed = TTS_SETTINGS["steps"], TTS_SETTINGS["seed"]
                _tts["model"].set_ddpm_inference_steps(num_steps=model_steps)
                if seed >= 0:
                    torch.manual_seed(seed)  # the diffusion head draws its noise from torch's global generator
                proc, model = _tts["proc"], _tts["model"]
                inputs = proc.process_input_with_cached_prompt(text=text, cached_prompt=prefill, padding=True,
                                                               return_tensors="pt", return_attention_mask=True)
                for k, v in inputs.items():
                    if torch.is_tensor(v):
                        inputs[k] = v.to(DEVICE)
                t0 = time.time()
                model.generate(**inputs, max_new_tokens=None, cfg_scale=TTS_SETTINGS["cfg"],
                               tokenizer=proc.tokenizer, generation_config={"do_sample": False}, verbose=False,
                               audio_streamer=streamer, all_prefilled_outputs=copy.deepcopy(prefill),
                               stop_check_fn=(stop.is_set if stop is not None else None))
                log.info("TTS done in %.2fs: %r", time.time() - t0, text[:80])
    except Exception:
        log.exception("TTS failed")
    finally:
        streamer.end()
        if DEVICE == "mps":
            with tts_lock:  # never while another generation is using the GPU
                torch.mps.empty_cache()

app = FastAPI()

@app.on_event("startup")
def startup():
    mlx_pool.submit(_load_parakeet).result()
    _load_vibevoice()
    # warm up both models
    s = AudioStreamer(batch_size=1)
    threading.Thread(target=_tts_generate, args=("Hello there.", VIBEVOICE_VOICE, s), daemon=True).start()
    while s.audio_queues[0].get() is not None:
        pass
    log.info("Warm-up complete, ready.")

@app.get("/health")
def health():
    if tts_health.stalled():
        return PlainTextResponse("speech generation stalled", status_code=503)
    return PlainTextResponse("ok")

@app.get("/v1/voices")
def voices():
    return {"stock": _list_voices(STOCK_VOICES), "custom": _list_voices(CUSTOM_VOICES)}

class TTSSettings(BaseModel):
    cfg: float | None = None
    steps: int | None = None
    seed: int | None = None

@app.get("/v1/tts/settings")
def get_tts_settings():
    return TTS_SETTINGS

@app.post("/v1/tts/settings")
def set_tts_settings(req: TTSSettings):
    if req.cfg is not None:
        TTS_SETTINGS["cfg"] = min(max(req.cfg, 1.0), 4.0)
    if req.steps is not None:
        TTS_SETTINGS["steps"] = min(max(req.steps, 2), 30)
    if req.seed is not None:
        TTS_SETTINGS["seed"] = max(req.seed, -1)
    log.info("TTS settings %s", TTS_SETTINGS)
    return TTS_SETTINGS

@app.get("/v1/models")
def models():
    return {"object": "list", "data": [{"id": "parakeet", "object": "model"}, {"id": "vibevoice", "object": "model"}]}

@app.post("/v1/audio/transcriptions")
async def transcriptions(file: UploadFile = File(...), model: str = Form("parakeet"),
                         language: str = Form(None), prompt: str = Form(None),
                         response_format: str = Form("json")):
    data = await file.read()
    suffix = os.path.splitext(file.filename or "audio.wav")[1] or ".wav"
    with tempfile.NamedTemporaryFile(suffix=suffix, delete=False) as f:
        f.write(data)
        path = f.name
    try:
        import asyncio
        t0 = time.time()
        text = await asyncio.get_running_loop().run_in_executor(mlx_pool, _transcribe, path)
        log.info("STT %.2fs: %r", time.time() - t0, text)
    finally:
        os.unlink(path)
    if response_format == "text":
        return PlainTextResponse(text)
    return JSONResponse({"text": text})

class SpeechReq(BaseModel):
    input: str
    model: str = "vibevoice"
    voice: str = VIBEVOICE_VOICE
    response_format: str = "pcm"
    speed: float | None = None

@app.post("/v1/audio/speech")
def speech(req: SpeechReq):
    text = " ".join(req.input.split())
    streamer = AudioStreamer(batch_size=1)
    stop = threading.Event()  # set when the client goes away (e.g. user interrupts) -> stop generating
    if text:
        threading.Thread(target=_tts_generate, args=(text, req.voice, streamer, stop), daemon=True).start()
    else:
        streamer.end()

    def gen():
        q = streamer.audio_queues[0]
        try:
            while True:
                chunk = q.get()
                if chunk is None:
                    break
                a = chunk.float().numpy().reshape(-1)
                yield (np.clip(a, -1.0, 1.0) * 32767).astype("<i2").tobytes()
        finally:
            stop.set()

    return StreamingResponse(gen(), media_type="audio/pcm")

if __name__ == "__main__":
    import uvicorn

    class PortReportingServer(uvicorn.Server):
        async def startup(self, sockets=None):
            await super().startup(sockets=sockets)
            path = os.getenv("VOICE_SERVER_PORT_FILE")
            if path:
                report_bound_port(self, path)

    config = uvicorn.Config(app, host="127.0.0.1", port=int(os.getenv("VOICE_SERVER_PORT", "8800")))
    PortReportingServer(config).run()
