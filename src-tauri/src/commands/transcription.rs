//! Transcription Commands
//!
//! Tauri commands for speech-to-text using Whisper.

use crate::state::{
    AppState, DictionaryReplacement, ProjectMemory, StyleMemory, TranscriptHistoryEntry,
    WritingProfile,
};
use crate::stt::WhisperModel;
use serde::{Deserialize, Serialize};
use serde_json::json;
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
            state.clear_recording_context().await;
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
            state.clear_recording_context().await;
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
            state.clear_recording_context().await;
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
            });
        }
    };
    drop(engine);

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
    } else {
        state.clear_recording_context().await;
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
    dictionary_replacements: Vec<FlexibleMemoryValue>,
    style_memories: Vec<FlexibleMemoryValue>,
    project_memories: Vec<FlexibleMemoryValue>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum FlexibleMemoryValue {
    String(String),
    Object(serde_json::Value),
}

impl FlexibleMemoryValue {
    fn into_value(self) -> Option<serde_json::Value> {
        match self {
            Self::String(value) => serde_json::from_str(&value).ok(),
            Self::Object(value) => Some(value),
        }
    }
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
    let use_minimal_context = should_use_minimal_rewrite_context(text);
    let context = build_template_context(
        if use_minimal_context { None } else { clipboard },
        None,
        dictionary_replacements,
        if use_minimal_context {
            &[]
        } else {
            transcript_history
        },
        if use_minimal_context {
            &[]
        } else {
            style_memories
        },
        if use_minimal_context {
            &[]
        } else {
            project_memories
        },
        text,
    );
    let mut prompt = render_prompt_template(&profile.tone, &context);

    if prompt.trim().is_empty() {
        prompt.push_str("Here is what Whisper captured:\n");
        prompt.push_str(text);
    }

    if use_minimal_context {
        prompt.push_str("\n\nIgnore clipboard, screenshots, and prior conversation context.");
    }

    let response_contract = render_prompt_template(&profile.format, &context);
    if !response_contract.trim().is_empty() {
        prompt.push_str("\n\nResponse contract:\n");
        prompt.push_str(&response_contract);
    }

    prompt
}

fn build_system_prompt(profile: &WritingProfile, context: &PromptTemplateContext) -> String {
    let rendered = render_prompt_template(&profile.purpose, context);
    let mut system_prompt = if rendered.trim().is_empty() {
        "You rewrite dictated speech into final paste-ready text.".to_string()
    } else {
        rendered
    };
    system_prompt.push_str("\n\nTechnical contract: submit exactly one submit_result tool call. Do not explain, ask questions, mention uncertainty, or describe what you changed.");
    system_prompt
}

struct PromptTemplateContext {
    time: String,
    clipboard: String,
    screen_shot: String,
    captured: String,
    dictionary: String,
    history: String,
    style_memory: String,
    project_memory: String,
}

fn build_template_context(
    clipboard: Option<String>,
    screenshot_data_url: Option<&str>,
    dictionary_replacements: &[DictionaryReplacement],
    transcript_history: &[TranscriptHistoryEntry],
    style_memories: &[StyleMemory],
    project_memories: &[ProjectMemory],
    text: &str,
) -> PromptTemplateContext {
    PromptTemplateContext {
        time: chrono::Local::now().to_rfc3339(),
        clipboard: clipboard
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "none".to_string()),
        screen_shot: screenshot_data_url
            .map(|_| "attached")
            .unwrap_or("not attached")
            .to_string(),
        captured: text.to_string(),
        dictionary: format_dictionary_context(dictionary_replacements),
        history: format_history_context(transcript_history),
        style_memory: format_style_memory_context(style_memories),
        project_memory: format_project_memory_context(project_memories),
    }
}

fn render_prompt_template(template: &str, context: &PromptTemplateContext) -> String {
    template
        .replace("{time}", &context.time)
        .replace("{clipboard}", &context.clipboard)
        .replace("{screen_shot}", &context.screen_shot)
        .replace("{captured}", &context.captured)
        .replace("{dictionary}", &context.dictionary)
        .replace("{history}", &context.history)
        .replace("{style_memory}", &context.style_memory)
        .replace("{project_memory}", &context.project_memory)
}

fn should_use_minimal_rewrite_context(text: &str) -> bool {
    let normalized = text.trim().to_ascii_lowercase();
    let word_count = normalized.split_whitespace().count();

    if word_count <= 8
        && (normalized.contains("test")
            || normalized.contains("testing")
            || (normalized.contains("one")
                && normalized.contains("two")
                && normalized.contains("three")))
    {
        return true;
    }

    normalized.contains("testing")
        && normalized.contains("one")
        && normalized.contains("two")
        && normalized.contains("three")
        && word_count <= 24
}

fn format_dictionary_context(dictionary_replacements: &[DictionaryReplacement]) -> String {
    let entries: Vec<_> = dictionary_replacements
        .iter()
        .filter(|entry| {
            entry.enabled && !entry.from.trim().is_empty() && !entry.to.trim().is_empty()
        })
        .rev()
        .take(30)
        .collect();

    if entries.is_empty() {
        return "none".to_string();
    }

    entries
        .into_iter()
        .rev()
        .map(|entry| format!("- {} => {}", entry.from, entry.to))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_history_context(transcript_history: &[TranscriptHistoryEntry]) -> String {
    let entries: Vec<_> = transcript_history
        .iter()
        .filter(|entry| !entry.raw_text.trim().is_empty())
        .rev()
        .take(8)
        .collect();

    if entries.is_empty() {
        return "none".to_string();
    }

    entries
        .into_iter()
        .rev()
        .map(|entry| {
            if entry.used_rewrite && entry.final_text != entry.raw_text {
                format!("- said: {}\n  pasted: {}", entry.raw_text, entry.final_text)
            } else {
                format!("- said: {}", entry.raw_text)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_style_memory_context(style_memories: &[StyleMemory]) -> String {
    let entries: Vec<_> = style_memories
        .iter()
        .filter(|entry| !entry.rule.trim().is_empty())
        .rev()
        .take(20)
        .collect();

    if entries.is_empty() {
        return "none".to_string();
    }

    entries
        .into_iter()
        .rev()
        .map(|entry| format!("- {}", entry.rule))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_project_memory_context(project_memories: &[ProjectMemory]) -> String {
    let entries: Vec<_> = project_memories
        .iter()
        .filter(|entry| !entry.key.trim().is_empty() && !entry.value.trim().is_empty())
        .rev()
        .take(20)
        .collect();

    if entries.is_empty() {
        return "none".to_string();
    }

    entries
        .into_iter()
        .rev()
        .map(|entry| format!("- {}: {}", entry.key, entry.value))
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_rewrite_message_content(
    prompt: String,
    screenshot_data_url: Option<String>,
) -> serde_json::Value {
    match screenshot_data_url {
        Some(data_url) => json!([
            {
                "type": "text",
                "text": prompt
            },
            {
                "type": "image_url",
                "image_url": {
                    "url": data_url
                }
            }
        ]),
        None => json!(prompt),
    }
}

async fn run_cloud_rewrite(state: &AppState, transcript: &str) -> Result<RewriteResult, String> {
    let clipboard = state.take_recording_clipboard().await;
    let screenshot = state.take_recording_screenshot().await;
    let settings = state.settings.read().await;
    let profile = settings.writing_profile.clone();
    let rewrite_settings = settings.cloud_rewrite.clone();
    let dictionary_replacements = settings.dictionary_replacements.clone();
    let transcript_history = settings.transcript_history.clone();
    let style_memories = settings.style_memories.clone();
    let project_memories = settings.project_memories.clone();
    drop(settings);

    let config = cloud_rewrite_config(&rewrite_settings)?;
    let use_minimal_context = should_use_minimal_rewrite_context(transcript);
    let screenshot_for_prompt = if use_minimal_context {
        None
    } else {
        screenshot.as_deref()
    };
    let system_context = build_template_context(
        if use_minimal_context {
            None
        } else {
            clipboard.clone()
        },
        screenshot_for_prompt,
        &dictionary_replacements,
        if use_minimal_context {
            &[]
        } else {
            &transcript_history
        },
        if use_minimal_context {
            &[]
        } else {
            &style_memories
        },
        if use_minimal_context {
            &[]
        } else {
            &project_memories
        },
        transcript,
    );
    let system_prompt = build_system_prompt(&profile, &system_context);
    let user_content = build_rewrite_message_content(
        build_rewrite_request(
            clipboard.clone(),
            &profile,
            &dictionary_replacements,
            &transcript_history,
            &style_memories,
            &project_memories,
            transcript,
        ),
        if use_minimal_context {
            None
        } else {
            screenshot
        },
    );

    let request_body = json!({
        "model": config.model,
        "messages": [
            {
                "role": "system",
                "content": system_prompt
            },
            {
                "role": "user",
                "content": user_content
            }
        ],
        "tools": [
            {
                "type": "function",
                "function": {
                    "name": "submit_result",
                    "description": "Submit final rewritten text. Optional memory updates can be encoded as JSON strings.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "text": {
                                "type": "string",
                                "description": "The speaker's final paste-ready rewritten text, preserving point of view and meaning. No explanation or commentary."
                            },
                            "dictionary_replacements": {
                                "type": "array",
                                "description": "Optional JSON strings like {\"from\":\"cloud code\",\"to\":\"claude code\",\"reason\":\"developer term\"}.",
                                "items": { "type": "string" }
                            },
                            "style_memories": {
                                "type": "array",
                                "description": "Optional JSON strings like {\"rule\":\"Keep bug reports concise\",\"reason\":\"user style\"}.",
                                "items": { "type": "string" }
                            },
                            "project_memories": {
                                "type": "array",
                                "description": "Optional JSON strings like {\"key\":\"current_project\",\"value\":\"Zana\",\"reason\":\"active dictation context\"}.",
                                "items": { "type": "string" }
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

        if looks_like_malformed_tool_call(&content) {
            return Err("Cloud rewrite returned malformed tool call".to_string());
        }

        Ok::<RewriteResult, String>(RewriteResult {
            text: content.trim().to_string(),
            memory: LearnedMemory::default(),
        })
    };

    let rewritten = match timeout(Duration::from_millis(config.timeout_ms), call).await {
        Ok(result) => result?,
        Err(_) => return Err("Cloud rewrite timed out".to_string()),
    };

    validate_rewrite_text(transcript, &rewritten.text)?;

    Ok(rewritten)
}

fn validate_rewrite_text(transcript: &str, rewritten: &str) -> Result<(), String> {
    if rewritten.trim().is_empty() {
        return Err("Cloud rewrite returned empty text".to_string());
    }

    if looks_like_assistant_meta_response(rewritten) {
        return Err(
            "Cloud rewrite returned assistant commentary instead of rewritten text".to_string(),
        );
    }

    let raw_len = transcript.trim().chars().count();
    let rewritten_len = rewritten.trim().chars().count();
    if raw_len > 0 && raw_len <= 120 {
        let max_reasonable_len = (raw_len * 3).max(raw_len + 80);
        if rewritten_len > max_reasonable_len {
            return Err("Cloud rewrite expanded short transcript too much".to_string());
        }
    }

    Ok(())
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
    cloud_rewrite_config_with_env(settings, |key| std::env::var(key).ok())
}

fn cloud_rewrite_config_with_env(
    settings: &crate::state::CloudRewriteSettings,
    env_get: impl Fn(&str) -> Option<String>,
) -> Result<CloudRewriteConfig, String> {
    let api_key = if settings.api_key.trim().is_empty() {
        env_get("ZANA_REWRITE_API_KEY")
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_default()
    } else {
        settings.api_key.clone()
    };

    let url = if settings.api_url.trim().is_empty() {
        env_get("ZANA_REWRITE_API_URL")
            .unwrap_or_else(|| "https://api.openai.com/v1/chat/completions".to_string())
    } else {
        settings.api_url.clone()
    };
    let url = normalize_rewrite_url(&url);

    if !is_allowed_rewrite_url(&url) {
        return Err(
            "Cloud rewrite URL must use https unless it is localhost or 127.0.0.1".to_string(),
        );
    }
    if !is_local_rewrite_url(&url) && api_key.trim().is_empty() {
        return Err("Cloud rewrite API key is required for remote providers".to_string());
    }

    let model = if settings.model.trim().is_empty() {
        env_get("ZANA_REWRITE_MODEL").unwrap_or_else(|| "gpt-4o-mini".to_string())
    } else {
        settings.model.clone()
    };

    let timeout_ms = if settings.timeout_ms == 0 {
        env_get("ZANA_REWRITE_TIMEOUT_MS")
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
        .filter_map(|proposal| proposal.into_value())
        .filter_map(|value| serde_json::from_value::<DictionaryReplacementProposal>(value).ok())
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
        .filter_map(|proposal| proposal.into_value())
        .filter_map(|value| serde_json::from_value::<StyleMemoryProposal>(value).ok())
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
        .filter_map(|proposal| proposal.into_value())
        .filter_map(|value| serde_json::from_value::<ProjectMemoryProposal>(value).ok())
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

fn looks_like_malformed_tool_call(text: &str) -> bool {
    let normalized = text.trim().to_ascii_lowercase();
    normalized.contains("<tool_call")
        || normalized.contains("</tool_call")
        || normalized.contains("<function=")
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
        apply_dictionary_replacements, build_rewrite_message_content, build_rewrite_request,
        build_system_prompt, cloud_rewrite_config, cloud_rewrite_config_with_env,
        looks_like_assistant_meta_response, looks_like_malformed_tool_call,
        rewrite_result_from_tool_arguments, should_use_minimal_rewrite_context,
        validate_rewrite_text,
    };
    use crate::state::{CloudRewriteSettings, DictionaryReplacement, WritingProfile};

    #[test]
    fn build_rewrite_request_renders_prompt_template_variables() {
        let profile = WritingProfile {
            purpose: "system".to_string(),
            tone: "Clipboard: {clipboard}\nCaptured: {captured}\nDictionary:\n{dictionary}"
                .to_string(),
            format: "Return {captured} as one paragraph.".to_string(),
        };

        let prompt = build_rewrite_request(
            Some("team status: launch next week".to_string()),
            &profile,
            &[DictionaryReplacement::enabled(
                "cloud code",
                "claude code",
                None,
            )],
            &[],
            &[],
            &[],
            "hey we shipped feature x yesterday",
        );

        assert!(prompt.contains("Clipboard:"));
        assert!(prompt.contains("team status: launch next week"));
        assert!(prompt.contains("Captured:"));
        assert!(prompt.contains("hey we shipped feature x yesterday"));
        assert!(prompt.contains("- cloud code => claude code"));
        assert!(prompt.contains("Response contract:"));
        assert!(prompt.contains("Return hey we shipped feature x yesterday as one paragraph."));
        assert!(!prompt.contains("{captured}"));
    }

    #[test]
    fn build_system_prompt_renders_time_and_attached_screenshot() {
        let profile = WritingProfile {
            purpose: "You are Zana. The time is {time}. Screenshot: {screen_shot}.".to_string(),
            ..WritingProfile::default()
        };

        let context = super::build_template_context(
            None,
            Some("data:image/jpeg;base64,abc"),
            &[],
            &[],
            &[],
            &[],
            "Testing testing one two three",
        );
        let prompt = build_system_prompt(&profile, &context);

        assert!(prompt.contains("You are Zana. The time is "));
        assert!(prompt.contains("Screenshot: attached."));
        assert!(prompt.contains("submit_result tool call"));
    }

    #[test]
    fn test_phrase_omits_clipboard_and_memory_context() {
        let profile = WritingProfile::default();
        let prompt = build_rewrite_request(
            Some("The test is verified and unverified, so I am testing this now.".to_string()),
            &profile,
            &[DictionaryReplacement::enabled(
                "testing",
                "verified and unverified",
                None,
            )],
            &[crate::state::TranscriptHistoryEntry {
                raw_text: "Testing testing one two three".to_string(),
                final_text: "The test is verified and unverified.".to_string(),
                used_rewrite: true,
                timestamp: "2026-05-28T17:52:14Z".to_string(),
            }],
            &[],
            &[],
            "Testing testing one two three",
        );

        assert!(prompt.contains("Ignore clipboard, screenshots, and prior conversation context."));
        assert!(prompt.contains("Clipboard: none"));
        assert!(prompt.contains("Here is what Whisper captured:\nTesting testing one two three"));
        assert!(!prompt.contains("verified and unverified"));
    }

    #[test]
    fn longer_dictation_keeps_context() {
        assert!(should_use_minimal_rewrite_context(
            "Testing testing one two three"
        ));
        assert!(!should_use_minimal_rewrite_context(
            "Can you prepare the repo for public release and make sure the checklist is updated"
        ));
    }

    #[test]
    fn rewrite_message_content_attaches_screenshot_when_available() {
        let content = build_rewrite_message_content(
            "rewrite this".to_string(),
            Some("data:image/jpeg;base64,abc".to_string()),
        );
        let items = content.as_array().expect("vision content should be array");

        assert_eq!(items[0]["type"], "text");
        assert_eq!(items[0]["text"], "rewrite this");
        assert_eq!(items[1]["type"], "image_url");
        assert_eq!(items[1]["image_url"]["url"], "data:image/jpeg;base64,abc");
    }

    #[test]
    fn cloud_rewrite_config_uses_saved_provider_settings() {
        let settings = CloudRewriteSettings {
            api_key: "sk-test".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_url: "https://example.com/v1/chat/completions".to_string(),
            timeout_ms: 12_000,
            include_screenshot: false,
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
    fn cloud_rewrite_config_rejects_remote_without_api_key() {
        let settings = CloudRewriteSettings {
            api_key: String::new(),
            api_url: "https://example.com/v1/chat/completions".to_string(),
            ..CloudRewriteSettings::default()
        };

        let error = cloud_rewrite_config_with_env(&settings, |_| None).unwrap_err();

        assert!(error.contains("API key"));
    }

    #[test]
    fn cloud_rewrite_config_allows_local_http_without_api_key() {
        let settings = CloudRewriteSettings {
            api_key: String::new(),
            model: "qwen3.5-0.8b".to_string(),
            api_url: "http://localhost:1234/v1/chat/completions".to_string(),
            timeout_ms: 15_000,
            include_screenshot: false,
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
            include_screenshot: false,
        };

        let config = cloud_rewrite_config(&settings).expect("local rewrite config should load");

        assert_eq!(config.url, "http://localhost:1234/v1/chat/completions");
    }

    #[test]
    fn parses_rewrite_text_from_tool_arguments() {
        let result = rewrite_result_from_tool_arguments(
            r#"{"text":"Testing one, two, three.","dictionary_replacements":["{\"from\":\"cloud code\",\"to\":\"claude code\",\"reason\":\"developer tool\"}"]}"#,
        )
            .expect("tool arguments should include text");

        assert_eq!(result.text, "Testing one, two, three.");
        assert_eq!(result.memory.dictionary_replacements.len(), 1);
        assert_eq!(result.memory.dictionary_replacements[0].from, "cloud code");
        assert_eq!(result.memory.dictionary_replacements[0].to, "claude code");
    }

    #[test]
    fn detects_malformed_tool_call_content() {
        assert!(looks_like_malformed_tool_call("\n</tool_call>"));
        assert!(looks_like_malformed_tool_call("<tool_call>"));
        assert!(!looks_like_malformed_tool_call("Testing one, two, three."));
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
    fn rejects_rewrite_that_bloats_short_transcript() {
        let transcript = "This is a test, testing one, two, three.";
        let rewritten = "The test is verified and unverified, so I am testing this now. This is a test, testing, testing, one, two, three. This is a test, testing, testing, one, two, three. This is a test, testing, testing, one, two, three.";

        let error = validate_rewrite_text(transcript, rewritten).unwrap_err();

        assert!(error.contains("expanded short transcript"));
    }

    #[test]
    fn allows_normal_cleanup_of_short_transcript() {
        let transcript = "this is a test testing one two three";
        let rewritten = "This is a test. Testing one, two, three.";

        assert!(validate_rewrite_text(transcript, rewritten).is_ok());
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
