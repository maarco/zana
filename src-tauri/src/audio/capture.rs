//! Audio Capture Module
//!
//! Cross-platform audio capture using cpal with real-time level monitoring
//! and FFT analysis for visualization.
//!
//! This module uses a dedicated audio thread to handle the cpal::Stream
//! which is not Send, and communicates via channels.

use crate::hooks::{EventBus, HookEvent};
use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tokio::sync::{mpsc, oneshot, RwLock};

/// Audio device information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AudioDevice {
    /// Device identifier (name-based)
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Whether this is the system default
    pub is_default: bool,
    /// Sample rate
    pub sample_rate: Option<u32>,
    /// Channel count
    pub channels: Option<u16>,
}

/// Audio capture configuration
#[derive(Debug, Clone)]
pub struct CaptureConfig {
    /// Target sample rate (will resample if device differs)
    pub sample_rate: u32,
    /// Target channels (will downmix if device differs)
    pub channels: u16,
    /// FFT size for frequency analysis
    pub fft_size: usize,
    /// Audio level smoothing factor (0-1, higher = smoother)
    pub level_smoothing: f32,
    /// Maximum recording duration in seconds (caps memory usage)
    pub max_duration_secs: u64,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000, // Whisper's native rate
            channels: 1,        // Mono
            fft_size: 64,       // 32 frequency bins
            level_smoothing: 0.8,
            max_duration_secs: 300, // 5 minutes max (~19MB at 16kHz)
        }
    }
}

/// Real-time audio metrics for visualization
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct AudioMetrics {
    /// Average level (0.0 - 1.0)
    pub level: f32,
    /// Peak level (0.0 - 1.0)
    pub peak: f32,
    /// FFT frequency bins (normalized 0.0 - 1.0)
    pub fft_bins: Vec<f32>,
    /// Whether audio is currently being captured
    pub is_active: bool,
    /// Duration captured in milliseconds
    pub duration_ms: u64,
}

/// Shared state for audio capture (thread-safe)
struct CaptureState {
    /// Accumulated samples for transcription
    samples: Vec<f32>,
    /// Current audio level (smoothed)
    level: f32,
    /// Peak level
    peak: f32,
    /// FFT bins
    fft_bins: Vec<f32>,
    /// Sample count for duration calculation
    sample_count: u64,
    /// Device sample rate
    device_sample_rate: u32,
    /// Maximum samples to accumulate (prevents unbounded memory growth)
    max_samples: u64,
}

/// Command to the audio thread
enum AudioCommand {
    Start {
        device_id: Option<String>,
        response: oneshot::Sender<Result<AudioStartInfo>>,
    },
    Stop {
        response: oneshot::Sender<Result<CapturedAudio>>,
    },
}

/// Information returned when starting audio capture
struct AudioStartInfo {
    device_name: String,
    sample_rate: u32,
    channels: u16,
}

/// Audio capture engine
///
/// Manages audio capture on a dedicated thread since cpal::Stream is not Send.
pub struct AudioCapture {
    /// Event bus for hooks
    event_bus: Arc<EventBus>,
    /// Capture configuration
    #[allow(dead_code)]
    config: CaptureConfig,
    /// Shared capture state
    state: Arc<RwLock<CaptureState>>,
    /// Recording active flag
    is_recording: Arc<AtomicBool>,
    /// Command sender to audio thread
    command_tx: mpsc::UnboundedSender<AudioCommand>,
    /// Audio thread handle
    _audio_thread: Option<thread::JoinHandle<()>>,
}

// Mark AudioCapture as Send + Sync since all its fields are thread-safe
// (the audio thread is separate and we communicate via channels)
unsafe impl Send for AudioCapture {}
unsafe impl Sync for AudioCapture {}

impl AudioCapture {
    /// Create a new audio capture engine
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self::with_config(event_bus, CaptureConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(event_bus: Arc<EventBus>, config: CaptureConfig) -> Self {
        // Pre-calculate max samples: max_duration * max_expected_device_rate
        // Use 48kHz as worst-case device rate for the cap
        let max_samples = config.max_duration_secs * 48000;

        let state = Arc::new(RwLock::new(CaptureState {
            samples: Vec::new(),
            level: 0.0,
            peak: 0.0,
            fft_bins: vec![0.0; 32],
            sample_count: 0,
            device_sample_rate: 48000,
            max_samples,
        }));

        let is_recording = Arc::new(AtomicBool::new(false));
        let (command_tx, command_rx) = mpsc::unbounded_channel();

        // Spawn the audio thread
        let state_clone = state.clone();
        let is_recording_clone = is_recording.clone();
        let smoothing = config.level_smoothing;

        let audio_thread = thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create audio thread runtime");

            rt.block_on(audio_thread_main(
                command_rx,
                state_clone,
                is_recording_clone,
                smoothing,
            ));
        });

        Self {
            event_bus,
            config,
            state,
            is_recording,
            command_tx,
            _audio_thread: Some(audio_thread),
        }
    }

    /// List available input devices
    pub fn list_devices() -> Result<Vec<AudioDevice>> {
        let host = cpal::default_host();
        let mut devices = Vec::new();

        let default_name = host.default_input_device().and_then(|d| d.name().ok());

        let input_devices = host
            .input_devices()
            .context("Failed to enumerate input devices")?;

        for device in input_devices {
            let name = device.name().unwrap_or_else(|_| "Unknown".to_string());
            let is_default = default_name.as_ref().map(|d| d == &name).unwrap_or(false);

            // Get supported config
            let (sample_rate, channels) = device
                .default_input_config()
                .ok()
                .map(|c| (Some(c.sample_rate().0), Some(c.channels())))
                .unwrap_or((None, None));

            devices.push(AudioDevice {
                id: name.clone(),
                name,
                is_default,
                sample_rate,
                channels,
            });
        }

        if devices.is_empty() {
            anyhow::bail!("No audio input devices found");
        }

        log::info!("Found {} audio input device(s)", devices.len());
        Ok(devices)
    }

    /// Start capturing audio
    pub async fn start(&self, device_id: Option<&str>) -> Result<()> {
        // Check if already recording
        if self.is_recording.load(Ordering::SeqCst) {
            anyhow::bail!("Recording already in progress");
        }

        // Clear previous state and free memory
        {
            let mut state = self.state.write().await;
            state.samples = Vec::new(); // Drop old allocation entirely
            state.sample_count = 0;
            state.level = 0.0;
            state.peak = 0.0;
        }

        // Send start command to audio thread
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(AudioCommand::Start {
                device_id: device_id.map(String::from),
                response: response_tx,
            })
            .map_err(|_| anyhow::anyhow!("Audio thread not running"))?;

        // Wait for response
        let info = response_rx
            .await
            .map_err(|_| anyhow::anyhow!("Audio thread died"))??;

        // Update recording state
        self.is_recording.store(true, Ordering::SeqCst);

        // Emit start event
        self.event_bus
            .emit(HookEvent::AudioCaptureStart {
                device_id: info.device_name.clone(),
                sample_rate: info.sample_rate,
                channels: info.channels,
            })
            .await;

        // Start metrics broadcast task
        let event_bus = self.event_bus.clone();
        let state = self.state.clone();
        let is_recording = self.is_recording.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(16));

            loop {
                interval.tick().await;

                if !is_recording.load(Ordering::SeqCst) {
                    break;
                }

                let (level, peak, fft_bins) = {
                    let state = state.read().await;
                    (state.level, state.peak, state.fft_bins.clone())
                };

                // Emit level change event
                event_bus
                    .emit(HookEvent::AudioLevelChange { level, peak })
                    .await;

                // Emit FFT event (move the vec instead of cloning again)
                let bin_count = fft_bins.len();
                event_bus
                    .emit(HookEvent::AudioFftReady {
                        bins: fft_bins,
                        bin_count,
                    })
                    .await;
            }
        });

        log::info!("Audio capture started");
        Ok(())
    }

    /// Stop capturing and return samples
    pub async fn stop(&self) -> Result<CapturedAudio> {
        if !self.is_recording.load(Ordering::SeqCst) {
            anyhow::bail!("No recording in progress");
        }

        // Send stop command to audio thread
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(AudioCommand::Stop {
                response: response_tx,
            })
            .map_err(|_| anyhow::anyhow!("Audio thread not running"))?;

        // Wait for response
        let captured = response_rx
            .await
            .map_err(|_| anyhow::anyhow!("Audio thread died"))??;

        self.is_recording.store(false, Ordering::SeqCst);

        // Emit stop event
        self.event_bus
            .emit(HookEvent::AudioCaptureStop {
                duration_ms: captured.duration_ms,
            })
            .await;

        log::info!(
            "Audio capture stopped: {} samples, {}ms",
            captured.samples.len(),
            captured.duration_ms
        );

        Ok(captured)
    }

    /// Get current audio metrics (for UI)
    pub async fn get_metrics(&self) -> AudioMetrics {
        let state = self.state.read().await;
        let duration_ms =
            (state.sample_count as f64 / state.device_sample_rate as f64 * 1000.0) as u64;

        AudioMetrics {
            level: state.level,
            peak: state.peak,
            fft_bins: state.fft_bins.clone(),
            is_active: self.is_recording.load(Ordering::SeqCst),
            duration_ms,
        }
    }

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }
}

/// Audio thread main function
async fn audio_thread_main(
    mut command_rx: mpsc::UnboundedReceiver<AudioCommand>,
    state: Arc<RwLock<CaptureState>>,
    _is_recording: Arc<AtomicBool>,
    smoothing: f32,
) {
    // Keep stream alive until stop is called
    let mut _current_stream: Option<Stream> = None;

    while let Some(cmd) = command_rx.recv().await {
        match cmd {
            AudioCommand::Start {
                device_id,
                response,
            } => {
                let result = start_capture(device_id.as_deref(), state.clone(), smoothing).await;

                match result {
                    Ok((stream, info)) => {
                        _current_stream = Some(stream);
                        let _ = response.send(Ok(info));
                    }
                    Err(e) => {
                        let _ = response.send(Err(e));
                    }
                }
            }
            AudioCommand::Stop { response } => {
                // Drop the stream to stop capture
                _current_stream = None;

                // Get captured data, resample, then free the buffer
                let captured = {
                    let mut state = state.write().await;
                    let duration_ms = (state.sample_count as f64 / state.device_sample_rate as f64
                        * 1000.0) as u64;

                    // Resample to 16kHz if needed (for Whisper)
                    let resampled = if state.device_sample_rate != 16000 {
                        resample(&state.samples, state.device_sample_rate, 16000)
                    } else {
                        std::mem::take(&mut state.samples)
                    };

                    // Free the samples buffer immediately (don't hold ~100s of MB)
                    state.samples = Vec::new();

                    CapturedAudio {
                        samples: resampled,
                        sample_rate: 16000,
                        channels: 1,
                        duration_ms,
                    }
                };

                let _ = response.send(Ok(captured));
            }
        }
    }
}

/// Start audio capture and return the stream
async fn start_capture(
    device_id: Option<&str>,
    state: Arc<RwLock<CaptureState>>,
    smoothing: f32,
) -> Result<(Stream, AudioStartInfo)> {
    let device = get_device(device_id)?;
    let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());

    let supported_config = device
        .default_input_config()
        .context("Failed to get default input config")?;

    let sample_format = supported_config.sample_format();
    let config: StreamConfig = supported_config.into();

    log::info!(
        "Starting capture: {} Hz, {} ch, {:?}",
        config.sample_rate.0,
        config.channels,
        sample_format
    );

    // Update state with device sample rate
    {
        let mut state = state.write().await;
        state.device_sample_rate = config.sample_rate.0;
    }

    let channels = config.channels as usize;

    // Build stream based on sample format
    let stream = match sample_format {
        SampleFormat::I16 => {
            let state = state.clone();
            device.build_input_stream(
                &config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    process_samples_i16(data, &state, channels, smoothing);
                },
                |err| log::error!("Audio stream error: {}", err),
                None,
            )?
        }
        SampleFormat::F32 => {
            let state = state.clone();
            device.build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    process_samples_f32(data, &state, channels, smoothing);
                },
                |err| log::error!("Audio stream error: {}", err),
                None,
            )?
        }
        _ => anyhow::bail!("Unsupported sample format: {:?}", sample_format),
    };

    stream.play().context("Failed to start audio stream")?;

    let info = AudioStartInfo {
        device_name,
        sample_rate: config.sample_rate.0,
        channels: config.channels,
    };

    Ok((stream, info))
}

/// Get device by ID (name) or default
fn get_device(device_id: Option<&str>) -> Result<Device> {
    let host = cpal::default_host();

    if let Some(id) = device_id {
        let devices = host
            .input_devices()
            .context("Failed to enumerate devices")?;

        for device in devices {
            if let Ok(name) = device.name() {
                if name == id {
                    log::info!("Using audio device: {}", name);
                    return Ok(device);
                }
            }
        }

        log::warn!("Device '{}' not found, using default", id);
    }

    host.default_input_device()
        .context("No default input device available")
}

/// Process i16 samples
fn process_samples_i16(
    data: &[i16],
    state: &Arc<RwLock<CaptureState>>,
    channels: usize,
    smoothing: f32,
) {
    // Convert to f32 and process
    let float_data: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
    process_samples(&float_data, state, channels, smoothing);
}

/// Process f32 samples
fn process_samples_f32(
    data: &[f32],
    state: &Arc<RwLock<CaptureState>>,
    channels: usize,
    smoothing: f32,
) {
    process_samples(data, state, channels, smoothing);
}

/// Common sample processing
fn process_samples(
    data: &[f32],
    state: &Arc<RwLock<CaptureState>>,
    channels: usize,
    smoothing: f32,
) {
    // Convert to mono if needed
    let mono_samples: Vec<f32> = if channels > 1 {
        data.chunks(channels)
            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        data.to_vec()
    };

    // Calculate level and peak for this chunk
    let mut sum = 0.0f32;
    let mut peak = 0.0f32;

    for &sample in &mono_samples {
        let abs = sample.abs();
        sum += abs;
        peak = peak.max(abs);
    }

    let avg = if !mono_samples.is_empty() {
        sum / mono_samples.len() as f32
    } else {
        0.0
    };

    // Update state (blocking in audio thread - keep it fast)
    if let Ok(mut state) = state.try_write() {
        // Accumulate samples for transcription (capped to prevent unbounded growth)
        if (state.samples.len() as u64) < state.max_samples {
            let remaining = (state.max_samples - state.samples.len() as u64) as usize;
            let to_add = mono_samples.len().min(remaining);
            state.samples.extend_from_slice(&mono_samples[..to_add]);
        }
        state.sample_count += mono_samples.len() as u64;

        // Smooth level
        state.level = state.level * smoothing + avg * (1.0 - smoothing);
        state.peak = state.peak.max(peak) * 0.99 + peak * 0.01; // Slow decay

        // Simple FFT approximation (frequency band energy)
        let bin_count = state.fft_bins.len();
        let samples_per_bin = mono_samples.len() / bin_count;

        if samples_per_bin > 0 {
            for (i, bin) in state.fft_bins.iter_mut().enumerate() {
                let start = i * samples_per_bin;
                let end = (start + samples_per_bin).min(mono_samples.len());

                let energy: f32 = mono_samples[start..end]
                    .iter()
                    .map(|s| s.abs())
                    .sum::<f32>()
                    / samples_per_bin as f32;

                // Smooth and normalize
                *bin = *bin * 0.7 + energy * 2.0 * 0.3;
                *bin = bin.min(1.0);
            }
        }
    }
}

/// Captured audio data ready for transcription
#[derive(Debug, Clone)]
pub struct CapturedAudio {
    /// Audio samples (f32, mono)
    pub samples: Vec<f32>,
    /// Sample rate (always 16000 for Whisper)
    pub sample_rate: u32,
    /// Channel count (always 1)
    pub channels: u16,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

/// Linear interpolation resampling
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
            let frac = (src_idx - idx as f64) as f32;

            let s0 = samples.get(idx).copied().unwrap_or(0.0);
            let s1 = samples.get(idx + 1).copied().unwrap_or(s0);

            s0 + (s1 - s0) * frac
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resample() {
        let samples = vec![0.0, 1.0, 0.0, -1.0, 0.0, 1.0, 0.0, -1.0];
        let resampled = resample(&samples, 48000, 16000);
        assert!(resampled.len() < samples.len());
    }

    #[test]
    #[ignore = "requires local audio hardware and can hang on hosted macOS runners"]
    fn test_list_devices() {
        let result = AudioCapture::list_devices();
        let _ = result;
    }
}
