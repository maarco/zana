//! Transcription Commands
//!
//! Tauri commands for speech-to-text using Whisper.

use crate::state::{
    AppState, DictionaryReplacement, ProjectMemory, StyleMemory, TranscriptHistoryEntry,
    WritingProfile,
};
use crate::stt::WhisperModel;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tauri::{Emitter, State};
use tokio::time::timeout;

/// Model info for UI
#[derive(Debug, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub size_mb: u64,
    pub downloaded: bool,
}

/// Response for model operations
#[derive(Debug, Serialize, Deserialize)]
pub struct ModelsResponse {
    pub success: bool,
    pub models: Vec<ModelInfo>,
    pub current_model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Response for transcription
#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptionResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rewritten_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rewrite_error: Option<String>,
    pub used_cloud_rewrite: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<SegmentInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub processing_ms: u64,
}

/// Segment info for UI
#[derive(Debug, Serialize, Deserialize)]
pub struct SegmentInfo {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

/// Download progress event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub model: String,
    pub downloaded: u64,
    pub total: u64,
    pub percent: f32,
}

/// List available Whisper models
#[tauri::command]
pub async fn list_models(state: State<'_, AppState>) -> Result<ModelsResponse, String> {
    let engine = state.whisper_engine.lock().await;

    let all_models = [
        WhisperModel::Tiny,
        WhisperModel::Base,
        WhisperModel::Small,
        WhisperModel::Medium,
        WhisperModel::Large,
    ];

    let models: Vec<ModelInfo> = all_models
        .iter()
        .map(|m| ModelInfo {
            id: format!("{:?}", m).to_lowercase(),
            name: m.name().to_string(),
            size_mb: m.size_mb(),
            downloaded: engine.is_model_downloaded(*m),
        })
        .collect();

    // Get current model from settings (default to Small)
    let current_model = state
        .settings
        .read()
        .await
        .whisper_model
        .clone()
        .unwrap_or_else(|| "small".to_string());

    Ok(ModelsResponse {
        success: true,
        models,
        current_model,
        error: None,
    })
}

/// Download a Whisper model
#[tauri::command]
pub async fn download_model(
    state: State<'_, AppState>,
    window: tauri::Window,
    model_id: String,
) -> Result<crate::commands::audio::RecordingResponse, String> {
    let model = model_id
        .parse::<WhisperModel>()
        .map_err(|_| format!("Unknown model: {}", model_id))?;

    let engine = state.whisper_engine.lock().await;

    // Progress callback that emits events to frontend
    let window_clone = window.clone();
    let model_name = model.name().to_string();

    let progress_callback = move |downloaded: u64, total: u64| {
        let percent = if total > 0 {
            (downloaded as f32 / total as f32) * 100.0
        } else {
            0.0
        };

        let _ = window_clone.emit(
            "download-progress",
            DownloadProgress {
                model: model_name.clone(),
                downloaded,
                total,
                percent,
            },
        );
    };

    match engine.download_model(model, Some(progress_callback)).await {
        Ok(_path) => {
            log::info!("Model {} downloaded successfully", model_id);
            Ok(crate::commands::audio::RecordingResponse {
                success: true,
                error: None,
            })
        }
        Err(e) => {
            log::error!("Failed to download model {}: {}", model_id, e);
            Ok(crate::commands::audio::RecordingResponse {
                success: false,
                error: Some(e.to_string()),
            })
        }
    }
}

/// Set the active Whisper model
#[tauri::command]
pub async fn set_model(
    state: State<'_, AppState>,
    model_id: String,
) -> Result<crate::commands::audio::RecordingResponse, String> {
    // Validate model ID
    if model_id.parse::<WhisperModel>().is_err() {
        return Ok(crate::commands::audio::RecordingResponse {
            success: false,
            error: Some(format!("Unknown model: {}", model_id)),
        });
    }

    // Update settings
    {
        let mut settings = state.settings.write().await;
        settings.whisper_model = Some(model_id.clone());
    }

    if let Err(error) = state.save_settings().await {
        return Ok(crate::commands::audio::RecordingResponse {
            success: false,
            error: Some(error.to_string()),
        });
    }

    log::info!("Set active model to: {}", model_id);

    Ok(crate::commands::audio::RecordingResponse {
        success: true,
        error: None,
    })
}

/// Internal transcription payload used by multiple entry paths.
#[derive(Debug)]
struct CloudRewriteConfig {
    model: String,
    api_key: String,
    url: String,
    timeout_ms: u64,
}

/// Transcribe the last captured audio
#[tauri::command]
pub async fn transcribe(state: State<'_, AppState>) -> Result<TranscriptionResponse, String> {
    transcribe_captured_audio(&state).await
}

pub async fn transcribe_captured_audio(state: &AppState) -> Result<TranscriptionResponse, String> {
    let start = Instant::now();

    // Take captured audio (moves it out, freeing the memory after transcription)
    let audio = {
        let mut audio = state.captured_audio.lock().await;
        audio.take()
    };

    let audio = match audio {
        Some(a) => a,
        None => {
            return Ok(TranscriptionResponse {
                success: false,
                text: None,
                raw_text: None,
                rewritten_text: None,
                rewrite_error: None,
                used_cloud_rewrite: false,
                segments: None,
                processing_ms: 0,
                error: Some("No audio captured. Record first.".to_string()),
            });
        }
    };

    let model_id = {
        let settings = state.settings.read().await;
        settings
            .whisper_model
            .clone()
            .unwrap_or_else(|| "small".to_string())
    };

    let model = model_id
        .parse::<WhisperModel>()
        .unwrap_or(WhisperModel::Small);

    // Check if model is downloaded
    {
        let engine = state.whisper_engine.lock().await;
        if !engine.is_model_downloaded(model) {
            return Ok(TranscriptionResponse {
                success: false,
                text: None,
                raw_text: None,
                rewritten_text: None,
                rewrite_error: None,
                used_cloud_rewrite: false,
                segments: None,
                processing_ms: 0,
                error: Some(format!(
                    "Model '{}' not downloaded. Please download it first.",
                    model.name()
                )),
            });
        }
    }

    // Run local transcription
    let engine = state.whisper_engine.lock().await;
    let local_result = match engine.transcribe(&audio.samples, model).await {
        Ok(result) => result,
        Err(e) => {
            return Ok(TranscriptionResponse {
                success: false,
                text: None,
                raw_text: None,
                rewritten_text: None,
                rewrite_error: None,
                used_cloud_rewrite: false,
                segments: None,
                processing_ms: start.elapsed().as_millis() as u64,
                error: Some(e.to_string()),
            })
        }
    };

    let segments: Vec<SegmentInfo> = local_result
        .segments
        .iter()
        .map(|s| SegmentInfo {
            start_ms: s.start_ms,
            end_ms: s.end_ms,
            text: s.text.clone(),
        })
        .collect();

    let dictionary_replacements = {
        let settings = state.settings.read().await;
        settings.dictionary_replacements.clone()
    };
    let corrected_local_text =
        apply_dictionary_replacements(&local_result.text, &dictionary_replacements);

    let mut final_text = corrected_local_text.clone();
    let mut raw_text = Some(corrected_local_text.clone());
    let mut rewritten_text = None;
    let mut rewrite_error = None;
    let mut used_cloud_rewrite = false;

    let should_rewrite = {
        let settings = state.settings.read().await;
        settings.cloud_rewrite_enabled
    };

    if should_rewrite {
        match run_cloud_rewrite(state, &corrected_local_text).await {
            Ok(rewrite_result) => {
                let rewritten = rewrite_result.text;
                rewritten_text = Some(rewritten.clone());
                raw_text = Some(corrected_local_text.clone());
                final_text = rewritten;
                used_cloud_rewrite = true;
                if let Err(error) = apply_learned_memory(state, rewrite_result.memory).await {
                    log::warn!("Failed to save learned rewrite memory: {}", error);
                }
            }
            Err(error) => {
                rewrite_error = Some(error);
            }
        }
    }

    if let Err(error) = record_transcript_history(
        state,
        &corrected_local_text,
        &final_text,
        used_cloud_rewrite,
    )
    .await
    {
        log::warn!("Failed to save transcript history: {}", error);
    }

    Ok(TranscriptionResponse {
        success: true,
        text: Some(final_text),
        raw_text,
        rewritten_text,
        rewrite_error,
        used_cloud_rewrite,
        segments: Some(segments),
        error: None,
        processing_ms: start.elapsed().as_millis() as u64,
    })
}

fn apply_dictionary_replacements(text: &str, replacements: &[DictionaryReplacement]) -> String {
    replacements
        .iter()
        .filter(|replacement| {
            replacement.enabled
                && !replacement.from.trim().is_empty()
                && !replacement.to.trim().is_empty()
        })
        .fold(text.to_string(), |current, replacement| {
            replace_phrase_case_insensitive(&current, &replacement.from, &replacement.to)
        })
}

fn replace_phrase_case_insensitive(text: &str, from: &str, to: &str) -> String {
    let needle = from.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return text.to_string();
    }

    let mut result = String::with_capacity(text.len());
    let mut rest = text;

    loop {
        let haystack = rest.to_ascii_lowercase();
        let Some(index) = haystack.find(&needle) else {
            result.push_str(rest);
            break;
        };

        result.push_str(&rest[..index]);
        result.push_str(to);
        rest = &rest[index + needle.len()..];
    }

    result
}

#[derive(Debug, Clone, Default)]
struct RewriteResult {
    text: String,
    memory: LearnedMemory,
}

#[derive(Debug, Clone, Default)]
struct LearnedMemory {
    dictionary_replacements: Vec<DictionaryReplacement>,
    style_memories: Vec<StyleMemory>,
    project_memories: Vec<ProjectMemory>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct SubmitResultArguments {
    text: String,
    dictionary_replacements: Vec<DictionaryReplacementProposal>,
    style_memories: Vec<StyleMemoryProposal>,
    project_memories: Vec<ProjectMemoryProposal>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct DictionaryReplacementProposal {
    from: String,
    to: String,
    reason: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct StyleMemoryProposal {
    rule: String,
    reason: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ProjectMemoryProposal {
    key: String,
    value: String,
    reason: Option<String>,
}

/// Transcribe and return immediately (for quick preview)
#[tauri::command]
pub async fn transcribe_preview(state: State<'_, AppState>) -> Result<String, String> {
    let response = transcribe(state).await?;

    if response.success {
        Ok(response.text.unwrap_or_default())
    } else {
        Err(response
            .error
            .unwrap_or_else(|| "Unknown error".to_string()))
    }
}

fn build_rewrite_request(
    clipboard: Option<String>,
    profile: &WritingProfile,
    dictionary_replacements: &[DictionaryReplacement],
    transcript_history: &[TranscriptHistoryEntry],
    style_memories: &[StyleMemory],
    project_memories: &[ProjectMemory],
    text: &str,
) -> String {
    let mut prompt = String::new();

    if let Some(previous_clipboard) = clipboard {
        if !previous_clipboard.trim().is_empty() {
            prompt.push_str("Context (last copied text):\n");
            prompt.push_str(&previous_clipboard);
            prompt.push('\n');
            prompt.push('\n');
        }
    }

    prompt.push_str("Rewrite the transcription into final paste-ready text.\n");
    prompt.push_str("- Return only the rewritten text.\n");
    prompt.push_str("- Do not explain, comment, ask questions, or mention missing context.\n");
    prompt.push_str("- If the input is a test phrase, clean it up as a test phrase.\n");
    prompt.push_str("- Keep all original meaning.\n");
    prompt.push_str("- Preserve the speaker's point of view and grammatical person.\n");
    prompt.push_str("- If the speaker says I, me, my, we, or our, keep that perspective.\n");
    prompt.push_str("- Do not turn the speaker into the assistant, user, or a third person.\n");
    prompt.push_str("- Do not add intent, context, claims, or details that were not spoken.\n");
    prompt.push_str("- Preserve names, numbers, and action items.\n");
    prompt.push_str("- Use learned dictionary, style, project, and recent transcript memory.\n");
    prompt.push_str("- Propose new memory only when it is clearly useful for future dictation.\n");
    prompt.push_str("- Do not propose memory for one-off facts or sensitive secrets.\n");
    prompt.push_str(&format!("- Tone: {}\n", profile.tone));
    prompt.push_str(&format!("- Purpose: {}\n", profile.purpose));
    prompt.push_str(&format!("- Format: {}\n", profile.format));
    prompt.push('\n');

    append_memory_context(
        &mut prompt,
        dictionary_replacements,
        transcript_history,
        style_memories,
        project_memories,
    );

    prompt.push_str("Input:\n");
    prompt.push_str(text);

    prompt
}

fn append_memory_context(
    prompt: &mut String,
    dictionary_replacements: &[DictionaryReplacement],
    transcript_history: &[TranscriptHistoryEntry],
    style_memories: &[StyleMemory],
    project_memories: &[ProjectMemory],
) {
    let active_replacements: Vec<_> = dictionary_replacements
        .iter()
        .filter(|entry| {
            entry.enabled && !entry.from.trim().is_empty() && !entry.to.trim().is_empty()
        })
        .rev()
        .take(30)
        .collect();

    if !active_replacements.is_empty() {
        prompt.push_str("Learned dictionary corrections:\n");
        for entry in active_replacements.into_iter().rev() {
            prompt.push_str(&format!("- {} => {}\n", entry.from, entry.to));
        }
        prompt.push('\n');
    }

    let style_entries: Vec<_> = style_memories
        .iter()
        .filter(|entry| !entry.rule.trim().is_empty())
        .rev()
        .take(20)
        .collect();
    if !style_entries.is_empty() {
        prompt.push_str("Learned style memory:\n");
        for entry in style_entries.into_iter().rev() {
            prompt.push_str(&format!("- {}\n", entry.rule));
        }
        prompt.push('\n');
    }

    let project_entries: Vec<_> = project_memories
        .iter()
        .filter(|entry| !entry.key.trim().is_empty() && !entry.value.trim().is_empty())
        .rev()
        .take(20)
        .collect();
    if !project_entries.is_empty() {
        prompt.push_str("Learned project memory:\n");
        for entry in project_entries.into_iter().rev() {
            prompt.push_str(&format!("- {}: {}\n", entry.key, entry.value));
        }
        prompt.push('\n');
    }

    let history_entries: Vec<_> = transcript_history
        .iter()
        .filter(|entry| !entry.raw_text.trim().is_empty())
        .rev()
        .take(8)
        .collect();
    if !history_entries.is_empty() {
        prompt.push_str("Recent dictation history:\n");
        for entry in history_entries.into_iter().rev() {
            prompt.push_str(&format!("- said: {}\n", entry.raw_text));
            if entry.used_rewrite && entry.final_text != entry.raw_text {
                prompt.push_str(&format!("  pasted: {}\n", entry.final_text));
            }
        }
        prompt.push('\n');
    }
}

async fn run_cloud_rewrite(state: &AppState, transcript: &str) -> Result<RewriteResult, String> {
    let clipboard = state.take_recording_clipboard().await;
    let settings = state.settings.read().await;
    let profile = settings.writing_profile.clone();
    let rewrite_settings = settings.cloud_rewrite.clone();
    let dictionary_replacements = settings.dictionary_replacements.clone();
    let transcript_history = settings.transcript_history.clone();
    let style_memories = settings.style_memories.clone();
    let project_memories = settings.project_memories.clone();
    drop(settings);

    let config = cloud_rewrite_config(&rewrite_settings)?;

    let request_body = serde_json::json!({
        "model": config.model,
        "messages": [
            {
                "role": "system",
                "content": "You rewrite dictated speech into the speaker's final paste-ready text and may propose local memory updates for future dictation. Preserve the speaker's point of view, grammatical person, and meaning exactly. Submit exactly one submit_result tool call. Do not become the speaker, do not refer to the speaker as the user, and do not explain, ask questions, mention uncertainty, or describe what you changed."
            },
            {
                "role": "user",
                "content": build_rewrite_request(
                    clipboard.clone(),
                    &profile,
                    &dictionary_replacements,
                    &transcript_history,
                    &style_memories,
                    &project_memories,
                    transcript,
                )
            }
        ],
        "tools": [
            {
                "type": "function",
                "function": {
                    "name": "submit_result",
                    "description": "Submit final rewritten text plus optional local memory updates qVoice should learn silently.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "text": {
                                "type": "string",
                                "description": "The speaker's final paste-ready rewritten text, preserving point of view and meaning. No explanation or commentary."
                            },
                            "dictionary_replacements": {
                                "type": "array",
                                "description": "Optional future automatic corrections for recurring names, tools, jargon, file names, or phrases.",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "from": { "type": "string" },
                                        "to": { "type": "string" },
                                        "reason": { "type": "string" }
                                    },
                                    "required": ["from", "to"],
                                    "additionalProperties": false
                                }
                            },
                            "style_memories": {
                                "type": "array",
                                "description": "Optional durable style preferences learned from the dictation.",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "rule": { "type": "string" },
                                        "reason": { "type": "string" }
                                    },
                                    "required": ["rule"],
                                    "additionalProperties": false
                                }
                            },
                            "project_memories": {
                                "type": "array",
                                "description": "Optional durable project vocabulary or context facts.",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "key": { "type": "string" },
                                        "value": { "type": "string" },
                                        "reason": { "type": "string" }
                                    },
                                    "required": ["key", "value"],
                                    "additionalProperties": false
                                }
                            }
                        },
                        "required": ["text"],
                        "additionalProperties": false
                    }
                }
            }
        ],
        "tool_choice": "required",
        "temperature": 0.3
    });

    let client = reqwest::Client::new();
    let call = async {
        let mut request = client.post(&config.url).json(&request_body);
        if !config.api_key.trim().is_empty() {
            request = request.bearer_auth(&config.api_key);
        }

        let response = request
            .send()
            .await
            .map_err(|e| format!("Cloud rewrite request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            return Err(format!("Cloud rewrite failed with status {}", status));
        }

        #[derive(Deserialize)]
        struct CloudResponse {
            choices: Vec<CloudChoice>,
        }
        #[derive(Deserialize)]
        struct CloudChoice {
            message: CloudMessage,
        }
        #[derive(Deserialize)]
        struct CloudMessage {
            content: Option<String>,
            tool_calls: Option<Vec<ToolCall>>,
        }
        #[derive(Deserialize)]
        struct ToolCall {
            function: ToolCallFunction,
        }
        #[derive(Deserialize)]
        struct ToolCallFunction {
            arguments: String,
        }

        let payload: CloudResponse = response
            .json()
            .await
            .map_err(|e| format!("Cloud rewrite response parse failed: {}", e))?;

        let message = payload
            .choices
            .first()
            .map(|choice| &choice.message)
            .ok_or_else(|| "Cloud rewrite response missing choices".to_string())?;

        let tool_result = message
            .tool_calls
            .as_ref()
            .and_then(|tool_calls| tool_calls.first())
            .and_then(|tool_call| {
                rewrite_result_from_tool_arguments(&tool_call.function.arguments)
            });

        if let Some(result) = tool_result {
            return Ok::<RewriteResult, String>(result);
        }

        let content = message
            .content
            .clone()
            .ok_or_else(|| "Cloud rewrite response missing text".to_string())?;

        Ok::<RewriteResult, String>(RewriteResult {
            text: content.trim().to_string(),
            memory: LearnedMemory::default(),
        })
    };

    let rewritten = match timeout(Duration::from_millis(config.timeout_ms), call).await {
        Ok(result) => result?,
        Err(_) => return Err("Cloud rewrite timed out".to_string()),
    };

    if rewritten.text.trim().is_empty() {
        return Err("Cloud rewrite returned empty text".to_string());
    }

    if looks_like_assistant_meta_response(&rewritten.text) {
        return Err(
            "Cloud rewrite returned assistant commentary instead of rewritten text".to_string(),
        );
    }

    Ok(rewritten)
}

async fn apply_learned_memory(state: &AppState, memory: LearnedMemory) -> anyhow::Result<()> {
    if memory.dictionary_replacements.is_empty()
        && memory.style_memories.is_empty()
        && memory.project_memories.is_empty()
    {
        return Ok(());
    }

    let mut settings = state.settings.write().await;

    for replacement in memory.dictionary_replacements {
        upsert_dictionary_replacement(&mut settings.dictionary_replacements, replacement);
    }
    cap_vec(&mut settings.dictionary_replacements, 200);

    for style_memory in memory.style_memories {
        upsert_style_memory(&mut settings.style_memories, style_memory);
    }
    cap_vec(&mut settings.style_memories, 100);

    for project_memory in memory.project_memories {
        upsert_project_memory(&mut settings.project_memories, project_memory);
    }
    cap_vec(&mut settings.project_memories, 100);

    settings.save()
}

async fn record_transcript_history(
    state: &AppState,
    raw_text: &str,
    final_text: &str,
    used_rewrite: bool,
) -> anyhow::Result<()> {
    if raw_text.trim().is_empty() {
        return Ok(());
    }

    let mut settings = state.settings.write().await;
    settings.transcript_history.push(TranscriptHistoryEntry {
        raw_text: raw_text.trim().to_string(),
        final_text: final_text.trim().to_string(),
        used_rewrite,
        timestamp: chrono::Utc::now().to_rfc3339(),
    });
    cap_vec(&mut settings.transcript_history, 50);
    settings.save()
}

fn upsert_dictionary_replacement(
    entries: &mut Vec<DictionaryReplacement>,
    replacement: DictionaryReplacement,
) {
    if let Some(existing) = entries.iter_mut().find(|entry| {
        entry
            .from
            .trim()
            .eq_ignore_ascii_case(replacement.from.trim())
    }) {
        if !replacement.to.trim().is_empty() {
            existing.to = replacement.to;
        }
        existing.enabled = true;
        existing.reason = replacement.reason.or_else(|| existing.reason.clone());
        return;
    }

    entries.push(replacement);
}

fn upsert_style_memory(entries: &mut Vec<StyleMemory>, memory: StyleMemory) {
    if entries
        .iter()
        .any(|entry| entry.rule.trim().eq_ignore_ascii_case(memory.rule.trim()))
    {
        return;
    }
    entries.push(memory);
}

fn upsert_project_memory(entries: &mut Vec<ProjectMemory>, memory: ProjectMemory) {
    if let Some(existing) = entries
        .iter_mut()
        .find(|entry| entry.key.trim().eq_ignore_ascii_case(memory.key.trim()))
    {
        existing.value = memory.value;
        existing.reason = memory.reason.or_else(|| existing.reason.clone());
        return;
    }
    entries.push(memory);
}

fn cap_vec<T>(entries: &mut Vec<T>, max_len: usize) {
    if entries.len() > max_len {
        let remove_count = entries.len() - max_len;
        entries.drain(0..remove_count);
    }
}

fn cloud_rewrite_config(
    settings: &crate::state::CloudRewriteSettings,
) -> Result<CloudRewriteConfig, String> {
    let api_key = if settings.api_key.trim().is_empty() {
        std::env::var("ZANA_REWRITE_API_KEY")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_default()
    } else {
        settings.api_key.clone()
    };

    let url = if settings.api_url.trim().is_empty() {
        std::env::var("ZANA_REWRITE_API_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1/chat/completions".to_string())
    } else {
        settings.api_url.clone()
    };
    let url = normalize_rewrite_url(&url);

    if !is_allowed_rewrite_url(&url) {
        return Err(
            "Cloud rewrite URL must use https unless it is localhost or 127.0.0.1".to_string(),
        );
    }

    let model = if settings.model.trim().is_empty() {
        std::env::var("ZANA_REWRITE_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string())
    } else {
        settings.model.clone()
    };

    let timeout_ms = if settings.timeout_ms == 0 {
        std::env::var("ZANA_REWRITE_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(15_000)
    } else {
        settings.timeout_ms
    };

    Ok(CloudRewriteConfig {
        model,
        api_key,
        url,
        timeout_ms,
    })
}

fn is_local_rewrite_url(url: &str) -> bool {
    url.starts_with("http://localhost:")
        || url.starts_with("http://127.0.0.1:")
        || url.starts_with("http://[::1]:")
}

fn is_allowed_rewrite_url(url: &str) -> bool {
    url.starts_with("https://") || is_local_rewrite_url(url)
}

fn rewrite_result_from_tool_arguments(arguments: &str) -> Option<RewriteResult> {
    let parsed = serde_json::from_str::<SubmitResultArguments>(arguments).ok()?;
    let text = parsed.text.trim();
    if text.is_empty() {
        return None;
    }

    let dictionary_replacements = parsed
        .dictionary_replacements
        .into_iter()
        .filter_map(|proposal| {
            let from = proposal.from.trim();
            let to = proposal.to.trim();
            if from.is_empty() || to.is_empty() || from.eq_ignore_ascii_case(to) {
                return None;
            }
            Some(DictionaryReplacement::enabled(
                from,
                to,
                proposal.reason.filter(|reason| !reason.trim().is_empty()),
            ))
        })
        .collect();

    let style_memories = parsed
        .style_memories
        .into_iter()
        .filter_map(|proposal| {
            let rule = proposal.rule.trim();
            if rule.is_empty() {
                return None;
            }
            Some(StyleMemory {
                rule: rule.to_string(),
                reason: proposal.reason.filter(|reason| !reason.trim().is_empty()),
            })
        })
        .collect();

    let project_memories = parsed
        .project_memories
        .into_iter()
        .filter_map(|proposal| {
            let key = proposal.key.trim();
            let value = proposal.value.trim();
            if key.is_empty() || value.is_empty() {
                return None;
            }
            Some(ProjectMemory {
                key: key.to_string(),
                value: value.to_string(),
                reason: proposal.reason.filter(|reason| !reason.trim().is_empty()),
            })
        })
        .collect();

    Some(RewriteResult {
        text: text.to_string(),
        memory: LearnedMemory {
            dictionary_replacements,
            style_memories,
            project_memories,
        },
    })
}

fn looks_like_assistant_meta_response(text: &str) -> bool {
    let normalized = text.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return true;
    }

    let refusal_or_meta_markers = [
        "i am unable to",
        "i'm unable to",
        "i cannot access",
        "i can't access",
        "i cannot rewrite",
        "i can't rewrite",
        "please provide",
        "as an ai",
        "i don't have access",
        "i do not have access",
        "your transcription",
        "user input",
        "private documents",
    ];

    refusal_or_meta_markers
        .iter()
        .filter(|marker| normalized.contains(*marker))
        .count()
        >= 2
}

fn normalize_rewrite_url(url: &str) -> String {
    let trimmed = url.trim();
    if !is_local_rewrite_url(trimmed) {
        return trimmed.to_string();
    }

    let Ok(mut parsed) = reqwest::Url::parse(trimmed) else {
        return trimmed.to_string();
    };

    if parsed.path().is_empty() || parsed.path() == "/" {
        parsed.set_path("/v1/chat/completions");
    }

    parsed.to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        apply_dictionary_replacements, build_rewrite_request, cloud_rewrite_config,
        looks_like_assistant_meta_response, rewrite_result_from_tool_arguments,
    };
    use crate::state::{CloudRewriteSettings, DictionaryReplacement, WritingProfile};

    #[test]
    fn build_rewrite_request_includes_clipboard_and_profile() {
        let profile = WritingProfile {
            purpose: "send a follow-up email".to_string(),
            tone: "warm".to_string(),
            format: "two sentences".to_string(),
        };

        let prompt = build_rewrite_request(
            Some("team status: launch next week".to_string()),
            &profile,
            &[],
            &[],
            &[],
            &[],
            "hey we shipped feature x yesterday",
        );

        assert!(prompt.contains("Context (last copied text):"));
        assert!(prompt.contains("team status: launch next week"));
        assert!(prompt.contains("Return only the rewritten text."));
        assert!(prompt.contains("Do not explain, comment, ask questions"));
        assert!(prompt.contains("Preserve the speaker's point of view"));
        assert!(prompt.contains("If the speaker says I, me, my, we, or our"));
        assert!(prompt.contains("Do not turn the speaker into the assistant"));
        assert!(prompt.contains("Tone: warm"));
        assert!(prompt.contains("Purpose: send a follow-up email"));
        assert!(prompt.contains("Format: two sentences"));
        assert!(prompt.contains("Input:"));
        assert!(prompt.contains("hey we shipped feature x yesterday"));
    }

    #[test]
    fn cloud_rewrite_config_uses_saved_provider_settings() {
        let settings = CloudRewriteSettings {
            api_key: "sk-test".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_url: "https://example.com/v1/chat/completions".to_string(),
            timeout_ms: 12_000,
        };

        let config = cloud_rewrite_config(&settings).expect("saved provider config should load");

        assert_eq!(config.api_key, "sk-test");
        assert_eq!(config.model, "gpt-4o-mini");
        assert_eq!(config.url, "https://example.com/v1/chat/completions");
        assert_eq!(config.timeout_ms, 12_000);
    }

    #[test]
    fn cloud_rewrite_config_rejects_non_https_saved_url() {
        let settings = CloudRewriteSettings {
            api_key: "sk-test".to_string(),
            api_url: "http://example.com/v1/chat/completions".to_string(),
            ..CloudRewriteSettings::default()
        };

        let error = cloud_rewrite_config(&settings).unwrap_err();

        assert!(error.contains("https"));
    }

    #[test]
    fn cloud_rewrite_config_allows_local_http_without_api_key() {
        let settings = CloudRewriteSettings {
            api_key: String::new(),
            model: "qwen3.5-0.8b".to_string(),
            api_url: "http://localhost:1234/v1/chat/completions".to_string(),
            timeout_ms: 15_000,
        };

        let config = cloud_rewrite_config(&settings).expect("local rewrite config should load");

        assert_eq!(config.api_key, "");
        assert_eq!(config.model, "qwen3.5-0.8b");
        assert_eq!(config.url, "http://localhost:1234/v1/chat/completions");
    }

    #[test]
    fn cloud_rewrite_config_expands_local_server_root() {
        let settings = CloudRewriteSettings {
            api_key: String::new(),
            model: "qwen3.5-0.8b".to_string(),
            api_url: "http://localhost:1234/".to_string(),
            timeout_ms: 15_000,
        };

        let config = cloud_rewrite_config(&settings).expect("local rewrite config should load");

        assert_eq!(config.url, "http://localhost:1234/v1/chat/completions");
    }

    #[test]
    fn parses_rewrite_text_from_tool_arguments() {
        let result = rewrite_result_from_tool_arguments(
            r#"{"text":"Testing one, two, three.","dictionary_replacements":[{"from":"cloud code","to":"claude code","reason":"developer tool"}]}"#,
        )
            .expect("tool arguments should include text");

        assert_eq!(result.text, "Testing one, two, three.");
        assert_eq!(result.memory.dictionary_replacements.len(), 1);
        assert_eq!(result.memory.dictionary_replacements[0].from, "cloud code");
        assert_eq!(result.memory.dictionary_replacements[0].to, "claude code");
    }

    #[test]
    fn detects_assistant_meta_response_as_failed_rewrite() {
        let response = "I am unable to rewrite or edit your transcription, as I cannot access private documents or modify user input. Please provide a clear, accurate version of what you intended to say.";

        assert!(looks_like_assistant_meta_response(response));
        assert!(!looks_like_assistant_meta_response(
            "The transcription that goes through the LLM does not get transcribed correctly."
        ));
    }

    #[test]
    fn dictionary_replacements_fix_developer_terms_before_rewrite() {
        let input = "Hello my name is Marco and this is a test. I am a developer and I use cloud code all the time. I always make sure I keep my cloud.markdown file updated.";
        let replacements = vec![
            DictionaryReplacement::enabled("cloud code", "claude code", None),
            DictionaryReplacement::enabled("cloud.markdown", "claude.md", None),
        ];

        let corrected = apply_dictionary_replacements(input, &replacements);

        assert_eq!(
            corrected,
            "Hello my name is Marco and this is a test. I am a developer and I use claude code all the time. I always make sure I keep my claude.md file updated."
        );
    }
}
