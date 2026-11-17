//! Commands for Voicebot mode: Preferences settings and the orb window's tools.

use crate::state::AppState;
use crate::voicebot;
use serde::Serialize;
use std::process::Command;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Debug, Serialize)]
pub struct VoicebotConfig {
    pub terminal: bool,
    pub context_command: String,
    pub wake_word: String,
    pub voice: String,
    pub voices: Vec<String>,
    pub voiceprint: bool,
    pub prompt: String,
    pub mcp: String,
    pub home: String,
    pub status: voicebot::Status,
}

const MCP_EXAMPLE: &str = "{\n  \"mcpServers\": {}\n}\n";

#[tauri::command]
pub async fn get_voicebot_config(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<VoicebotConfig, String> {
    let settings = state.settings.read().await;
    let home = voicebot::home_dir();
    let prompt = voicebot::ensure_prompt(&voicebot::code_dir(app.path().resource_dir().ok()))
        .and_then(std::fs::read_to_string)
        .unwrap_or_default();
    Ok(VoicebotConfig {
        terminal: settings.voicebot_terminal,
        context_command: settings.voicebot_context_command.clone(),
        wake_word: settings.voicebot_wake_word.clone(),
        voice: settings.voicebot_voice.clone(),
        voices: voicebot::voices(),
        voiceprint: home.join("voiceprint.npy").is_file(),
        prompt,
        mcp: std::fs::read_to_string(home.join("mcp.json")).unwrap_or_else(|_| MCP_EXAMPLE.into()),
        home: home.display().to_string(),
        status: voicebot::status(),
    })
}

/// Applies from the next conversation (the agent reads config.json per conversation).
#[tauri::command]
pub async fn set_voicebot_terminal(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    let settings = {
        let mut settings = state.settings.write().await;
        settings.voicebot_terminal = enabled;
        settings.clone()
    };
    state.save_settings().await.map_err(|e| e.to_string())?;
    voicebot::write_agent_config(&settings).map_err(|e| e.to_string())
}

/// Empty means Voicebot always answers; applies from the next conversation.
#[tauri::command]
pub async fn set_voicebot_wake_word(
    state: State<'_, AppState>,
    wake_word: String,
) -> Result<(), String> {
    let settings = {
        let mut settings = state.settings.write().await;
        settings.voicebot_wake_word = wake_word.trim().to_string();
        settings.clone()
    };
    state.save_settings().await.map_err(|e| e.to_string())?;
    voicebot::write_agent_config(&settings).map_err(|e| e.to_string())
}

/// Voicebot's voice; a fresh conversation starts so it speaks in it right away.
#[tauri::command]
pub async fn set_voicebot_voice(
    app: AppHandle,
    state: State<'_, AppState>,
    voice: String,
) -> Result<(), String> {
    let (settings, changed) = {
        let mut settings = state.settings.write().await;
        let changed = settings.voicebot_voice != voice.trim();
        settings.voicebot_voice = voice.trim().to_string();
        (settings.clone(), changed)
    };
    state.save_settings().await.map_err(|e| e.to_string())?;
    voicebot::write_agent_config(&settings).map_err(|e| e.to_string())?;
    if changed {
        let _ = app.emit("voicebot-config-changed", ());
    }
    Ok(())
}

/// Path of a fresh screenshot for the agent's vision tool.
#[tauri::command]
pub async fn voicebot_screenshot() -> Result<String, String> {
    tokio::task::spawn_blocking(voicebot::screenshot)
        .await
        .map_err(|e| e.to_string())?
        .map(|p| p.display().to_string())
}

#[tauri::command]
pub async fn set_voicebot_context_command(
    state: State<'_, AppState>,
    command: String,
) -> Result<(), String> {
    state.settings.write().await.voicebot_context_command = command.trim().to_string();
    state.save_settings().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_voicebot_prompt(prompt: String) -> Result<(), String> {
    let home = voicebot::home_dir();
    std::fs::create_dir_all(&home).map_err(|e| e.to_string())?;
    std::fs::write(home.join("system_prompt.md"), prompt).map_err(|e| e.to_string())
}

/// MCP servers in Claude Desktop format; rejected unless it parses.
#[tauri::command]
pub async fn save_voicebot_mcp(json: String) -> Result<(), String> {
    let value: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| format!("Not valid JSON: {e}"))?;
    let servers = value
        .get("mcpServers")
        .and_then(|s| s.as_object())
        .ok_or("Expected an object with an \"mcpServers\" object inside")?;
    for (name, spec) in servers {
        let has_target = spec.get("command").and_then(|c| c.as_str()).is_some()
            || spec.get("url").and_then(|u| u.as_str()).is_some();
        if !has_target {
            return Err(format!(
                "Server \"{name}\" needs a \"command\" or a \"url\""
            ));
        }
    }
    let home = voicebot::home_dir();
    std::fs::create_dir_all(&home).map_err(|e| e.to_string())?;
    let pretty = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
    std::fs::write(home.join("mcp.json"), pretty + "\n").map_err(|e| e.to_string())
}

/// Forget the voice print; Voicebot answers anyone again from the next conversation.
#[tauri::command]
pub async fn clear_voicebot_voiceprint() -> Result<(), String> {
    let path = voicebot::home_dir().join("voiceprint.npy");
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn voicebot_status() -> Result<voicebot::Status, String> {
    Ok(voicebot::status())
}

/// A token for a fresh room: each call starts a new conversation.
#[tauri::command]
pub async fn voicebot_join() -> Result<voicebot::Join, String> {
    voicebot::join()
}

#[tauri::command]
pub async fn open_voicebot_folder() -> Result<(), String> {
    let home = voicebot::home_dir();
    std::fs::create_dir_all(&home).map_err(|e| e.to_string())?;
    Command::new("open")
        .arg(&home)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// What the user is looking at: the front app and window now, plus the output of
/// the optional screen-history command from Preferences.
#[tauri::command]
pub async fn voicebot_screen_context(state: State<'_, AppState>) -> Result<String, String> {
    let context_command = state.settings.read().await.voicebot_context_command.clone();
    let front = Command::new("osascript")
        .args([
            "-e",
            r#"tell application "System Events" to set p to first application process whose frontmost is true
set n to name of p
set t to ""
try
  set t to name of front window of p
end try
return n & " | " & t"#,
        ])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let mut out = format!(
        "Front app and window now: {}",
        if front.is_empty() { "unknown" } else { &front }
    );
    if !context_command.is_empty() {
        out.push_str("\nScreen history:\n");
        out.push_str(&run_capped(&context_command, Duration::from_secs(2), 4000));
    }
    Ok(out)
}

/// Run a zsh command with a time limit and an output cap.
fn run_capped(command: &str, limit: Duration, max_chars: usize) -> String {
    let child = Command::new("/bin/zsh")
        .args(["-c", command])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn();
    let Ok(mut child) = child else {
        return "(the screen-history command could not start)".into();
    };
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return "(the screen-history command timed out)".into();
            }
        }
    }
    let output = child
        .wait_with_output()
        .map(|o| o.stdout)
        .unwrap_or_default();
    String::from_utf8_lossy(&output)
        .chars()
        .take(max_chars)
        .collect()
}

#[derive(Debug, Serialize)]
pub struct DebugTail {
    pub offset: u64,
    pub lines: Vec<String>,
}

/// New lines of Voicebot's debug log since `offset` (the last 300 on the first call).
#[tauri::command]
pub async fn voicebot_debug_tail(offset: u64) -> Result<DebugTail, String> {
    use std::io::{Read, Seek, SeekFrom};
    let path = voicebot::home_dir().join("logs/debug.jsonl");
    let Ok(mut file) = std::fs::File::open(&path) else {
        return Ok(DebugTail {
            offset: 0,
            lines: Vec::new(),
        });
    };
    let end = file.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;
    let start = if offset > end { 0 } else { offset }; // the log was cleared
    file.seek(SeekFrom::Start(start))
        .map_err(|e| e.to_string())?;
    let mut text = String::new();
    file.take(end - start)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    if lines.len() > 300 {
        lines.drain(..lines.len() - 300);
    }
    Ok(DebugTail { offset: end, lines })
}

#[tauri::command]
pub async fn clear_voicebot_debug() -> Result<(), String> {
    let path = voicebot::home_dir().join("logs/debug.jsonl");
    if path.exists() {
        std::fs::write(&path, "").map_err(|e| e.to_string())?;
    }
    Ok(())
}
