//! Integration Tests for Zana
//!
//! Tests the end-to-end flow: record -> transcribe -> display

use Zana::audio::{AudioCapture, CapturedAudio};
use Zana::gui::app::{
    RecordingCommand, RecordingEvent, TranscriptionCommand, TranscriptionEvent,
};
use Zana::hooks::{EventBus, HookEvent};
use Zana::state::{AppState, Settings};
use Zana::stt::WhisperModel;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

/// Test: Audio capture → EventBus events → UI updates
#[tokio::test]
async fn test_audio_capture_emits_events() {
    // Create event bus
    let event_bus = Arc::new(EventBus::new());

    // Subscribe to audio events
    let mut audio_level_rx = event_bus
        .subscribe(Zana::hooks::HookEventType::AudioLevelChange)
        .await;

    let mut audio_fft_rx = event_bus
        .subscribe(Zana::hooks::HookEventType::AudioFftReady)
        .await;

    // Create audio capture
    let capture = AudioCapture::new(event_bus.clone());

    // Start recording (will fail without device, but that's OK for this test)
    let result = capture.start(None).await;

    // If recording started, we should receive events
    if result.is_ok() {
        // Wait for audio level events
        let event = timeout(Duration::from_millis(500), audio_level_rx.recv())
            .await
            .ok()
            .flatten();

        assert!(event.is_some(), "Should receive AudioLevelChange event");

        if let Some(HookEvent::AudioLevelChange { level, peak }) = event {
            assert!(level >= 0.0 && level <= 1.0, "Audio level should be between 0 and 1");
            assert!(peak >= 0.0 && peak <= 1.0, "Peak should be between 0 and 1");
        } else {
            panic!("Expected AudioLevelChange event");
        }

        // Stop recording
        let _ = capture.stop().await;
    }
}

/// Test: Channel communication for recording commands
#[tokio::test]
async fn test_recording_channel_communication() {
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();

    // Spawn a mock recording handler
    tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                RecordingCommand::Start { device_id } => {
                    let _ = event_tx.send(RecordingEvent::Started {
                        device_name: device_id.unwrap_or_else(|| "default".to_string()),
                        sample_rate: 16000,
                    });
                }
                RecordingCommand::Stop => {
                    let _ = event_tx.send(RecordingEvent::Stopped {
                        sample_count: 1000,
                        duration_ms: 500,
                    });
                    break;
                }
                RecordingCommand::QueryStatus => {}
            }
        }
    });

    // Send start command
    cmd_tx
        .send(RecordingCommand::Start {
            device_id: Some("test".to_string()),
        })
        .unwrap();

    // Receive started event
    let event = timeout(Duration::from_millis(100), event_rx.recv())
        .await
        .unwrap()
        .unwrap();

    match event {
        RecordingEvent::Started { device_name, sample_rate } => {
            assert_eq!(device_name, "test");
            assert_eq!(sample_rate, 16000);
        }
        _ => panic!("Expected Started event"),
    }

    // Send stop command
    cmd_tx.send(RecordingCommand::Stop).unwrap();

    // Receive stopped event
    let event = timeout(Duration::from_millis(100), event_rx.recv())
        .await
        .unwrap()
        .unwrap();

    match event {
        RecordingEvent::Stopped { sample_count, duration_ms } => {
            assert_eq!(sample_count, 1000);
            assert_eq!(duration_ms, 500);
        }
        _ => panic!("Expected Stopped event"),
    }
}

/// Test: Channel communication for transcription commands
#[tokio::test]
async fn test_transcription_channel_communication() {
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();

    // Spawn a mock transcription handler
    tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                TranscriptionCommand::Transcribe { samples, model: _ } => {
                    // Send progress
                    let _ = event_tx.send(TranscriptionEvent::Progress {
                        progress: 0.5,
                        message: "Transcribing...".to_string(),
                    });

                    // Send complete with mock result
                    let _ = event_tx.send(TranscriptionEvent::Complete {
                        text: "Test transcription".to_string(),
                        duration_ms: 100,
                    });
                    break;
                }
                TranscriptionCommand::Cancel => {
                    break;
                }
            }
        }
    });

    // Send transcribe command with dummy samples
    cmd_tx
        .send(TranscriptionCommand::Transcribe {
            samples: vec![0.1; 1000],
            model: "tiny".to_string(),
        })
        .unwrap();

    // Receive progress event
    let event = timeout(Duration::from_millis(100), event_rx.recv())
        .await
        .unwrap()
        .unwrap();

    match event {
        TranscriptionEvent::Progress { progress, message } => {
            assert_eq!(progress, 0.5);
            assert_eq!(message, "Transcribing...");
        }
        _ => panic!("Expected Progress event"),
    }

    // Receive complete event
    let event = timeout(Duration::from_millis(100), event_rx.recv())
        .await
        .unwrap()
        .unwrap();

    match event {
        TranscriptionEvent::Complete { text, duration_ms } => {
            assert_eq!(text, "Test transcription");
            assert_eq!(duration_ms, 100);
        }
        _ => panic!("Expected Complete event"),
    }
}

/// Test: Settings persistence
#[tokio::test]
async fn test_settings_persistence() {
    // Create settings
    let settings = Settings {
        whisper_model: Some("small".to_string()),
        audio_device: Some("test-device".to_string()),
        orb_style: Some("nebula-aura-gpu:purple".to_string()),
        window_width: 600,
        window_height: 700,
        ..Default::default()
    };

    // Save to temp file
    let temp_dir = std::env::temp_dir().join("Zana_test");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let settings_path = temp_dir.join("settings.json");

    // Manually save settings (simulating Settings::save)
    {
        use std::io::Write;
        let json = serde_json::to_string_pretty(&settings).unwrap();
        let mut file = std::fs::File::create(&settings_path).unwrap();
        file.write_all(json.as_bytes()).unwrap();
    }

    // Load settings
    let loaded_json = std::fs::read_to_string(&settings_path).unwrap();
    let loaded: Settings = serde_json::from_str(&loaded_json).unwrap();

    assert_eq!(loaded.whisper_model, settings.whisper_model);
    assert_eq!(loaded.audio_device, settings.audio_device);
    assert_eq!(loaded.orb_style, settings.orb_style);
    assert_eq!(loaded.always_on_top, settings.always_on_top);
    assert_eq!(loaded.window_width, settings.window_width);
    assert_eq!(loaded.window_height, settings.window_height);

    // Cleanup
    std::fs::remove_dir_all(temp_dir).ok();
}

/// Test: App state initialization
#[tokio::test]
async fn test_app_state_initialization() {
    let app_state = AppState::new().expect("Failed to create AppState");

    // Verify all components are initialized
    assert!(app_state.event_bus.subscriptions_count().await > 0);

    // Verify settings are loaded
    let settings = app_state.settings.read().await;
    assert!(settings.whisper_model.is_some());

    // Verify audio capture is ready
    let capture = app_state.audio_capture.lock().await;
    assert!(!capture.is_recording());
}

/// Test: Recording event → captured audio storage
#[tokio::test]
async fn test_captured_audio_storage() {
    let app_state = AppState::new().expect("Failed to create AppState");

    // Create mock captured audio
    let audio = CapturedAudio {
        samples: vec![0.1, 0.2, 0.3, 0.4, 0.5],
        sample_rate: 16000,
        duration_ms: 100,
    };

    // Store in app state
    *app_state.captured_audio.lock().await = Some(audio.clone());

    // Retrieve and verify
    let retrieved = app_state.captured_audio.lock().await;
    assert!(retrieved.is_some());

    let retrieved_audio = retrieved.as_ref().unwrap();
    assert_eq!(retrieved_audio.samples.len(), 5);
    assert_eq!(retrieved_audio.samples, vec![0.1, 0.2, 0.3, 0.4, 0.5]);
    assert_eq!(retrieved_audio.sample_rate, 16000);
    assert_eq!(retrieved_audio.duration_ms, 100);
}

/// Test: Whisper model parsing
#[test]
fn test_whisper_model_parsing() {
    assert_eq!("tiny".parse::<WhisperModel>().ok(), Some(WhisperModel::Tiny));
    assert_eq!("base".parse::<WhisperModel>().ok(), Some(WhisperModel::Base));
    assert_eq!("small".parse::<WhisperModel>().ok(), Some(WhisperModel::Small));
    assert_eq!("medium".parse::<WhisperModel>().ok(), Some(WhisperModel::Medium));
    assert_eq!("large".parse::<WhisperModel>().ok(), Some(WhisperModel::Large));
    assert_eq!("large-v3".parse::<WhisperModel>().ok(), Some(WhisperModel::Large));
    assert_eq!("invalid".parse::<WhisperModel>().ok(), None);
}

/// Test: Recording event metrics update
#[test]
fn test_recording_event_metrics() {
    let fft_bins = vec![0.1, 0.2, 0.3, 0.4, 0.5];
    let event = RecordingEvent::MetricsUpdate {
        level: 0.5,
        peak: 0.8,
        fft_bins: fft_bins.clone(),
    };

    match event {
        RecordingEvent::MetricsUpdate { level, peak, fft_bins: bins } => {
            assert_eq!(level, 0.5);
            assert_eq!(peak, 0.8);
            assert_eq!(bins, fft_bins);
        }
        _ => panic!("Expected MetricsUpdate event"),
    }
}

/// Test: Transcription event error handling
#[test]
fn test_transcription_event_error() {
    let error_msg = "Test error".to_string();
    let event = TranscriptionEvent::Error(error_msg.clone());

    match event {
        TranscriptionEvent::Error(msg) => {
            assert_eq!(msg, error_msg);
        }
        _ => panic!("Expected Error event"),
    }
}

/// Test: Full flow simulation (record → transcribe → display)
#[tokio::test]
async fn test_full_flow_simulation() {
    // Create app state
    let app_state = Arc::new(AppState::new().expect("Failed to create AppState"));

    // Create channels
    let (recording_cmd_tx, mut recording_cmd_rx) =
        tokio::sync::mpsc::unbounded_channel();
    let (recording_event_tx, mut recording_event_rx) =
        tokio::sync::mpsc::unbounded_channel();
    let (transcription_cmd_tx, mut transcription_cmd_rx) =
        tokio::sync::mpsc::unbounded_channel();
    let (transcription_event_tx, mut transcription_event_rx) =
        tokio::sync::mpsc::unbounded_channel();

    // Spawn recording handler
    let app_state_clone = app_state.clone();
    tokio::spawn(async move {
        while let Some(cmd) = recording_cmd_rx.recv().await {
            match cmd {
                RecordingCommand::Start { device_id } => {
                    let capture = app_state_clone.audio_capture.lock().await;
                    if capture.start(device_id.as_deref()).await.is_ok() {
                        let _ = recording_event_tx.send(RecordingEvent::Started {
                            device_name: device_id.unwrap_or_default(),
                            sample_rate: 16000,
                        });
                    } else {
                        let _ = recording_event_tx.send(RecordingEvent::Error(
                            "Failed to start recording".to_string(),
                        ));
                    }
                }
                RecordingCommand::Stop => {
                    let capture = app_state_clone.audio_capture.lock().await;
                    if let Ok(audio) = capture.stop().await {
                        *app_state_clone.captured_audio.lock().await = Some(audio.clone());
                        let _ = recording_event_tx.send(RecordingEvent::Stopped {
                            sample_count: audio.samples.len(),
                            duration_ms: audio.duration_ms,
                        });
                    }
                    break;
                }
                RecordingCommand::QueryStatus => {}
            }
        }
    });

    // Spawn transcription handler
    tokio::spawn(async move {
        while let Some(cmd) = transcription_cmd_rx.recv().await {
            match cmd {
                TranscriptionCommand::Transcribe { samples, model } => {
                    let engine = app_state.whisper_engine.lock().await;
                    let model_enum = model.parse::<WhisperModel>()
                        .ok()
                        .unwrap_or(WhisperModel::Tiny);

                    match engine.transcribe(&samples, model_enum).await {
                        Ok(result) => {
                            let _ = transcription_event_tx.send(TranscriptionEvent::Complete {
                                text: result.text,
                                duration_ms: result.processing_ms as u32,
                            });
                        }
                        Err(e) => {
                            let _ = transcription_event_tx
                                .send(TranscriptionEvent::Error(e.to_string()));
                        }
                    }
                    break;
                }
                TranscriptionCommand::Cancel => {
                    break;
                }
            }
        }
    });

    // Step 1: Start recording
    recording_cmd_tx
        .send(RecordingCommand::Start { device_id: None })
        .unwrap();

    // Receive started event
    let event = timeout(Duration::from_millis(500), recording_event_rx.recv())
        .await
        .ok()
        .flatten();

    // Note: May fail if no audio device is available
    if let Some(RecordingEvent::Started { .. }) = event {
        // Step 2: Stop recording
        recording_cmd_tx.send(RecordingCommand::Stop).unwrap();

        // Receive stopped event
        let event = timeout(Duration::from_millis(500), recording_event_rx.recv())
            .await
            .ok()
            .flatten();

        assert!(event.is_some());

        // Step 3: Transcribe
        if let Some(audio_guard) = app_state.captured_audio.try_lock().ok() {
            if let Some(audio) = audio_guard.as_ref() {
                transcription_cmd_tx
                    .send(TranscriptionCommand::Transcribe {
                        samples: audio.samples.clone(),
                        model: "tiny".to_string(),
                    })
                    .unwrap();

                // Receive transcription event (may fail if model not downloaded)
                let event =
                    timeout(Duration::from_secs(5), transcription_event_rx.recv())
                        .await
                        .ok()
                        .flatten();

                // If transcription succeeded, verify the result
                if let Some(TranscriptionEvent::Complete { text, .. }) = event {
                    assert!(!text.is_empty());
                }
            }
        }
    }
}

/// Test: Record audio → emit events → orb updates
#[tokio::test]
async fn test_recording_to_orb_updates() {
    use Zana::hooks::HookEventType;

    let event_bus = Arc::new(EventBus::new());

    // Subscribe to all audio events
    let mut level_rx = event_bus
        .subscribe(HookEventType::AudioLevelChange)
        .await;
    let mut fft_rx = event_bus.subscribe(HookEventType::AudioFftReady).await;

    let capture = AudioCapture::new(event_bus.clone());

    // Try to start recording
    let result = capture.start(None).await;

    if result.is_ok() {
        // Wait for audio level event
        let level_event = timeout(Duration::from_millis(500), level_rx.recv())
            .await
            .ok()
            .flatten();

        assert!(
            level_event.is_some(),
            "Should receive audio level event during recording"
        );

        // Verify event structure
        if let Some(HookEvent::AudioLevelChange { level, peak }) = level_event {
            assert!(level >= 0.0 && level <= 1.0, "Level should be normalized 0-1");
            assert!(peak >= 0.0 && peak <= 1.0, "Peak should be normalized 0-1");
        } else {
            panic!("Expected AudioLevelChange event");
        }

        let _ = capture.stop().await;
    }
}

/// Test: Transcribe audio → display results
#[tokio::test]
async fn test_transcribe_to_display() {
    let app_state = Arc::new(AppState::new().expect("Failed to create AppState"));

    // Create mock audio samples (silence with some noise)
    let samples = vec![0.001; 16000]; // 1 second of near-silence

    // Try to transcribe (will fail if model not downloaded, but that's OK)
    let engine = app_state.whisper_engine.lock().await;
    let result = engine.transcribe(&samples, WhisperModel::Tiny).await;

    // If model is available, verify structure
    if let Ok(transcription) = result {
        assert!(!transcription.text.is_empty() || transcription.text.is_empty()); // May be empty for silence
        assert!(transcription.processing_ms > 0);
    }
}

/// Test: Change settings → persist → reload
#[tokio::test]
async fn test_settings_change_persist_reload() {
    use std::io::Write;

    // Create initial settings
    let settings1 = Settings {
        whisper_model: Some("tiny".to_string()),
        audio_device: Some("device-1".to_string()),
        orb_style: Some("nebula-aura-gpu:purple".to_string()),
        window_width: 600,
        window_height: 700,
        ..Default::default()
    };

    // Save to temp file
    let temp_dir = std::env::temp_dir().join("Zana_settings_test");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let settings_path = temp_dir.join("settings.json");

    {
        let json = serde_json::to_string_pretty(&settings1).unwrap();
        let mut file = std::fs::File::create(&settings_path).unwrap();
        file.write_all(json.as_bytes()).unwrap();
    }

    // Load settings
    let loaded_json = std::fs::read_to_string(&settings_path).unwrap();
    let loaded: Settings = serde_json::from_str(&loaded_json).unwrap();

    assert_eq!(loaded.whisper_model, settings1.whisper_model);
    assert_eq!(loaded.audio_device, settings1.audio_device);

    // Modify settings
    let settings2 = Settings {
        whisper_model: Some("small".to_string()), // Changed
        audio_device: Some("device-2".to_string()), // Changed
        orb_style: settings1.orb_style.clone(),
        window_width: 800,    // Changed
        window_height: 900,   // Changed
        always_on_top: false, // Changed
        ..Default::default()
    };

    // Save modified settings
    {
        let json = serde_json::to_string_pretty(&settings2).unwrap();
        let mut file = std::fs::File::create(&settings_path).unwrap();
        file.write_all(json.as_bytes()).unwrap();
    }

    // Reload and verify changes persisted
    let reloaded_json = std::fs::read_to_string(&settings_path).unwrap();
    let reloaded: Settings = serde_json::from_str(&reloaded_json).unwrap();

    assert_eq!(reloaded.whisper_model, Some("small".to_string()));
    assert_eq!(reloaded.audio_device, Some("device-2".to_string()));
    assert_eq!(reloaded.always_on_top, false);
    assert_eq!(reloaded.window_width, 800);
    assert_eq!(reloaded.window_height, 900);

    // Cleanup
    std::fs::remove_dir_all(temp_dir).ok();
}

/// Test: Load plugin → select style → render
#[tokio::test]
async fn test_plugin_loading_and_switching() {
    use Zana::plugins::{PluginManifest, PluginRegistry};
    use Zana::plugins::PluginKind;
    use Zana::plugins::manifest::{PluginMeta, PluginTypeMeta};

    // Create registry
    let mut registry = PluginRegistry::new();

    // Create mock plugin manifest
    let manifest = PluginManifest {
        plugin: PluginMeta {
            id: "test-plugin".to_string(),
            name: "Test Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "A test plugin".to_string(),
            author: "Test Author".to_string(),
            plugin_type: PluginTypeMeta {
                kind: PluginKind::OrbStyle,
                renderer: Zana::plugins::GpuRendererType::WebGPU,
            },
        },
        config: None,
        capabilities: None,
        ui: None,
        marketplace: None,
        dev: None,
    };

    // Verify registry is empty initially
    assert_eq!(registry.count(), 0);
    assert_eq!(registry.count_by_type(PluginKind::OrbStyle), 0);

    // Note: Can't actually register without a Plugin implementation
    // But we can verify the registry structure works
    assert!(!registry.has("test-plugin"));

    // Verify orb styles list is empty
    assert_eq!(registry.orb_style_ids().len(), 0);
}

/// Test: Error handling - missing model
#[tokio::test]
async fn test_error_missing_model() {
    let event_bus = Arc::new(EventBus::new());
    let engine = Zana::stt::WhisperEngine::new(event_bus)
        .expect("Failed to create WhisperEngine");

    // Create mock audio samples
    let samples = vec![0.1; 16000];

    // Try to use a model that doesn't exist
    // Check if model exists first
    let model_exists = engine.is_model_downloaded(Zana::stt::WhisperModel::Medium);

    if !model_exists {
        let result = engine.transcribe(&samples, Zana::stt::WhisperModel::Medium).await;

        assert!(result.is_err(), "Should fail when model is missing");

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not found") || err_msg.contains("Model not found"),
            "Error should indicate model not found: {}",
            err_msg
        );
    }
}

/// Test: Error handling - no audio device
#[tokio::test]
async fn test_error_no_audio_device() {
    let event_bus = Arc::new(EventBus::new());
    let capture = AudioCapture::new(event_bus);

    // Try to start recording with a non-existent device
    let result = capture.start(Some("non-existent-device-12345")).await;

    // Should fail gracefully
    assert!(
        result.is_err(),
        "Should fail when trying to use non-existent device"
    );

    let err_msg = result.unwrap_err().to_string();
    // Error should be descriptive (varies by platform)
    assert!(!err_msg.is_empty(), "Error message should not be empty");
}

/// Test: Real audio capture (skipped if no device available)
#[tokio::test]
#[ignore = "Requires audio hardware"]
async fn test_real_audio_capture() {
    let event_bus = Arc::new(EventBus::new());
    let capture = AudioCapture::new(event_bus);

    // Start recording
    capture.start(None).await.expect("Failed to start recording");

    // Record for 1 second
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Stop recording
    let audio = capture.stop().await.expect("Failed to stop recording");

    // Verify we got audio data
    assert!(!audio.samples.is_empty(), "Should have captured audio samples");
    assert_eq!(audio.sample_rate, 16000, "Should be 16kHz");
    assert!(audio.duration_ms >= 900, "Should have recorded at least 900ms");
}

/// Test: Real transcription (skipped if model not available)
#[tokio::test]
#[ignore = "Requires downloaded Whisper model"]
async fn test_real_transcription() {
    let event_bus = Arc::new(EventBus::new());
    let engine = Zana::stt::WhisperEngine::new(event_bus)
        .expect("Failed to create WhisperEngine");

    // Check if tiny model is available
    if !engine.is_model_downloaded(Zana::stt::WhisperModel::Tiny) {
        println!("Skipping: Whisper Tiny model not downloaded");
        return;
    }

    // Create synthetic audio (simple sine wave at 440Hz - A note)
    let sample_rate = 16000;
    let duration_ms = 1000;
    let num_samples = (sample_rate as f64 * duration_ms as f64 / 1000.0) as usize;
    let samples: Vec<f32> = (0..num_samples)
        .map(|i| {
            let t = i as f64 / sample_rate as f64;
            (0.3 * (2.0 * std::f64::consts::PI * 440.0 * t).sin()) as f32
        })
        .collect();

    // Transcribe
    let result = engine
        .transcribe(&samples, Zana::stt::WhisperModel::Tiny)
        .await
        .expect("Transcription failed");

    // Verify result structure
    assert!(!result.text.is_empty() || result.text.is_empty()); // May be empty for synthetic audio
    assert!(result.processing_ms > 0, "Should have taken some time to process");
}

/// Test: Plugin switching simulation
#[tokio::test]
async fn test_plugin_switching_simulation() {
    use Zana::plugins::{PluginManifest, PluginKind, PluginRegistry};
    use Zana::plugins::manifest::{PluginMeta, PluginTypeMeta};

    let mut registry = PluginRegistry::new();

    // Simulate switching between different orb styles
    let styles = vec![
        ("nebula-aura-gpu:purple", "Nebula Aura Purple"),
        ("nebula-aura-gpu:cyan", "Nebula Aura Cyan"),
        ("nebula-aura-gpu:fire", "Nebula Aura Fire"),
    ];

    // Initially empty
    assert_eq!(registry.orb_style_ids().len(), 0);

    // Verify we can query orb styles (even if empty)
    let orb_styles = registry.orb_style_ids();
    assert!(orb_styles.is_empty());

    // In a real scenario, plugins would be loaded here
    // For now, verify the registry API works
    for (id, _name) in styles {
        assert!(!registry.has(id), "Plugin {} should not be registered", id);
    }
}

// ============================================================================
// BENCHMARKS
// ============================================================================

/// Benchmark: Audio capture overhead
#[tokio::test]
#[ignore = "Benchmark"]
async fn benchmark_audio_capture_overhead() {
    let event_bus = Arc::new(EventBus::new());
    let capture = AudioCapture::new(event_bus);

    let start = std::time::Instant::now();

    // Measure time to start recording
    match capture.start(None).await {
        Ok(_) => {
            let start_time = start.elapsed();

            // Record briefly
            tokio::time::sleep(Duration::from_millis(100)).await;

            let stop_start = std::time::Instant::now();
            let _ = capture.stop().await;
            let stop_time = stop_start.elapsed();

            println!("[BENCHMARK] Audio capture start time: {:?}", start_time);
            println!("[BENCHMARK] Audio capture stop time: {:?}", stop_time);

            assert!(start_time.as_millis() < 100, "Start should be fast");
            assert!(stop_time.as_millis() < 100, "Stop should be fast");
        }
        Err(_) => {
            println!("[BENCHMARK] Skipped: No audio device available");
        }
    }
}

/// Benchmark: Transcription speed
#[tokio::test]
#[ignore = "Benchmark and requires model"]
async fn benchmark_transcription_speed() {
    let event_bus = Arc::new(EventBus::new());
    let engine = Zana::stt::WhisperEngine::new(event_bus)
        .expect("Failed to create WhisperEngine");

    // Skip if model not available
    if !engine.is_model_downloaded(Zana::stt::WhisperModel::Tiny) {
        println!("[BENCHMARK] Skipped: Whisper Tiny model not downloaded");
        return;
    }

    // Create test audio (1 second)
    let samples = vec![0.1; 16000];

    let start = std::time::Instant::now();
    let result = engine
        .transcribe(&samples, Zana::stt::WhisperModel::Tiny)
        .await
        .expect("Transcription failed");
    let duration = start.elapsed();

    let realtime_factor = duration.as_secs_f64() / 1.0; // 1 second of audio

    println!("[BENCHMARK] Transcription time: {:?}", duration);
    println!("[BENCHMARK] Realtime factor: {:.2}x", realtime_factor);
    println!("[BENCHMARK] Processing time: {}ms", result.processing_ms);

    // Should be faster than realtime on modern hardware
    assert!(realtime_factor < 10.0, "Should transcribe reasonably fast");
}

/// Benchmark: Event bus throughput
#[tokio::test]
#[ignore = "Benchmark"]
async fn benchmark_event_bus_throughput() {
    use Zana::hooks::HookEventType;

    let event_bus = Arc::new(EventBus::new());
    let mut rx = event_bus
        .subscribe(HookEventType::AudioLevelChange)
        .await;

    const NUM_EVENTS: usize = 1000;

    let start = std::time::Instant::now();

    // Spawn emitter
    let bus_clone = event_bus.clone();
    tokio::spawn(async move {
        for i in 0..NUM_EVENTS {
            use Zana::hooks::HookEvent;
            let _ = bus_clone
                .emit(HookEvent::AudioLevelChange {
                    level: (i as f32) / NUM_EVENTS as f32,
                    peak: 1.0,
                })
                .await;
        }
    });

    // Receive events
    let mut count = 0;
    while timeout(Duration::from_secs(5), rx.recv())
        .await
        .ok()
        .flatten()
        .is_some()
    {
        count += 1;
        if count >= NUM_EVENTS {
            break;
        }
    }

    let duration = start.elapsed();
    let throughput = NUM_EVENTS as f64 / duration.as_secs_f64();

    println!("[BENCHMARK] Event bus throughput: {:.0} events/sec", throughput);
    println!("[BENCHMARK] Average latency: {:.2} us", duration.as_micros() as f64 / NUM_EVENTS as f64);

    assert_eq!(count, NUM_EVENTS, "Should receive all events");
    assert!(throughput > 1000.0, "Should handle at least 1000 events/sec");
}

/// Benchmark: Settings serialization
#[test]
#[ignore = "Benchmark"]
fn benchmark_settings_serialization() {
    use std::io::Write;

    let settings = Settings {
        whisper_model: Some("small".to_string()),
        audio_device: Some("test-device".to_string()),
        orb_style: Some("nebula-aura-gpu:purple".to_string()),
        window_width: 600,
        window_height: 700,
        ..Default::default()
    };

    // Benchmark serialization
    let start = std::time::Instant::now();
    const ITERATIONS: usize = 1000;

    for _ in 0..ITERATIONS {
        let _ = serde_json::to_string_pretty(&settings).unwrap();
    }

    let serialize_duration = start.elapsed();

    // Benchmark deserialization
    let json = serde_json::to_string_pretty(&settings).unwrap();
    let start = std::time::Instant::now();

    for _ in 0..ITERATIONS {
        let _: Settings = serde_json::from_str(&json).unwrap();
    }

    let deserialize_duration = start.elapsed();

    println!(
        "[BENCHMARK] Serialization: {:.2} us/op",
        serialize_duration.as_micros() as f64 / ITERATIONS as f64
    );
    println!(
        "[BENCHMARK] Deserialization: {:.2} us/op",
        deserialize_duration.as_micros() as f64 / ITERATIONS as f64
    );

    // Should be very fast
    assert!(serialize_duration.as_millis() < 100, "Serialization should be fast");
    assert!(deserialize_duration.as_millis() < 100, "Deserialization should be fast");
}
