//! Voicebot: Zana's optional LiveKit voice agent (macOS).
//!
//! Zana supervises three child processes: livekit-server, the speech server
//! (voice_server.py) and the agent (agent.py). Their code ships in the app
//! bundle (`Resources/voicebot`); everything per-user lives in the Voicebot
//! home folder inside Zana's config directory: the Python venv, `.env`,
//! `system_prompt.md`, `config.json`, `mcp.json`, downloaded voices and logs.
//! LiveKit gets fresh random keys and free ports on every start, and Zana mints
//! the join token itself.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const VIBEVOICE_COMMIT: &str = "07cb79feadd2d3fd7f47530d4c964a12857936a0";
const STOCK_VOICES: [&str; 7] = [
    "en-Carter_man",
    "en-Davis_man",
    "en-Emma_woman",
    "en-Frank_man",
    "en-Grace_woman",
    "en-Mike_man",
    "in-Samuel_man",
];
/// Files the bundle ships (tauri.conf.json bundle.resources maps the same list).
/// tool_guidance.md is app-owned; Python reads it from this code directory and
/// never copies it into the user's editable system_prompt.md.
pub const CODE_FILES: [&str; 14] = [
    "agent.py",
    "voice_server.py",
    "speech.py",
    "listening.py",
    "jobs.py",
    "fillers.py",
    "orb_state.py",
    "voiceprint.py",
    "updater.py",
    "prompt_guidance.py",
    "tool_guidance.md",
    "system_prompt.default.md",
    "requirements.txt",
    "requirements-nodeps.txt",
];

/// The Voicebot home folder: `<config dir>/Zana/voicebot`.
pub fn home_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Zana")
        .join("voicebot")
}

/// Where the agent code lives: `ZANA_VOICEBOT_DIR` for development, else the bundled resources.
pub fn code_dir(resource_dir: Option<PathBuf>) -> PathBuf {
    std::env::var_os("ZANA_VOICEBOT_DIR")
        .map(PathBuf::from)
        .or_else(|| resource_dir.map(|d| d.join("voicebot")))
        .unwrap_or_else(|| PathBuf::from("voicebot"))
}

/// Seed only the user's personal prompt from the shipped default, and only when missing.
/// App-owned tool guidance stays in the bundled code directory and is assembled by Python.
fn ensure_prompt_at(home: &Path, code: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(home)?;
    let prompt = home.join("system_prompt.md");
    if !prompt.exists() {
        std::fs::copy(code.join("system_prompt.default.md"), &prompt)?;
    }
    Ok(prompt)
}

pub fn ensure_prompt(code: &Path) -> std::io::Result<PathBuf> {
    ensure_prompt_at(&home_dir(), code)
}

/// Settings the agent reads at the start of every conversation: its switches and the chat
/// model from Preferences > Model. The file holds the API key, so only the user can read it.
pub fn write_agent_config(settings: &crate::state::Settings) -> std::io::Result<()> {
    let home = home_dir();
    std::fs::create_dir_all(&home)?;
    let path = home.join("config.json");
    // Merge: keys the agent owns (such as its name) stay.
    let mut config: serde_json::Map<String, serde_json::Value> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    config.insert("terminal".into(), settings.voicebot_terminal.into());
    config.insert(
        "wake_word".into(),
        settings.voicebot_wake_word.trim().into(),
    );
    let model = &settings.cloud_rewrite;
    config.insert(
        "llm".into(),
        serde_json::json!({
            "base_url": llm_base_url(&model.api_url),
            "model": model.model.trim(),
            "api_key": model.api_key.trim(),
            "extra_body": llm_extra_body(&model.api_url, &model.reasoning),
            "vision_model": model.vision_model.trim(),
        }),
    );
    config.insert("voice".into(), settings.voicebot_voice.trim().into());
    config.insert("mood_minutes".into(), settings.voicebot_mood_minutes.into());
    config.insert(
        "fillers".into(),
        serde_json::json!({
            "on": settings.voicebot_fillers,
            "phrases": settings.voicebot_filler_phrases,
            "days": settings.voicebot_filler_days,
        }),
    );
    write_private(&path, &serde_json::Value::Object(config).to_string())
}

/// Write a file only the user can read: a temp file created with mode 0600 (never briefly
/// readable by others) is filled, then renamed over the target in one step.
pub fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let tmp = path.with_extension("tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(0o600);
        let mut file = options.open(&tmp)?;
        // a temp file left by a crash keeps its old mode; tighten it before the key goes in
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
    }
    #[cfg(not(unix))]
    options.open(&tmp)?.write_all(text.as_bytes())?;
    std::fs::rename(&tmp, path)
}

/// Extra request fields for the model's reasoning setting. OpenRouter takes a `reasoning`
/// object (an effort, or enabled: false for Off); other OpenAI-compatible servers take
/// `reasoning_effort` and have no Off. "default" sends nothing and leaves it to the model.
pub fn llm_extra_body(api_url: &str, reasoning: &str) -> serde_json::Value {
    let openrouter = api_url.contains("openrouter.ai");
    match (reasoning.trim(), openrouter) {
        ("off", true) => serde_json::json!({ "reasoning": { "enabled": false } }),
        (effort @ ("low" | "medium" | "high"), true) => {
            serde_json::json!({ "reasoning": { "effort": effort, "exclude": true } })
        }
        (effort @ ("low" | "medium" | "high"), false) => {
            serde_json::json!({ "reasoning_effort": effort })
        }
        _ => serde_json::json!({}),
    }
}

/// Voices the speech server can use: the stock VibeVoice voices plus any `.pt` voice in
/// the Voicebot folder's voices/custom.
pub fn voices() -> Vec<String> {
    let mut names: Vec<String> = STOCK_VOICES.iter().map(|v| v.to_string()).collect();
    if let Ok(entries) = std::fs::read_dir(home_dir().join("voices").join("custom")) {
        for path in entries.flatten().map(|e| e.path()) {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(str::to_string);
            if let (Some(stem), true) = (stem, path.extension().is_some_and(|e| e == "pt")) {
                if !names.contains(&stem) {
                    names.push(stem);
                }
            }
        }
    }
    names
}

/// The OpenAI-compatible base URL for a chat-completions endpoint: ".../v1/chat/completions"
/// becomes ".../v1", and a bare server root (a local server) gets "/v1".
pub fn llm_base_url(api_url: &str) -> String {
    let url = api_url.trim().trim_end_matches('/');
    let url = url.strip_suffix("/chat/completions").unwrap_or(url);
    match url.split_once("://") {
        Some((_, rest)) if !rest.contains('/') => format!("{url}/v1"),
        _ => url.to_string(),
    }
}

/// A fresh screenshot for the agent's vision tool, scaled down to keep it cheap.
pub fn screenshot() -> Result<PathBuf, String> {
    let path = home_dir().join("screen.jpg");
    let ok = std::process::Command::new("/usr/sbin/screencapture")
        .args(["-x", "-t", "jpg"])
        .arg(&path)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok || !path.is_file() {
        return Err("Screen capture failed; Zana may need Screen Recording permission".into());
    }
    let _ = std::process::Command::new("/usr/bin/sips")
        .args(["-Z", "1600"])
        .arg(&path)
        .output();
    Ok(path)
}

/// A command-line tool from PATH or the usual install folders (apps opened from
/// Finder do not get the shell's PATH).
pub fn find_tool(name: &str) -> Option<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    dirs.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        home.join(".local/bin"),
        home.join(".cargo/bin"),
    ]);
    dirs.into_iter().map(|d| d.join(name)).find(|p| p.is_file())
}

// ---------------------------------------------------------------------------
// LiveKit join token (HS256 JWT)
// ---------------------------------------------------------------------------

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut block = [0u8; BLOCK];
    if key.len() > BLOCK {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let inner_pad: Vec<u8> = block.iter().map(|b| b ^ 0x36).collect();
    let outer_pad: Vec<u8> = block.iter().map(|b| b ^ 0x5c).collect();
    let inner = Sha256::new()
        .chain_update(&inner_pad)
        .chain_update(message)
        .finalize();
    Sha256::new()
        .chain_update(&outer_pad)
        .chain_update(inner)
        .finalize()
        .into()
}

/// A LiveKit access token that lets `identity` join `room` for `ttl`.
pub fn mint_token(key: &str, secret: &str, identity: &str, room: &str, ttl: Duration) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
    let claims = serde_json::json!({
        "iss": key,
        "sub": identity,
        "name": identity,
        "nbf": now.saturating_sub(10),
        "attributes": { "client": "zana" },
        "exp": now + ttl.as_secs(),
        "video": {
            "roomJoin": true,
            "room": room,
            "canPublish": true,
            "canSubscribe": true,
            "canPublishData": true,
            "canUpdateOwnMetadata": true
        }
    });
    let payload = URL_SAFE_NO_PAD.encode(claims.to_string());
    let signing_input = format!("{header}.{payload}");
    let signature =
        URL_SAFE_NO_PAD.encode(hmac_sha256(secret.as_bytes(), signing_input.as_bytes()));
    format!("{signing_input}.{signature}")
}

// ---------------------------------------------------------------------------
// Supervisor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    /// off, installing, starting, loading, ready, error
    pub stage: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Join {
    pub url: String,
    pub token: String,
    pub room: String,
}

struct Running {
    url: String,
    key: String,
    secret: String,
}

#[derive(Default)]
struct Inner {
    status: Option<Status>,
    running: Option<Running>,
    children: Vec<(String, std::process::Child)>,
}

fn state() -> &'static Mutex<Inner> {
    static STATE: OnceLock<Mutex<Inner>> = OnceLock::new();
    STATE.get_or_init(Mutex::default)
}

fn set_status(stage: &str, detail: impl Into<String>) {
    let detail = detail.into();
    log::info!("[Voicebot] {stage}: {detail}");
    state().lock().unwrap().status = Some(Status {
        stage: stage.into(),
        detail,
    });
}

pub fn status() -> Status {
    state().lock().unwrap().status.clone().unwrap_or(Status {
        stage: "off".into(),
        detail: "Voicebot Mode is off".into(),
    })
}

/// A join token for a fresh room (a new conversation each call).
pub fn join() -> Result<Join, String> {
    let guard = state().lock().unwrap();
    let running = guard
        .running
        .as_ref()
        .filter(|_| guard.status.as_ref().is_some_and(|s| s.stage == "ready"))
        .ok_or_else(|| format!("Voicebot is not ready ({})", status_text(&guard.status)))?;
    let room = format!("zana-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
    Ok(Join {
        url: running.url.clone(),
        token: mint_token(
            &running.key,
            &running.secret,
            "zana-user",
            &room,
            Duration::from_secs(12 * 3600),
        ),
        room,
    })
}

fn status_text(status: &Option<Status>) -> String {
    status
        .as_ref()
        .map(|s| s.detail.clone())
        .unwrap_or_else(|| "off".into())
}

/// Start everything on a background thread; progress shows through `status()`.
pub fn start(code: PathBuf) {
    set_status("starting", "Starting Voicebot");
    std::thread::spawn(move || {
        if let Err(e) = run(&code) {
            log::error!("[Voicebot] {e}");
            set_status("error", e);
            stop();
        }
    });
}

#[cfg(unix)]
fn run(code: &Path) -> Result<(), String> {
    let home = home_dir();
    let logs = home.join("logs");
    std::fs::create_dir_all(&logs).map_err(|e| e.to_string())?;
    sweep_orphans(&home);
    ensure_prompt(code).map_err(|e| format!("Could not set up {}: {e}", home.display()))?;

    let livekit = find_tool("livekit-server")
        .ok_or("livekit-server is not installed (brew install livekit)")?;
    let python = bootstrap(code, &home, &logs)?;
    fetch_voices(&home)?;

    let path_env = child_path();
    let key = format!("zana{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
    let secret = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let lk_port = free_tcp_port()?;
    let voice_port = free_tcp_port()?;
    let rtc_tcp = free_tcp_port()?;
    let rtc_udp = free_udp_port()?;
    let url = format!("ws://127.0.0.1:{lk_port}");

    set_status("starting", "Starting LiveKit");
    let config = format!(
        "port: {lk_port}\nbind_addresses:\n  - 127.0.0.1\nrtc:\n  tcp_port: {rtc_tcp}\n  udp_port: {rtc_udp}\n  use_external_ip: false\nlogging:\n  level: info\n"
    );
    spawn(
        "livekit",
        std::process::Command::new(&livekit)
            .args(["--node-ip", "127.0.0.1"])
            .env("LIVEKIT_CONFIG", config)
            .env("LIVEKIT_KEYS", format!("{key}: {secret}")),
        &logs,
        &home,
    )?;
    wait_until(Duration::from_secs(20), "LiveKit did not start", || {
        std::net::TcpStream::connect(("127.0.0.1", lk_port)).is_ok()
    })?;

    set_status(
        "loading",
        "Loading speech models (first start can take a few minutes)",
    );
    let common = |cmd: &mut std::process::Command| {
        cmd.current_dir(code)
            .env("PATH", &path_env)
            .env("VOICEBOT_HOME", &home)
            .env("TOKENIZERS_PARALLELISM", "false")
            .env("PYTHONUNBUFFERED", "1")
            // never write bytecode next to the code: in a signed .app that breaks the seal
            .env("PYTHONPYCACHEPREFIX", home.join("pycache"));
    };
    let mut voice = std::process::Command::new(&python);
    voice
        .arg("voice_server.py")
        .env("VOICE_SERVER_PORT", voice_port.to_string());
    common(&mut voice);
    spawn("voice", &mut voice, &logs, &home)?;
    let health = format!("http://127.0.0.1:{voice_port}/health");
    wait_until(
        Duration::from_secs(600),
        "The speech server did not start; see logs/voice.log",
        || {
            std::process::Command::new("curl")
                .args(["-sf", "-m", "2", &health])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        },
    )?;

    set_status("loading", "Starting the agent");
    let mut agent = std::process::Command::new(&python);
    agent
        .args(["agent.py", "start"])
        .env("LIVEKIT_URL", &url)
        .env("LIVEKIT_API_KEY", &key)
        .env("LIVEKIT_API_SECRET", &secret)
        .env(
            "VOICE_SERVER_URL",
            format!("http://127.0.0.1:{voice_port}/v1"),
        );
    common(&mut agent);
    spawn("agent", &mut agent, &logs, &home)?;
    let agent_log = logs.join("agent.log");
    wait_until(
        Duration::from_secs(120),
        "The agent did not register; see logs/agent.log",
        || {
            std::fs::read_to_string(&agent_log)
                .map(|t| t.contains("registered worker"))
                .unwrap_or(false)
        },
    )?;

    state().lock().unwrap().running = Some(Running { url, key, secret });
    set_status("ready", "Ready");
    Ok(())
}

#[cfg(not(unix))]
fn run(_code: &Path) -> Result<(), String> {
    Err("Voicebot runs on macOS only".into())
}

/// Build or refresh the venv when the pinned requirements change.
#[cfg(unix)]
fn bootstrap(code: &Path, home: &Path, logs: &Path) -> Result<PathBuf, String> {
    let venv = home.join("venv");
    let python = venv.join("bin/python");
    let marker = venv.join(".zana-requirements");
    let reqs = std::fs::read(code.join("requirements.txt")).map_err(|e| e.to_string())?;
    let nodeps = std::fs::read(code.join("requirements-nodeps.txt")).map_err(|e| e.to_string())?;
    let want = format!(
        "{:x}",
        Sha256::new()
            .chain_update(&reqs)
            .chain_update(&nodeps)
            .finalize()
    );
    if python.is_file() && std::fs::read_to_string(&marker).ok().as_deref() == Some(want.as_str()) {
        return Ok(python);
    }

    let uv = find_tool("uv").ok_or("uv is not installed (brew install uv)")?;
    let log_path = logs.join("install.log");
    let step = |detail: &str, args: &[&str]| -> Result<(), String> {
        set_status("installing", detail);
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| e.to_string())?;
        let ok = std::process::Command::new(&uv)
            .args(args)
            .current_dir(code)
            .env("PATH", child_path())
            .stdout(log.try_clone().map_err(|e| e.to_string())?)
            .stderr(log)
            .status()
            .map_err(|e| e.to_string())?
            .success();
        if ok {
            Ok(())
        } else {
            Err(format!("{detail} failed; see logs/install.log"))
        }
    };
    let venv_s = venv.to_string_lossy().to_string();
    let python_s = python.to_string_lossy().to_string();
    step(
        "Creating the Python environment",
        &["venv", "--allow-existing", "--python", "3.12", &venv_s],
    )?;
    step(
        "Installing Voicebot packages (about 1.5 GB the first time)",
        &[
            "pip",
            "install",
            "--python",
            &python_s,
            "-r",
            "requirements.txt",
        ],
    )?;
    step(
        "Installing VibeVoice",
        &[
            "pip",
            "install",
            "--python",
            &python_s,
            "--no-deps",
            "-r",
            "requirements-nodeps.txt",
        ],
    )?;
    set_status("installing", "Downloading the turn-detection model");
    let ok = std::process::Command::new(&python)
        .args(["agent.py", "download-files"])
        .current_dir(code)
        .env("VOICEBOT_HOME", home)
        .env("PYTHONPYCACHEPREFIX", home.join("pycache"))
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !ok {
        return Err("Downloading the agent's models failed".into());
    }
    std::fs::write(&marker, want).map_err(|e| e.to_string())?;
    Ok(python)
}

/// VibeVoice's stock voices, from the same pinned commit as the package.
#[cfg(unix)]
fn fetch_voices(home: &Path) -> Result<(), String> {
    let dir = home.join("voices/stock");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    for name in STOCK_VOICES {
        let file = dir.join(format!("{name}.pt"));
        if file.is_file() {
            continue;
        }
        set_status("installing", format!("Downloading voice {name}"));
        let url = format!(
            "https://raw.githubusercontent.com/vibevoice-community/VibeVoice/{VIBEVOICE_COMMIT}/demo/voices/streaming_model/{name}.pt"
        );
        let partial = dir.join(format!("{name}.pt.part"));
        let ok = std::process::Command::new("curl")
            .args(["-fsSL", "-m", "120", "-o"])
            .arg(&partial)
            .arg(&url)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            let _ = std::fs::remove_file(&partial);
            return Err(format!("Downloading voice {name} failed"));
        }
        std::fs::rename(&partial, &file).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(unix)]
fn spawn(
    name: &str,
    cmd: &mut std::process::Command,
    logs: &Path,
    home: &Path,
) -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    let log = std::fs::File::create(logs.join(format!("{name}.log"))).map_err(|e| e.to_string())?;
    let child = cmd
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone().map_err(|e| e.to_string())?)
        .stderr(log)
        .process_group(0)
        .spawn()
        .map_err(|e| format!("Could not start {name}: {e}"))?;
    log::info!("[Voicebot] started {name} (pid {})", child.id());
    let mut guard = state().lock().unwrap();
    guard.children.push((name.to_string(), child));
    let pids: String = guard
        .children
        .iter()
        .map(|(n, c)| format!("{} {n}\n", c.id()))
        .collect();
    drop(guard);
    std::fs::write(home.join("pids"), pids).map_err(|e| e.to_string())
}

/// Stop every child process group (agent first, LiveKit last).
pub fn stop() {
    let children: Vec<_> = {
        let mut guard = state().lock().unwrap();
        guard.running = None;
        std::mem::take(&mut guard.children)
    };
    for (name, mut child) in children.into_iter().rev() {
        terminate_group(child.id());
        let _ = child.wait();
        log::info!("[Voicebot] stopped {name}");
    }
    let _ = std::fs::remove_file(home_dir().join("pids"));
}

#[cfg(unix)]
fn terminate_group(pgid: u32) {
    let pgid = pgid as i32;
    // SAFETY: kill(2) on a process group we created; no memory is touched.
    unsafe {
        libc::kill(-pgid, libc::SIGTERM);
    }
    let deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < deadline {
        // SAFETY: signal 0 only checks whether the group still exists.
        if unsafe { libc::kill(-pgid, 0) } != 0 {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    // SAFETY: as above; the group ignored SIGTERM.
    unsafe {
        libc::kill(-pgid, libc::SIGKILL);
    }
}

#[cfg(not(unix))]
fn terminate_group(_pgid: u32) {}

/// Kill process groups left behind by a Zana that crashed or was force-quit.
#[cfg(unix)]
fn sweep_orphans(home: &Path) {
    let Ok(pids) = std::fs::read_to_string(home.join("pids")) else {
        return;
    };
    let home_s = home.to_string_lossy().to_string();
    for line in pids.lines() {
        let Some(pid) = line
            .split_whitespace()
            .next()
            .and_then(|p| p.parse::<u32>().ok())
        else {
            continue;
        };
        let command = std::process::Command::new("ps")
            .args(["-o", "command=", "-p", &pid.to_string()])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        // Only our own children: the venv python in the Voicebot home, or livekit-server.
        if command.contains(&home_s) || command.contains("livekit-server") {
            log::info!(
                "[Voicebot] stopping leftover process {pid}: {}",
                command.trim()
            );
            terminate_group(pid);
        }
    }
    let _ = std::fs::remove_file(home.join("pids"));
}

fn child_path() -> String {
    let home = dirs::home_dir().unwrap_or_default();
    let mut dirs = vec![
        home.join(".local/bin"),
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ];
    if let Some(p) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&p));
    }
    dirs.extend([PathBuf::from("/usr/bin"), PathBuf::from("/bin")]);
    std::env::join_paths(dirs)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn free_tcp_port() -> Result<u16, String> {
    std::net::TcpListener::bind(("127.0.0.1", 0))
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .map_err(|e| e.to_string())
}

fn free_udp_port() -> Result<u16, String> {
    std::net::UdpSocket::bind(("127.0.0.1", 0))
        .and_then(|s| s.local_addr())
        .map(|a| a.port())
        .map_err(|e| e.to_string())
}

fn wait_until(limit: Duration, error: &str, mut ready: impl FnMut() -> bool) -> Result<(), String> {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if ready() {
            return Ok(());
        }
        // Stop early if a child already died.
        if let Some(dead) = dead_child() {
            return Err(format!("{dead} exited; see logs/{dead}.log"));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Err(error.into())
}

fn dead_child() -> Option<String> {
    let mut guard = state().lock().unwrap();
    guard
        .children
        .iter_mut()
        .find_map(|(n, c)| matches!(c.try_wait(), Ok(Some(_))).then(|| n.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn private_files_are_only_readable_by_the_user() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("zana-private-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, "old").unwrap(); // an existing, world-readable file
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        write_private(&path, r#"{"llm":{"api_key":"k"}}"#).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            r#"{"llm":{"api_key":"k"}}"#
        );
        assert!(!path.with_extension("tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn ensure_prompt_seeds_personal_default_once_and_keeps_guidance_bundled() {
        let unique = format!(
            "zana-prompt-seed-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        let home = root.join("home");
        let code = root.join("voicebot");
        std::fs::create_dir_all(&code).unwrap();
        std::fs::write(code.join("system_prompt.default.md"), "personal default").unwrap();
        std::fs::write(code.join("tool_guidance.md"), "app-owned guidance").unwrap();

        let prompt = ensure_prompt_at(&home, &code).unwrap();
        assert_eq!(
            std::fs::read_to_string(&prompt).unwrap(),
            "personal default"
        );
        assert!(!home.join("tool_guidance.md").exists());

        std::fs::write(&prompt, "legacy user prompt, kept verbatim").unwrap();
        ensure_prompt_at(&home, &code).unwrap();
        assert_eq!(
            std::fs::read_to_string(&prompt).unwrap(),
            "legacy user prompt, kept verbatim"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reasoning_setting_becomes_the_provider_field() {
        let or = "https://openrouter.ai/api/v1/chat/completions";
        assert_eq!(
            llm_extra_body(or, "low"),
            serde_json::json!({"reasoning": {"effort": "low", "exclude": true}})
        );
        assert_eq!(
            llm_extra_body(or, "off"),
            serde_json::json!({"reasoning": {"enabled": false}})
        );
        assert_eq!(llm_extra_body(or, "default"), serde_json::json!({}));
        assert_eq!(
            llm_extra_body("https://api.openai.com/v1/chat/completions", "high"),
            serde_json::json!({"reasoning_effort": "high"})
        );
        assert_eq!(
            llm_extra_body("http://localhost:1234/v1", "off"),
            serde_json::json!({})
        );
    }

    #[test]
    fn chat_completions_urls_become_base_urls() {
        assert_eq!(
            llm_base_url("https://openrouter.ai/api/v1/chat/completions"),
            "https://openrouter.ai/api/v1"
        );
        assert_eq!(
            llm_base_url(" https://api.openai.com/v1/chat/completions/ "),
            "https://api.openai.com/v1"
        );
        assert_eq!(
            llm_base_url("http://localhost:1234"),
            "http://localhost:1234/v1"
        );
        assert_eq!(
            llm_base_url("http://localhost:1234/v1"),
            "http://localhost:1234/v1"
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn hmac_matches_rfc4231_case_2() {
        let mac = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(
            hex(&mac),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn hmac_hashes_long_keys_first() {
        // RFC 4231 test case 6: a 131-byte key.
        let key = [0xaa_u8; 131];
        let mac = hmac_sha256(
            &key,
            b"Test Using Larger Than Block-Size Key - Hash Key First",
        );
        assert_eq!(
            hex(&mac),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    #[test]
    fn token_has_room_grant_and_valid_signature() {
        let token = mint_token(
            "k",
            "s3cret",
            "zana-user",
            "zana-room",
            Duration::from_secs(60),
        );
        let parts: Vec<&str> = token.split('.').collect();
        assert_eq!(parts.len(), 3);
        let claims: serde_json::Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();
        assert_eq!(claims["iss"], "k");
        assert_eq!(claims["sub"], "zana-user");
        assert_eq!(claims["video"]["room"], "zana-room");
        assert_eq!(claims["video"]["roomJoin"], true);
        assert_eq!(claims["attributes"]["client"], "zana");
        let expected = URL_SAFE_NO_PAD.encode(hmac_sha256(
            b"s3cret",
            format!("{}.{}", parts[0], parts[1]).as_bytes(),
        ));
        assert_eq!(parts[2], expected);
    }
}
