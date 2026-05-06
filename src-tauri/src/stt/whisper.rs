//! Whisper Speech-to-Text Module
//!
//! Pure Rust implementation using whisper-rs (whisper.cpp bindings).
//! No Python dependency required - works in sandboxed apps.
//!
//! Ported from kollabor-app-v1 with hook system integration.

use crate::hooks::{EventBus, HookEvent, TranscriptionSegmentData};
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Whisper model sizes
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WhisperModel {
    Tiny,
    Base,
    #[default]
    Small,
    Medium,
    Large,
}

impl WhisperModel {
    /// Get the model filename
    pub fn filename(&self) -> &'static str {
        match self {
            WhisperModel::Tiny => "ggml-tiny.bin",
            WhisperModel::Base => "ggml-base.bin",
            WhisperModel::Small => "ggml-small.bin",
            WhisperModel::Medium => "ggml-medium.bin",
            WhisperModel::Large => "ggml-large-v3.bin",
        }
    }

    /// Get download URL from Hugging Face
    pub fn download_url(&self) -> String {
        format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
            self.filename()
        )
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            WhisperModel::Tiny => "Tiny",
            WhisperModel::Base => "Base",
            WhisperModel::Small => "Small",
            WhisperModel::Medium => "Medium",
            WhisperModel::Large => "Large v3",
        }
    }

    /// Get approximate model size
    pub fn size_mb(&self) -> u64 {
        match self {
            WhisperModel::Tiny => 39,
            WhisperModel::Base => 140,
            WhisperModel::Small => 466,
            WhisperModel::Medium => 1500,
            WhisperModel::Large => 2900,
        }
    }
}

impl std::fmt::Display for WhisperModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl FromStr for WhisperModel {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "tiny" => Ok(WhisperModel::Tiny),
            "base" => Ok(WhisperModel::Base),
            "small" => Ok(WhisperModel::Small),
            "medium" => Ok(WhisperModel::Medium),
            "large" | "large-v3" => Ok(WhisperModel::Large),
            _ => Err(format!("unknown Whisper model: {s}")),
        }
    }
}

/// Cached whisper context
struct CachedContext {
    model: WhisperModel,
    ctx: WhisperContext,
}

/// Transcription result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TranscriptionResult {
    pub text: String,
    pub segments: Vec<TranscriptionSegment>,
    pub processing_ms: u64,
}

/// Transcription segment
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TranscriptionSegment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

/// Whisper STT Engine
///
/// Manages whisper model loading and transcription with hook integration.
#[derive(Clone)]
pub struct WhisperEngine {
    /// Event bus for emitting hooks
    event_bus: Arc<EventBus>,

    /// Cached context (lazily loaded)
    context_cache: Arc<Mutex<Option<CachedContext>>>,

    /// Models directory
    models_dir: PathBuf,
}

impl WhisperEngine {
    /// Create a new Whisper engine
    pub fn new(event_bus: Arc<EventBus>) -> Result<Self> {
        let models_dir = Self::get_models_dir()?;

        Ok(Self {
            event_bus,
            context_cache: Arc::new(Mutex::new(None)),
            models_dir,
        })
    }

    /// Get the models directory path
    pub fn get_models_dir() -> Result<PathBuf> {
        let data_dir = dirs::data_local_dir()
            .context("Failed to get local data directory")?
            .join("Zana")
            .join("whisper-models");

        std::fs::create_dir_all(&data_dir).context("Failed to create whisper models directory")?;

        Ok(data_dir)
    }

    /// Get path to a specific model
    pub fn get_model_path(&self, model: WhisperModel) -> PathBuf {
        self.models_dir.join(model.filename())
    }

    /// Check if a model is downloaded
    pub fn is_model_downloaded(&self, model: WhisperModel) -> bool {
        self.get_model_path(model).exists()
    }

    /// Preload the whisper model (non-blocking)
    pub async fn preload_model(&self, model: WhisperModel) -> Result<()> {
        self.get_or_create_context(model).await
    }

    /// Get list of downloaded models
    pub fn downloaded_models(&self) -> Vec<WhisperModel> {
        [
            WhisperModel::Tiny,
            WhisperModel::Base,
            WhisperModel::Small,
            WhisperModel::Medium,
            WhisperModel::Large,
        ]
        .into_iter()
        .filter(|m| self.is_model_downloaded(*m))
        .collect()
    }

    /// Download a whisper model
    pub async fn download_model<F>(
        &self,
        model: WhisperModel,
        progress_callback: Option<F>,
    ) -> Result<PathBuf>
    where
        F: Fn(u64, u64) + Send,
    {
        let model_path = self.get_model_path(model);

        if model_path.exists() {
            log::info!(
                "Model {} already exists at {:?}",
                model.filename(),
                model_path
            );
            return Ok(model_path);
        }

        log::info!(
            "Downloading model {} from {}",
            model.filename(),
            model.download_url()
        );

        let client = reqwest::Client::new();
        let response = client
            .get(model.download_url())
            .send()
            .await
            .context("Failed to start model download")?;

        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;

        // Create temp file for atomic write
        let temp_path = model_path.with_extension("tmp");
        let mut file = tokio::fs::File::create(&temp_path)
            .await
            .context("Failed to create temp model file")?;

        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;

        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Error downloading model chunk")?;
            file.write_all(&chunk)
                .await
                .context("Failed to write model chunk")?;
            downloaded += chunk.len() as u64;

            if let Some(ref cb) = progress_callback {
                cb(downloaded, total_size);
            }
        }

        file.flush().await?;
        drop(file);

        // Atomic rename
        tokio::fs::rename(&temp_path, &model_path)
            .await
            .context("Failed to finalize model file")?;

        log::info!("Model downloaded successfully to {:?}", model_path);
        Ok(model_path)
    }

    /// Load or get cached whisper context
    async fn get_or_create_context(&self, model: WhisperModel) -> Result<()> {
        let mut guard = self.context_cache.lock().await;

        // Check if we have the right model loaded
        if let Some(ref cached) = *guard {
            if cached.model == model {
                return Ok(());
            }
        }

        // Need to load new model
        let model_path = self.get_model_path(model);
        if !model_path.exists() {
            anyhow::bail!(
                "Model not found: {:?}. Please download it first.",
                model_path
            );
        }

        log::info!("Loading whisper model from {:?}", model_path);

        let ctx_params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(
            model_path.to_str().context("Invalid model path")?,
            ctx_params,
        )
        .context("Failed to load whisper model")?;

        *guard = Some(CachedContext { model, ctx });

        log::info!("Whisper model loaded successfully");
        Ok(())
    }

    /// Transcribe audio samples
    ///
    /// Samples should be f32, mono, 16kHz
    pub async fn transcribe(
        &self,
        samples: &[f32],
        model: WhisperModel,
    ) -> Result<TranscriptionResult> {
        let start_time = std::time::Instant::now();

        // Calculate audio duration
        let audio_duration_ms = (samples.len() as f64 / 16000.0 * 1000.0) as u64;

        // Emit start event
        self.event_bus
            .emit(HookEvent::TranscriptionStart {
                model: model.to_string(),
                audio_duration_ms,
            })
            .await;

        // Ensure model is loaded
        self.get_or_create_context(model).await?;

        // Run transcription
        let result = {
            let guard = self.context_cache.lock().await;
            let cached = guard.as_ref().context("Whisper context not initialized")?;

            let mut state = cached
                .ctx
                .create_state()
                .context("Failed to create whisper state")?;

            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            params.set_language(Some("en"));
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);
            params.set_suppress_blank(true);
            params.set_suppress_nst(true);

            state
                .full(params, samples)
                .context("Transcription failed")?;

            let num_segments = state.full_n_segments();
            let mut segments = Vec::new();
            let mut full_text = String::new();

            for i in 0..num_segments {
                if let Some(segment) = state.get_segment(i) {
                    let segment_text = segment
                        .to_str_lossy()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|_| String::new());
                    let start = segment.start_timestamp();
                    let end = segment.end_timestamp();

                    let seg = TranscriptionSegment {
                        start_ms: start * 10,
                        end_ms: end * 10,
                        text: segment_text.clone(),
                    };

                    // Emit segment event
                    self.event_bus
                        .emit(HookEvent::TranscriptionSegment {
                            start_ms: seg.start_ms,
                            end_ms: seg.end_ms,
                            text: seg.text.clone(),
                        })
                        .await;

                    segments.push(seg);
                    full_text.push_str(&segment_text);
                }
            }

            TranscriptionResult {
                text: full_text.trim().to_string(),
                segments,
                processing_ms: start_time.elapsed().as_millis() as u64,
            }
        };

        // Emit complete event
        self.event_bus
            .emit(HookEvent::TranscriptionComplete {
                text: result.text.clone(),
                segments: result
                    .segments
                    .iter()
                    .map(|s| TranscriptionSegmentData {
                        start_ms: s.start_ms,
                        end_ms: s.end_ms,
                        text: s.text.clone(),
                    })
                    .collect(),
                processing_ms: result.processing_ms,
            })
            .await;

        log::info!(
            "Transcription complete: {} segments, {} chars in {}ms",
            result.segments.len(),
            result.text.len(),
            result.processing_ms
        );

        Ok(result)
    }

    /// Transcribe audio from a WAV file
    pub async fn transcribe_file(
        &self,
        path: &str,
        model: WhisperModel,
    ) -> Result<TranscriptionResult> {
        log::info!("Transcribing file: {} with model {:?}", path, model);

        let samples = load_wav_file(path)?;
        self.transcribe(&samples, model).await
    }
}

/// Load and convert WAV file to f32 samples at 16kHz mono
fn load_wav_file(path: &str) -> Result<Vec<f32>> {
    use hound::WavReader;

    if !std::path::Path::new(path).exists() {
        anyhow::bail!("WAV file does not exist: {}", path);
    }

    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    log::info!("Opening WAV file: {}, size: {} bytes", path, file_size);

    let mut reader = WavReader::open(path).with_context(|| {
        format!(
            "Failed to open WAV file: {} (size: {} bytes)",
            path, file_size
        )
    })?;
    let spec = reader.spec();

    log::info!(
        "Loading WAV: {} channels, {} Hz, {} bits",
        spec.channels,
        spec.sample_rate,
        spec.bits_per_sample
    );

    // Read samples based on format
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_val = (1 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max_val))
                .collect::<Result<Vec<_>, _>>()
                .context("Failed to read samples")?
        }
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to read samples")?,
    };

    // Convert stereo to mono
    let mono_samples = if spec.channels == 2 {
        samples
            .chunks(2)
            .map(|chunk| (chunk[0] + chunk.get(1).copied().unwrap_or(0.0)) / 2.0)
            .collect()
    } else if spec.channels > 2 {
        samples
            .chunks(spec.channels as usize)
            .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
            .collect()
    } else {
        samples
    };

    // Resample to 16kHz if needed
    let target_rate = 16000;
    let resampled = if spec.sample_rate != target_rate {
        resample(&mono_samples, spec.sample_rate, target_rate)
    } else {
        mono_samples
    };

    Ok(resampled)
}

/// Simple linear interpolation resampling
fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let new_len = (samples.len() as f64 / ratio) as usize;

    (0..new_len)
        .map(|i| {
            let src_idx = i as f64 * ratio;
            let idx = src_idx as usize;
            let frac = src_idx - idx as f64;

            let s0 = samples.get(idx).copied().unwrap_or(0.0);
            let s1 = samples.get(idx + 1).copied().unwrap_or(s0);

            s0 + (s1 - s0) * frac as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_from_str() {
        assert_eq!("tiny".parse::<WhisperModel>(), Ok(WhisperModel::Tiny));
        assert_eq!("SMALL".parse::<WhisperModel>(), Ok(WhisperModel::Small));
        assert_eq!("large-v3".parse::<WhisperModel>(), Ok(WhisperModel::Large));
        assert!("invalid".parse::<WhisperModel>().is_err());
    }

    #[test]
    fn test_resample() {
        let samples = vec![0.0, 1.0, 0.0, -1.0];
        let resampled = resample(&samples, 48000, 16000);
        assert!(resampled.len() < samples.len());
    }
}
