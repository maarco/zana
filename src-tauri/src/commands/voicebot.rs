//! Commands for Voicebot mode: Preferences settings and the orb window's tools.

use crate::state::{AppState, Settings};
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
    pub fillers: bool,
    pub filler_phrases: String,
    pub filler_days: u32,
    pub mic_check_minutes: u32,
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
        fillers: settings.voicebot_fillers,
        filler_phrases: settings.voicebot_filler_phrases.join("\n"),
        filler_days: settings.voicebot_filler_days,
        mic_check_minutes: settings.voicebot_mic_check_minutes,
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

/// Thinking sounds: on or off, their lines (one per line; empty means the built-in ones) and
/// how many days before they are made again. Applies from the next conversation.
#[tauri::command]
pub async fn set_voicebot_fillers(
    state: State<'_, AppState>,
    on: bool,
    phrases: String,
    days: u32,
) -> Result<(), String> {
    let settings = {
        let mut settings = state.settings.write().await;
        settings.voicebot_fillers = on;
        settings.voicebot_filler_phrases = phrases
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect();
        settings.voicebot_filler_days = days.max(1);
        settings.clone()
    };
    state.save_settings().await.map_err(|e| e.to_string())?;
    voicebot::write_agent_config(&settings).map_err(|e| e.to_string())
}

/// What changing one setting did, for whoever asked to say.
#[derive(Debug, Serialize, PartialEq)]
pub struct SettingChange {
    pub message: String,
    /// The change takes a fresh conversation (the chat model is chosen when one starts).
    pub restart: bool,
}

fn on_off(value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "on" | "true" | "yes" | "1" | "enable" | "enabled" => Ok(true),
        "off" | "false" | "no" | "0" | "disable" | "disabled" => Ok(false),
        _ => Err("Use on or off".into()),
    }
}

fn whole_number(
    value: &str,
    range: std::ops::RangeInclusive<u32>,
    what: &str,
) -> Result<u32, String> {
    value
        .parse::<u32>()
        .ok()
        .filter(|n| range.contains(n))
        .ok_or_else(|| {
            format!(
                "{what} is a whole number from {} to {}",
                range.start(),
                range.end()
            )
        })
}

/// Settings Voicebot may change about itself, shared with Preferences. Never the API key or URL
/// or the screen history command (it runs a shell command), and the terminal tool only off:
/// text on screen could otherwise talk Voicebot into widening its own access.
pub fn apply_setting(
    settings: &mut Settings,
    name: &str,
    value: &str,
) -> Result<SettingChange, String> {
    let value = value.trim();
    let change = |message: String, restart: bool| Ok(SettingChange { message, restart });
    match name {
        "voice" => {
            let voices = voicebot::voices();
            let wanted = value.to_lowercase();
            let found = voices
                .iter()
                .find(|v| v.to_lowercase() == wanted)
                .or_else(|| {
                    voices
                        .iter()
                        .find(|v| !wanted.is_empty() && v.to_lowercase().contains(&wanted))
                });
            let Some(voice) = found else {
                return Err(format!(
                    "There is no voice called {value}; the voices are {}",
                    voices.join(", ")
                ));
            };
            settings.voicebot_voice = voice.clone();
            change(format!("Voice set to {voice}"), false)
        }
        "model" | "vision_model" => {
            if value.contains(char::is_whitespace) || (name == "model" && value.is_empty()) {
                return Err("A model id has no spaces, like z-ai/glm-5.3-flash".into());
            }
            if name == "model" {
                settings.cloud_rewrite.model = value.to_string();
            } else {
                settings.cloud_rewrite.vision_model = value.to_string();
            }
            let what = if name == "model" {
                "Chat model"
            } else {
                "Vision model"
            };
            change(
                if value.is_empty() {
                    "Screenshots now use the chat model; this applies in a fresh conversation"
                        .into()
                } else {
                    format!("{what} set to {value}; it applies in a fresh conversation")
                },
                true,
            )
        }
        "reasoning" => {
            if !["default", "off", "low", "medium", "high"].contains(&value) {
                return Err("Reasoning is one of default, off, low, medium or high".into());
            }
            settings.cloud_rewrite.reasoning = value.to_string();
            change(
                format!("Reasoning set to {value}; it applies in a fresh conversation"),
                true,
            )
        }
        "thinking_sounds" => {
            let on = on_off(value)?;
            settings.voicebot_fillers = on;
            change(
                format!("Thinking sounds {}", if on { "on" } else { "off" }),
                false,
            )
        }
        "thinking_sound_lines" => {
            let lines: Vec<String> = value
                .split(['|', '\n'])
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.eq_ignore_ascii_case("default"))
                .map(String::from)
                .collect();
            let message = if lines.is_empty() {
                "Thinking sounds are back to the built-in ones".to_string()
            } else {
                format!(
                    "{} thinking sound lines set; they are made in the background",
                    lines.len()
                )
            };
            settings.voicebot_filler_phrases = lines;
            change(message, false)
        }
        "remake_sounds_days" => {
            let days = whole_number(value, 1..=365, "Days")?;
            settings.voicebot_filler_days = days;
            change(
                format!("Thinking sounds are remade every {days} days"),
                false,
            )
        }
        "mic_check_minutes" => {
            let minutes = whole_number(value, 1..=120, "Minutes")?;
            settings.voicebot_mic_check_minutes = minutes;
            change(
                format!("The orb pulses red after {minutes} minutes without a working microphone"),
                false,
            )
        }
        "terminal" => {
            if on_off(value)? {
                return Err(
                    "Only the user can turn the terminal tool on, in Zana's Preferences".into(),
                );
            }
            settings.voicebot_terminal = false;
            change("Terminal tool off".into(), false)
        }
        _ => Err(format!(
            "{name} is not a setting Voicebot can change; it can change voice, model, reasoning, \
             vision_model, thinking_sounds, thinking_sound_lines, remake_sounds_days, \
             mic_check_minutes and terminal (off only)"
        )),
    }
}

/// Change one Voicebot setting (from Voicebot itself, through the orb, or from Preferences).
#[tauri::command]
pub async fn set_voicebot_setting(
    state: State<'_, AppState>,
    name: String,
    value: String,
) -> Result<SettingChange, String> {
    let (settings, change) = {
        let mut settings = state.settings.write().await;
        let change = apply_setting(&mut settings, &name, &value)?;
        (settings.clone(), change)
    };
    state.save_settings().await.map_err(|e| e.to_string())?;
    voicebot::write_agent_config(&settings).map_err(|e| e.to_string())?;
    Ok(change)
}

/// What Voicebot may know about its own settings: everything it can see in Preferences except
/// the API key and URL.
#[tauri::command]
pub async fn voicebot_self_settings(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let s = state.settings.read().await;
    let home = voicebot::home_dir();
    let name = std::fs::read_to_string(home.join("config.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|c| c.get("name").and_then(|n| n.as_str()).map(String::from))
        .unwrap_or_else(|| "Voicebot".into());
    Ok(serde_json::json!({
        "name": name,
        "wake_word": s.voicebot_wake_word,
        "voice": if s.voicebot_voice.is_empty() { "en-Emma_woman (default)" } else { &s.voicebot_voice },
        "voices": voicebot::voices(),
        "model": s.cloud_rewrite.model,
        "reasoning": s.cloud_rewrite.reasoning,
        "vision_model": if s.cloud_rewrite.vision_model.is_empty() { "same as the chat model" } else { &s.cloud_rewrite.vision_model },
        "thinking_sounds": s.voicebot_fillers,
        "thinking_sound_lines": if s.voicebot_filler_phrases.is_empty() { "built-in".to_string() } else { s.voicebot_filler_phrases.join(" | ") },
        "remake_sounds_days": s.voicebot_filler_days,
        "mic_check_minutes": s.voicebot_mic_check_minutes,
        "terminal": s.voicebot_terminal,
        "voice_print": home.join("voiceprint.npy").is_file(),
    }))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voicebot_can_change_its_own_settings_within_bounds() {
        let mut s = Settings::default();
        assert!(apply_setting(&mut s, "voice", "emma").is_ok());
        assert_eq!(s.voicebot_voice, "en-Emma_woman");
        assert!(apply_setting(&mut s, "voice", "nobody").is_err());
        assert!(apply_setting(&mut s, "reasoning", "high").unwrap().restart);
        assert_eq!(s.cloud_rewrite.reasoning, "high");
        assert!(apply_setting(&mut s, "reasoning", "max").is_err());
        assert!(apply_setting(&mut s, "model", "two words").is_err());
        apply_setting(&mut s, "thinking_sound_lines", "Hmm... | Uhh...").unwrap();
        assert_eq!(s.voicebot_filler_phrases, vec!["Hmm...", "Uhh..."]);
        apply_setting(&mut s, "thinking_sound_lines", "default").unwrap();
        assert!(s.voicebot_filler_phrases.is_empty());
        assert!(apply_setting(&mut s, "mic_check_minutes", "0").is_err());
        apply_setting(&mut s, "mic_check_minutes", "5").unwrap();
        assert_eq!(s.voicebot_mic_check_minutes, 5);
    }

    #[test]
    fn voicebot_cannot_widen_its_own_access() {
        let mut s = Settings {
            voicebot_terminal: false,
            ..Settings::default()
        };
        assert!(apply_setting(&mut s, "terminal", "on").is_err());
        assert!(!s.voicebot_terminal);
        s.voicebot_terminal = true;
        assert!(apply_setting(&mut s, "terminal", "off").is_ok());
        assert!(!s.voicebot_terminal);
        for name in ["api_key", "api_url", "context_command"] {
            assert!(
                apply_setting(&mut s, name, "x").is_err(),
                "{name} must not be changeable"
            );
        }
    }
}
