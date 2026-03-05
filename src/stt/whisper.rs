//! Whisper Speech-to-Text Module
//!
//! Pure Rust implementation using whisper-rs (whisper.cpp bindings).
//! No Python dependency required - works in sandboxed apps.
//!
//! Ported from kollabor-app-v1 with hook system integration.

use crate::hooks::{EventBus, HookEvent, TranscriptionSegmentData};
use crate::errors::{Result, WhisperError};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Whisper model sizes
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WhisperModel {
    Tiny,
    Base,
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

impl Default for WhisperModel {
    fn default() -> Self {
        WhisperModel::Small
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
            _ => Err(format!("Unknown Whisper model: {}", s)),
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
        log::debug!("Initializing WhisperEngine");
        let models_dir = Self::get_models_dir()?;
        log::info!("Whisper models directory: {:?}", models_dir);

        Ok(Self {
            event_bus,
            context_cache: Arc::new(Mutex::new(None)),
            models_dir,
        })
    }

    /// Get the models directory path
    pub fn get_models_dir() -> Result<PathBuf> {
        let data_dir = dirs::data_local_dir()
            .ok_or_else(|| WhisperError::ModelsDirCreationFailed {
                path: PathBuf::from("unknown"),
                source: None,
            })?
            .join("Zana")
            .join("whisper-models");

        std::fs::create_dir_all(&data_dir).map_err(|e| WhisperError::ModelsDirCreationFailed {
            path: data_dir.clone(),
            source: Some(Box::new(e) as _),
        })?;

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
                "Model {} ({}) already exists at {:?}",
                model.name(),
                model.filename(),
                model_path
            );
            return Ok(model_path);
        }

        log::info!(
            "Downloading model {} ({} MB) from {}",
            model.name(),
            model.size_mb(),
            model.download_url()
        );

        let client = reqwest::Client::new();
        let response = client
            .get(model.download_url())
            .send()
            .await
            .map_err(|e| WhisperError::DownloadFailed {
                model: model.name().to_string(),
                size_mb: model.size_mb(),
                reason: "Failed to start download".to_string(),
                url: model.download_url(),
                source: Some(Box::new(e) as _),
            })?;

        let total_size = response.content_length().unwrap_or(0);
        log::debug!("Model download size: {} bytes", total_size);

        let mut downloaded: u64 = 0;
        let last_log = std::sync::atomic::AtomicU64::new(0);

        // Create temp file for atomic write
        let temp_path = model_path.with_extension("tmp");
        let mut file = tokio::fs::File::create(&temp_path)
            .await
            .map_err(|e| WhisperError::SaveFailed {
                path: temp_path.clone(),
                dir: self.models_dir.clone(),
                reason: "Failed to create temp file".to_string(),
                source: Some(Box::new(e) as _),
            })?;

        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;

        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| WhisperError::DownloadFailed {
                model: model.name().to_string(),
                size_mb: model.size_mb(),
                reason: "Error downloading chunk".to_string(),
                url: model.download_url(),
                source: Some(Box::new(e) as _),
            })?;
            file.write_all(&chunk)
                .await
                .map_err(|e| WhisperError::SaveFailed {
                    path: temp_path.clone(),
                    dir: self.models_dir.clone(),
                    reason: "Failed to write chunk".to_string(),
                    source: Some(Box::new(e) as _),
                })?;
            downloaded += chunk.len() as u64;

            // Log progress every 10%
            if total_size > 0 {
                let percent = downloaded * 100 / total_size;
                let last_percent = last_log.load(std::sync::atomic::Ordering::Relaxed);
                if percent >= last_percent + 10 {
                    log::debug!("Download progress: {}% ({} / {} bytes)", percent, downloaded, total_size);
                    last_log.store(percent, std::sync::atomic::Ordering::Relaxed);
                }
            }

            if let Some(ref cb) = progress_callback {
                cb(downloaded, total_size);
            }
        }

        file.flush().await.map_err(|e| WhisperError::SaveFailed {
            path: temp_path.clone(),
            dir: self.models_dir.clone(),
            reason: "Failed to flush file".to_string(),
            source: Some(Box::new(e) as _),
        })?;
        drop(file);

        // Atomic rename
        tokio::fs::rename(&temp_path, &model_path)
            .await
            .map_err(|e| WhisperError::SaveFailed {
                path: model_path.clone(),
                dir: self.models_dir.clone(),
                reason: "Failed to finalize model file".to_string(),
                source: Some(Box::new(e) as _),
            })?;

        log::info!("Model {} downloaded successfully ({:?})", model.name(), model_path);
        Ok(model_path)
    }

    /// Load or get cached whisper context
    async fn get_or_create_context(&self, model: WhisperModel) -> Result<()> {
        let mut guard = self.context_cache.lock().await;

        // Check if we have the right model loaded
        if let Some(ref cached) = *guard {
            if cached.model == model {
                log::trace!("Using cached model: {}", model.name());
                return Ok(());
            }
        }

        // Need to load new model
        let model_path = self.get_model_path(model);
        if !model_path.exists() {
            log::error!("Model file not found: {:?}", model_path);
            return Err(WhisperError::ModelNotFound {
                model: model.name().to_string(),
                path: model_path.clone(),
                size_mb: model.size_mb(),
                url: model.download_url(),
            }.into());
        }

        log::info!("Loading whisper model {} from {:?}", model.name(), model_path);
        let load_start = std::time::Instant::now();

        let ctx_params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(
            model_path.to_str().ok_or_else(|| WhisperError::ModelLoadFailed {
                model: model.name().to_string(),
                reason: "Invalid model path characters".to_string(),
                source: None,
            })?,
            ctx_params,
        )
        .map_err(|e| WhisperError::ModelLoadFailed {
            model: model.name().to_string(),
            reason: "Failed to load model file".to_string(),
            source: Some(Box::new(e) as _),
        })?;

        *guard = Some(CachedContext { model, ctx });

        log::info!(
            "Whisper model {} loaded successfully in {}ms",
            model.name(),
            load_start.elapsed().as_millis()
        );
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

        log::info!(
            "Starting transcription: model={}, audio_duration={}ms, samples={}",
            model.name(),
            audio_duration_ms,
            samples.len()
        );

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
            let cached = guard.as_ref().ok_or_else(|| WhisperError::ContextCreationFailed {
                reason: "Whisper context not initialized".to_string(),
                source: None,
            })?;

            log::trace!("Creating whisper state");
            let mut state = cached
                .ctx
                .create_state()
                .map_err(|e| WhisperError::ContextCreationFailed {
                    reason: "Failed to create state".to_string(),
                    source: Some(Box::new(e) as _),
                })?;

            let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
            params.set_language(Some("en"));
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);
            params.set_suppress_blank(true);
            params.set_suppress_nst(true);

            log::trace!("Running whisper transcription");
            state.full(params, samples).map_err(|e| WhisperError::TranscriptionFailed {
                reason: "Whisper transcription failed".to_string(),
                source: Some(Box::new(e) as _),
            })?;

            let num_segments = state.full_n_segments();
            log::debug!("Transcription produced {} segments", num_segments);

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
                        start_ms: start as i64 * 10,
                        end_ms: end as i64 * 10,
                        text: segment_text.clone(),
                    };

                    log::trace!(
                        "Segment {}: {}ms - {}ms: '{}'",
                        i,
                        seg.start_ms,
                        seg.end_ms,
                        seg.text
                    );

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

            let processing_ms = start_time.elapsed().as_millis() as u64;
            let real_time_factor = if audio_duration_ms > 0 {
                processing_ms as f64 / audio_duration_ms as f64
            } else {
                0.0
            };

            log::info!(
                "Transcription complete: {} segments, {} chars, {}ms (RTF: {:.2}x)",
                segments.len(),
                full_text.len(),
                processing_ms,
                real_time_factor
            );

            TranscriptionResult {
                text: full_text.trim().to_string(),
                segments,
                processing_ms,
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

        Ok(result)
    }

    /// Transcribe audio from a WAV file
    pub async fn transcribe_file(
        &self,
        path: &str,
        model: WhisperModel,
    ) -> Result<TranscriptionResult> {
        log::info!("Transcribing file '{}' with model {}", path, model.name());

        let samples = load_wav_file(path)?;
        log::debug!("Loaded {} samples from file", samples.len());

        self.transcribe(&samples, model).await
    }
}

/// Load and convert WAV file to f32 samples at 16kHz mono
fn load_wav_file(path: &str) -> Result<Vec<f32>> {
    use hound::WavReader;

    if !std::path::Path::new(path).exists() {
        log::error!("WAV file does not exist: {}", path);
        return Err(WhisperError::FileNotFound { path: path.to_string() }.into());
    }

    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    log::debug!("Opening WAV file: {}, size: {} bytes", path, file_size);

    let mut reader = WavReader::open(path).map_err(|e| WhisperError::InvalidWavFormat {
        reason: format!("Failed to open WAV file: {}", path),
        source: Some(Box::new(e) as _),
    })?;
    let spec = reader.spec();

    log::info!(
        "Loading WAV file: {} ch, {} Hz, {} bits, {} samples",
        spec.channels,
        spec.sample_rate,
        spec.bits_per_sample,
        reader.duration()
    );

    // Read samples based on format
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_val = (1 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max_val))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| WhisperError::InvalidWavFormat {
                    reason: "Failed to read int samples".to_string(),
                    source: Some(Box::new(e) as _),
                })?
        }
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| WhisperError::InvalidWavFormat {
                reason: "Failed to read float samples".to_string(),
                source: Some(Box::new(e) as _),
            })?,
    };

    log::trace!("Read {} samples from WAV file", samples.len());

    // Convert stereo to mono
    let mono_samples = if spec.channels == 2 {
        log::debug!("Converting stereo to mono");
        samples
            .chunks(2)
            .map(|chunk| (chunk[0] + chunk.get(1).copied().unwrap_or(0.0)) / 2.0)
            .collect()
    } else if spec.channels > 2 {
        log::debug!("Converting {} channels to mono (average)", spec.channels);
        samples
            .chunks(spec.channels as usize)
            .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
            .collect()
    } else {
        log::trace!("Audio is already mono");
        samples
    };

    // Resample to 16kHz if needed
    let target_rate = 16000;
    let resampled = if spec.sample_rate != target_rate {
        log::debug!("Resampling from {} Hz to {} Hz", spec.sample_rate, target_rate);
        resample(&mono_samples, spec.sample_rate, target_rate)
    } else {
        mono_samples
    };

    log::info!("Loaded {} samples ({} ms audio)", resampled.len(), resampled.len() as u64 * 1000 / 16000);
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
        assert_eq!(WhisperModel::from_str("tiny"), Some(WhisperModel::Tiny));
        assert_eq!(WhisperModel::from_str("SMALL"), Some(WhisperModel::Small));
        assert_eq!(WhisperModel::from_str("large-v3"), Some(WhisperModel::Large));
        assert_eq!(WhisperModel::from_str("invalid"), None);
    }

    #[test]
    fn test_resample() {
        let samples = vec![0.0, 1.0, 0.0, -1.0];
        let resampled = resample(&samples, 48000, 16000);
        assert!(resampled.len() < samples.len());
    }
}
