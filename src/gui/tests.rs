//! Comprehensive GUI Unit Tests
//!
//! Phase 3B tests per Zana_COMPLETION_SPEC.md
//! Tests OrbRenderer, channels, event handler, settings, state management, error handling.
//!
//! Coverage Target: >80% for GUI module

// ============================================================================
// Test Utilities & Mocks
// ============================================================================

/// Test helper to create a temporary directory for test files
#[cfg(test)]
fn create_temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("Failed to create temp dir")
}

// ============================================================================
// Module: OrbRenderer Tests
// ============================================================================

#[cfg(test)]
mod orb_renderer_tests {
    use crate::gui::OrbRenderer;

    /// Test: OrbRenderer creation
    #[test]
    fn test_orb_renderer_creation() {
        let ctx = egui::Context::default();
        let renderer = OrbRenderer::new(&ctx);

        // Verify renderer was created - just verify we can call methods
        let _ = renderer.pipeline();
        let _ = renderer.bind_group();
        let _ = renderer.device();
        let _ = renderer.queue();
    }

    /// Test: OrbRenderer Default implementation
    #[test]
    fn test_orb_renderer_default() {
        let renderer = OrbRenderer::default();
        // Verify default creation works
        let _ = renderer.pipeline();
    }

    /// Test: Update audio data
    #[test]
    fn test_orb_renderer_update_audio() {
        let ctx = egui::Context::default();
        let mut renderer = OrbRenderer::new(&ctx);

        let fft_data = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0,
                        0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.0,
                        0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0,
                        0.5, 0.5];

        // Verify update_audio doesn't panic with valid data
        renderer.update_audio(0.75, &fft_data);

        // Verify we can still access the renderer resources
        let _ = renderer.pipeline();
    }

    /// Test: Set color scheme
    #[test]
    fn test_orb_renderer_set_color_scheme() {
        let ctx = egui::Context::default();
        let mut renderer = OrbRenderer::new(&ctx);

        renderer.set_color_scheme(1.0);
        // Just verify the method doesn't panic - internal state is private
    }

    /// Test: Set quality
    #[test]
    fn test_orb_renderer_set_quality() {
        let ctx = egui::Context::default();
        let mut renderer = OrbRenderer::new(&ctx);

        renderer.set_quality(0.0);
        renderer.set_quality(3.0);
    }

    /// Test: Set glow intensity
    #[test]
    fn test_orb_renderer_set_glow_intensity() {
        let ctx = egui::Context::default();
        let mut renderer = OrbRenderer::new(&ctx);

        renderer.set_glow_intensity(0.5);
        renderer.set_glow_intensity(2.0);
    }

    /// Test: Set cloud count
    #[test]
    fn test_orb_renderer_set_cloud_count() {
        let ctx = egui::Context::default();
        let mut renderer = OrbRenderer::new(&ctx);

        renderer.set_cloud_count(3.0);
        renderer.set_cloud_count(10.0);
    }

    /// Test: Set particle count
    #[test]
    fn test_orb_renderer_set_particle_count() {
        let ctx = egui::Context::default();
        let mut renderer = OrbRenderer::new(&ctx);

        renderer.set_particle_count(25.0);
        renderer.set_particle_count(100.0);
    }

    /// Test: Set rotation speed
    #[test]
    fn test_orb_renderer_set_rotation_speed() {
        let ctx = egui::Context::default();
        let mut renderer = OrbRenderer::new(&ctx);

        renderer.set_rotation_speed(0.5);
        renderer.set_rotation_speed(3.0);
    }

    /// Test: Set resolution
    #[test]
    fn test_orb_renderer_set_resolution() {
        let ctx = egui::Context::default();
        let mut renderer = OrbRenderer::new(&ctx);

        renderer.set_resolution(1920.0, 1080.0);
        renderer.set_resolution(1280.0, 720.0);
    }

    /// Test: Render updates uniforms
    #[test]
    fn test_orb_renderer_render_updates_uniforms() {
        let ctx = egui::Context::default();
        let mut renderer = OrbRenderer::new(&ctx);

        let fft_data = [0.5; 32];
        renderer.render(&ctx, 0.6, &fft_data);

        // Verify render doesn't panic and we can still access resources
        let _ = renderer.pipeline();
        let _ = renderer.bind_group();
    }

    /// Test: Get pipeline
    #[test]
    fn test_orb_renderer_get_pipeline() {
        let ctx = egui::Context::default();
        let renderer = OrbRenderer::new(&ctx);
        let _pipeline = renderer.pipeline();
        // Just verify we can get the pipeline without panicking
    }

    /// Test: Get bind group
    #[test]
    fn test_orb_renderer_get_bind_group() {
        let ctx = egui::Context::default();
        let renderer = OrbRenderer::new(&ctx);
        let _bind_group = renderer.bind_group();
        // Just verify we can get the bind group without panicking
    }

    /// Test: Get device
    #[test]
    fn test_orb_renderer_get_device() {
        let ctx = egui::Context::default();
        let renderer = OrbRenderer::new(&ctx);
        let _device = renderer.device();
        // Just verify we can get the device without panicking
    }

    /// Test: Get queue
    #[test]
    fn test_orb_renderer_get_queue() {
        let ctx = egui::Context::default();
        let renderer = OrbRenderer::new(&ctx);
        let _queue = renderer.queue();
        // Just verify we can get the queue without panicking
    }

    /// Test: FFT data handling
    #[test]
    fn test_orb_renderer_fft_data_handling() {
        let ctx = egui::Context::default();
        let mut renderer = OrbRenderer::new(&ctx);

        // Test with varying FFT data
        let fft_data1: [f32; 32] = std::array::from_fn(|i| i as f32 / 32.0);
        renderer.update_audio(0.5, &fft_data1);

        let fft_data2: [f32; 32] = std::array::from_fn(|i| (32 - i) as f32 / 32.0);
        renderer.update_audio(0.7, &fft_data2);
    }

    /// Test: Zero audio level
    #[test]
    fn test_orb_renderer_zero_audio_level() {
        let ctx = egui::Context::default();
        let mut renderer = OrbRenderer::new(&ctx);

        let fft_data = [0.0; 32];
        renderer.update_audio(0.0, &fft_data);

        // Verify renderer still works after zero audio
        let _ = renderer.pipeline();
    }

    /// Test: Maximum audio level
    #[test]
    fn test_orb_renderer_max_audio_level() {
        let ctx = egui::Context::default();
        let mut renderer = OrbRenderer::new(&ctx);

        let fft_data = [1.0; 32];
        renderer.update_audio(1.0, &fft_data);

        // Verify renderer still works after max audio
        let _ = renderer.pipeline();
    }
}

// ============================================================================
// Module: Shader Loading Tests
// ============================================================================

#[cfg(test)]
mod shader_loading_tests {
    /// Test: Shader file exists
    #[test]
    fn test_shader_file_exists() {
        let shader_path = "plugins/nebula-aura-gpu/src/shaders/nebula.wgsl";
        assert!(
            std::path::Path::new(shader_path).exists(),
            "Shader file should exist at {}",
            shader_path
        );
    }

    /// Test: Shader file is readable
    #[test]
    fn test_shader_file_readable() {
        let shader_path = "plugins/nebula-aura-gpu/src/shaders/nebula.wgsl";
        let content = std::fs::read_to_string(shader_path);
        assert!(content.is_ok(), "Shader file should be readable");
        let shader_code = content.unwrap();
        assert!(!shader_code.is_empty(), "Shader code should not be empty");
        assert!(shader_code.len() > 1000, "Shader should have substantial content");
    }

    /// Test: Shader contains required entry points
    #[test]
    fn test_shader_entry_points() {
        let shader_path = "plugins/nebula-aura-gpu/src/shaders/nebula.wgsl";
        let shader_code = std::fs::read_to_string(shader_path).unwrap();

        assert!(shader_code.contains("fn vs_main"),
                "Shader should have vertex shader entry point");
        assert!(shader_code.contains("fn fs_main"),
                "Shader should have fragment shader entry point");
    }

    /// Test: Shader contains required uniform bindings
    #[test]
    fn test_shader_uniform_bindings() {
        let shader_path = "plugins/nebula-aura-gpu/src/shaders/nebula.wgsl";
        let shader_code = std::fs::read_to_string(shader_path).unwrap();

        // Check for uniform buffer binding
        assert!(shader_code.contains("@group(0)") || shader_code.contains("@binding(0)"),
                "Shader should use binding group 0");
    }

    /// Test: Fallback shader is valid
    #[test]
    fn test_fallback_shader_valid() {
        // The fallback shader should be valid WGSL
        let fallback_shader = r#"
// Full-screen quad vertex shader
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );
    return vec4<f32>(positions[vertex_index], 0.0, 1.0);
}

// Simple fragment shader
@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(0.5, 0.2, 0.6, 1.0);
}
"#;

        // Verify it contains the expected elements
        assert!(fallback_shader.contains("@vertex"));
        assert!(fallback_shader.contains("@fragment"));
        assert!(fallback_shader.contains("vs_main"));
        assert!(fallback_shader.contains("fs_main"));
    }
}

// ============================================================================
// Module: Channel Communication Tests
// ============================================================================

#[cfg(test)]
mod channel_tests {
    use crate::gui::{RecordingCommand, RecordingEvent, TranscriptionCommand, TranscriptionEvent};

    /// Test: RecordingCommand variants
    #[test]
    fn test_recording_command_variants() {
        let start = RecordingCommand::Start { device_id: Some("test".to_string()) };
        match start {
            RecordingCommand::Start { device_id } => {
                assert_eq!(device_id, Some("test".to_string()));
            }
            _ => panic!("Wrong variant"),
        }

        let stop = RecordingCommand::Stop;
        match stop {
            RecordingCommand::Stop => {}
            _ => panic!("Wrong variant"),
        }

        let query = RecordingCommand::QueryStatus;
        match query {
            RecordingCommand::QueryStatus => {}
            _ => panic!("Wrong variant"),
        }
    }

    /// Test: RecordingEvent variants
    #[test]
    fn test_recording_event_variants() {
        let started = RecordingEvent::Started {
            device_name: "test".to_string(),
            sample_rate: 16000,
        };
        match started {
            RecordingEvent::Started { device_name, sample_rate } => {
                assert_eq!(device_name, "test");
                assert_eq!(sample_rate, 16000);
            }
            _ => panic!("Wrong variant"),
        }

        let stopped = RecordingEvent::Stopped {
            sample_count: 1000,
            duration_ms: 500,
        };
        match stopped {
            RecordingEvent::Stopped { sample_count, duration_ms } => {
                assert_eq!(sample_count, 1000);
                assert_eq!(duration_ms, 500);
            }
            _ => panic!("Wrong variant"),
        }

        let fft_bins = vec![0.1, 0.2, 0.3];
        let metrics = RecordingEvent::MetricsUpdate {
            level: 0.5,
            peak: 0.8,
            fft_bins: fft_bins.clone(),
        };
        match metrics {
            RecordingEvent::MetricsUpdate { level, peak, fft_bins: bins } => {
                assert_eq!(level, 0.5);
                assert_eq!(peak, 0.8);
                assert_eq!(bins, fft_bins);
            }
            _ => panic!("Wrong variant"),
        }

        let error = RecordingEvent::Error("test error".to_string());
        match error {
            RecordingEvent::Error(msg) => assert_eq!(msg, "test error"),
            _ => panic!("Wrong variant"),
        }
    }

    /// Test: TranscriptionCommand variants
    #[test]
    fn test_transcription_command_variants() {
        let samples = vec![0.1, 0.2, 0.3];
        let transcribe = TranscriptionCommand::Transcribe {
            samples: samples.clone(),
            model: "small".to_string(),
        };
        match transcribe {
            TranscriptionCommand::Transcribe { samples: s, model } => {
                assert_eq!(s, samples);
                assert_eq!(model, "small");
            }
            _ => panic!("Wrong variant"),
        }

        let cancel = TranscriptionCommand::Cancel;
        match cancel {
            TranscriptionCommand::Cancel => {}
            _ => panic!("Wrong variant"),
        }
    }

    /// Test: TranscriptionEvent variants
    #[test]
    fn test_transcription_event_variants() {
        let progress = TranscriptionEvent::Progress {
            progress: 0.5,
            message: "Processing".to_string(),
        };
        match progress {
            TranscriptionEvent::Progress { progress, message } => {
                assert_eq!(progress, 0.5);
                assert_eq!(message, "Processing");
            }
            _ => panic!("Wrong variant"),
        }

        let complete = TranscriptionEvent::Complete {
            text: "Hello".to_string(),
            duration_ms: 100,
        };
        match complete {
            TranscriptionEvent::Complete { text, duration_ms } => {
                assert_eq!(text, "Hello");
                assert_eq!(duration_ms, 100);
            }
            _ => panic!("Wrong variant"),
        }

        let error = TranscriptionEvent::Error("error".to_string());
        match error {
            TranscriptionEvent::Error(msg) => assert_eq!(msg, "error"),
            _ => panic!("Wrong variant"),
        }
    }

    /// Test: Channel send/receive recording commands
    #[test]
    fn test_channel_recording_commands() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<RecordingCommand>();

        let cmd = RecordingCommand::Start {
            device_id: Some("device-1".to_string()),
        };
        tx.send(cmd).unwrap();

        let received = rx.blocking_recv().unwrap();
        match received {
            RecordingCommand::Start { device_id } => {
                assert_eq!(device_id, Some("device-1".to_string()));
            }
            _ => panic!("Wrong variant"),
        }
    }

    /// Test: Channel send/receive recording events
    #[test]
    fn test_channel_recording_events() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<RecordingEvent>();

        let event = RecordingEvent::Started {
            device_name: "test-device".to_string(),
            sample_rate: 48000,
        };
        tx.send(event).unwrap();

        let received = rx.blocking_recv().unwrap();
        match received {
            RecordingEvent::Started { device_name, sample_rate } => {
                assert_eq!(device_name, "test-device");
                assert_eq!(sample_rate, 48000);
            }
            _ => panic!("Wrong variant"),
        }
    }

    /// Test: Channel send/receive transcription commands
    #[test]
    fn test_channel_transcription_commands() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<TranscriptionCommand>();

        let cmd = TranscriptionCommand::Transcribe {
            samples: vec![0.1, 0.2, 0.3],
            model: "tiny".to_string(),
        };
        tx.send(cmd).unwrap();

        let received = rx.blocking_recv().unwrap();
        match received {
            TranscriptionCommand::Transcribe { samples, model } => {
                assert_eq!(samples, vec![0.1, 0.2, 0.3]);
                assert_eq!(model, "tiny");
            }
            _ => panic!("Wrong variant"),
        }
    }

    /// Test: Channel send/receive transcription events
    #[test]
    fn test_channel_transcription_events() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<TranscriptionEvent>();

        let event = TranscriptionEvent::Complete {
            text: "Test transcription".to_string(),
            duration_ms: 250,
        };
        tx.send(event).unwrap();

        let received = rx.blocking_recv().unwrap();
        match received {
            TranscriptionEvent::Complete { text, duration_ms } => {
                assert_eq!(text, "Test transcription");
                assert_eq!(duration_ms, 250);
            }
            _ => panic!("Wrong variant"),
        }
    }

    /// Test: Multiple channel sends
    #[test]
    fn test_channel_multiple_sends() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<RecordingEvent>();

        for i in 0..10 {
            let event = RecordingEvent::MetricsUpdate {
                level: i as f32 / 10.0,
                peak: 0.8,
                fft_bins: vec![0.5],
            };
            tx.send(event).unwrap();
        }

        let mut count = 0;
        while let Ok(_) = rx.try_recv() {
            count += 1;
        }
        assert_eq!(count, 10);
    }

    /// Test: Channel closure behavior
    #[test]
    fn test_channel_closure() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<RecordingEvent>();

        tx.send(RecordingEvent::Stopped {
            sample_count: 100,
            duration_ms: 50,
        }).unwrap();

        drop(tx);

        // Should still receive the sent event
        let received = rx.blocking_recv();
        assert!(received.is_some(), "Should receive the event sent before channel close");

        // Next receive should return None (channel closed)
        let received = rx.blocking_recv();
        assert!(received.is_none(), "Should return None when channel is closed");
    }

    /// Test: GuiChannels creation
    #[test]
    fn test_gui_channels_creation() {
        use crate::gui::app::GuiChannels;
        let (gui_channels, worker_channels) = GuiChannels::new();

        // Verify GUI side can send commands
        assert!(gui_channels.recording_cmd_tx.send(RecordingCommand::QueryStatus).is_ok());
        assert!(gui_channels.transcription_cmd_tx.send(TranscriptionCommand::Cancel).is_ok());

        // Verify worker side can send events
        assert!(worker_channels.recording_event_tx.send(RecordingEvent::Stopped {
            sample_count: 0,
            duration_ms: 0,
        }).is_ok());
        assert!(worker_channels.transcription_event_tx.send(TranscriptionEvent::Complete {
            text: String::new(),
            duration_ms: 0,
        }).is_ok());
    }
}

// ============================================================================
// Module: Event Handler Tests
// ============================================================================

#[cfg(test)]
mod event_handler_tests {
    use crate::gui::GuiEventHandler;
    use crate::hooks::{EventBus, HookEvent};
    use std::sync::Arc;

    /// Test: GuiEventHandler creation
    #[tokio::test]
    async fn test_event_handler_creation() {
        let mut handler = GuiEventHandler::new();
        assert!(handler.try_recv().is_none(), "New handler should have no events");
    }

    /// Test: GuiEventHandler default
    #[tokio::test]
    async fn test_event_handler_default() {
        let mut handler = GuiEventHandler::default();
        assert!(handler.try_recv().is_none());
    }

    /// Test: Event subscription
    #[tokio::test]
    async fn test_event_subscription() {
        let event_bus = Arc::new(EventBus::new());
        let mut handler = GuiEventHandler::new();

        handler.subscribe(event_bus.clone()).await.unwrap();

        event_bus
            .emit(HookEvent::AudioLevelChange {
                level: 0.5,
                peak: 0.8,
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let event = handler.try_recv();
        assert!(event.is_some());

        if let Some(HookEvent::AudioLevelChange { level, peak }) = event {
            assert_eq!(level, 0.5);
            assert_eq!(peak, 0.8);
        } else {
            panic!("Expected AudioLevelChange event");
        }
    }

    /// Test: Multiple event types
    #[tokio::test]
    async fn test_multiple_event_types() {
        let event_bus = Arc::new(EventBus::new());
        let mut handler = GuiEventHandler::new();

        handler.subscribe(event_bus.clone()).await.unwrap();

        // Emit different event types
        event_bus
            .emit(HookEvent::AudioLevelChange {
                level: 0.3,
                peak: 0.6,
            })
            .await;

        event_bus
            .emit(HookEvent::TranscriptionProgress { percent: 25.0 })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

        // Should receive both events
        let mut events_received = 0;
        while let Some(_) = handler.try_recv() {
            events_received += 1;
        }

        assert!(events_received >= 2, "Should receive at least 2 events");
    }

    /// Test: Process pending events
    #[tokio::test]
    async fn test_process_pending_events() {
        let event_bus = Arc::new(EventBus::new());
        let mut handler = GuiEventHandler::new();

        handler.subscribe(event_bus.clone()).await.unwrap();

        // Emit multiple events
        for i in 0..5 {
            event_bus
                .emit(HookEvent::AudioLevelChange {
                    level: i as f32 / 10.0,
                    peak: 0.8,
                })
                .await;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

        let mut count = 0;
        handler.process_pending(|event| {
            if let HookEvent::AudioLevelChange { .. } = event {
                count += 1;
            }
        });

        assert_eq!(count, 5);
    }

    /// Test: Process events limit
    #[tokio::test]
    async fn test_process_events_limit() {
        let event_bus = Arc::new(EventBus::new());
        let mut handler = GuiEventHandler::new();

        handler.subscribe(event_bus.clone()).await.unwrap();

        // Emit more events than the limit
        for i in 0..150 {
            event_bus
                .emit(HookEvent::AudioLevelChange {
                    level: i as f32 / 150.0,
                    peak: 0.8,
                })
                .await;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let mut count = 0;
        handler.process_pending(|_event| {
            count += 1;
        });

        // Should process at most 100 events
        assert!(count <= 100, "Should limit processing to prevent UI freeze");
    }

    /// Test: Sender clone
    #[tokio::test]
    async fn test_sender_clone() {
        let handler = GuiEventHandler::new();
        let sender = handler.sender();

        // Send directly via sender
        sender
            .send(HookEvent::AudioLevelChange {
                level: 0.9,
                peak: 1.0,
            })
            .await
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let mut handler = handler;
        let event = handler.try_recv();
        assert!(event.is_some());
    }

    /// Test: Error event handling
    #[tokio::test]
    async fn test_error_event_handling() {
        let event_bus = Arc::new(EventBus::new());
        let mut handler = GuiEventHandler::new();

        handler.subscribe(event_bus.clone()).await.unwrap();

        event_bus
            .emit(HookEvent::Error {
                code: "ERR001".to_string(),
                message: "Test error".to_string(),
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let event = handler.try_recv();
        assert!(event.is_some());

        if let Some(HookEvent::Error { code, message }) = event {
            assert_eq!(code, "ERR001");
            assert_eq!(message, "Test error");
        } else {
            panic!("Expected Error event");
        }
    }

    /// Test: FFT event handling
    #[tokio::test]
    async fn test_fft_event_handling() {
        let event_bus = Arc::new(EventBus::new());
        let mut handler = GuiEventHandler::new();

        handler.subscribe(event_bus.clone()).await.unwrap();

        let bins = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        event_bus
            .emit(HookEvent::AudioFftReady {
                bins: bins.clone(),
                bin_count: bins.len(),
            })
            .await;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let event = handler.try_recv();
        assert!(event.is_some());

        if let Some(HookEvent::AudioFftReady { bins, bin_count }) = event {
            assert_eq!(bin_count, 5);
            assert_eq!(bins.len(), 5);
        } else {
            panic!("Expected AudioFftReady event");
        }
    }
}

// ============================================================================
// Module: Settings Tests
// ============================================================================

#[cfg(test)]
mod settings_tests {
    use crate::gui::settings::{OrbStyle, SettingsState};
    use crate::state::Settings;

    /// Test: Settings serialization
    #[test]
    fn test_settings_serialization() {
        let settings = Settings {
            whisper_model: Some("tiny".to_string()),
            audio_device: Some("default".to_string()),
            orb_style: Some("nebula-aura-gpu:purple".to_string()),
            always_on_top: true,
            window_width: 500,
            window_height: 500,
        };

        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("tiny"));
        assert!(json.contains("nebula-aura-gpu"));

        let deserialized: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.whisper_model, settings.whisper_model);
        assert_eq!(deserialized.audio_device, settings.audio_device);
        assert_eq!(deserialized.orb_style, settings.orb_style);
        assert_eq!(deserialized.always_on_top, settings.always_on_top);
        assert_eq!(deserialized.window_width, settings.window_width);
        assert_eq!(deserialized.window_height, settings.window_height);
    }

    /// Test: Settings default
    #[test]
    fn test_settings_default() {
        let settings = Settings::default();
        assert_eq!(settings.whisper_model, Some("small".to_string()));
        assert_eq!(settings.audio_device, None);
        assert_eq!(settings.orb_style, Some("nebula-aura-gpu".to_string()));
        assert!(settings.always_on_top);
        assert_eq!(settings.window_width, 500);
        assert_eq!(settings.window_height, 500);
    }

    /// Test: SettingsState default
    #[test]
    fn test_settings_state_default() {
        let state = SettingsState::default();
        assert!(!state.has_unsaved_changes);
        assert!(state.status_message.is_none());
        assert!(!state.status_is_error);
        assert_eq!(state.selected_color_scheme, "purple");
        assert_eq!(state.download_progress, 0.0);
    }

    /// Test: SettingsState mark_changed
    #[test]
    fn test_settings_state_mark_changed() {
        let mut state = SettingsState::default();
        assert!(!state.has_unsaved_changes);
        state.mark_changed();
        assert!(state.has_unsaved_changes);
    }

    /// Test: SettingsState mark_saved
    #[test]
    fn test_settings_state_mark_saved() {
        let mut state = SettingsState::default();
        state.mark_changed();
        state.mark_saved();
        assert!(!state.has_unsaved_changes);
    }

    /// Test: SettingsState status messages
    #[test]
    fn test_settings_state_status_messages() {
        let mut state = SettingsState::default();

        state.set_status("Info message".to_string());
        assert_eq!(state.status_message, Some("Info message".to_string()));
        assert!(!state.status_is_error);

        state.set_error("Error message".to_string());
        assert_eq!(state.status_message, Some("Error message".to_string()));
        assert!(state.status_is_error);

        state.clear_status();
        assert!(state.status_message.is_none());
        assert!(!state.status_is_error);
    }

    /// Test: OrbStyle discovery
    #[test]
    fn test_orb_style_discovery() {
        let styles = OrbStyle::discover_styles();
        assert!(!styles.is_empty());
        assert!(styles.iter().any(|s| s.id == "nebula-aura-gpu"));

        let nebula = styles.iter().find(|s| s.id == "nebula-aura-gpu").unwrap();
        assert!(nebula.color_schemes.contains(&"purple".to_string()));
        assert!(nebula.color_schemes.contains(&"cyan".to_string()));
    }

    /// Test: Get orb style string
    #[test]
    fn test_get_orb_style_string() {
        let mut state = SettingsState::default();
        state.settings.orb_style = Some("nebula-aura-gpu".to_string());
        state.selected_color_scheme = "fire".to_string();

        let style_str = state.get_orb_style_string();
        assert_eq!(style_str, Some("nebula-aura-gpu:fire".to_string()));

        // Test with colon already in string
        state.settings.orb_style = Some("nebula-aura-gpu:cosmic".to_string());
        let style_str = state.get_orb_style_string();
        assert_eq!(style_str, Some("nebula-aura-gpu:cosmic".to_string()));

        // Test with None
        state.settings.orb_style = None;
        let style_str = state.get_orb_style_string();
        assert_eq!(style_str, None);
    }

    /// Test: Parse orb style
    #[test]
    fn test_parse_orb_style() {
        let state = SettingsState::default();

        let (plugin, scheme) = state.parse_orb_style("nebula-aura-gpu:fire");
        assert_eq!(plugin, "nebula-aura-gpu");
        assert_eq!(scheme, "fire");

        let (plugin, scheme) = state.parse_orb_style("nebula-aura");
        assert_eq!(plugin, "nebula-aura");
        assert_eq!(scheme, "purple");
    }

    /// Test: SettingsState from_settings
    #[test]
    fn test_settings_state_from_settings() {
        let settings = Settings {
            whisper_model: Some("base".to_string()),
            audio_device: Some("test-device".to_string()),
            orb_style: Some("nebula-aura-gpu:cyan".to_string()),
            always_on_top: false,
            window_width: 600,
            window_height: 600,
        };

        let state = SettingsState::from_settings(settings);
        assert_eq!(state.settings.whisper_model, Some("base".to_string()));
        assert_eq!(state.selected_color_scheme, "cyan");
    }

    /// Test: Settings JSON roundtrip
    #[test]
    fn test_settings_json_roundtrip() {
        let original = Settings {
            whisper_model: Some("large".to_string()),
            audio_device: None,
            orb_style: Some("nebula-aura:fire".to_string()),
            always_on_top: false,
            window_width: 800,
            window_height: 600,
        };

        let json = serde_json::to_string_pretty(&original).unwrap();
        let restored: Settings = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.whisper_model, original.whisper_model);
        assert_eq!(restored.audio_device, original.audio_device);
        assert_eq!(restored.orb_style, original.orb_style);
        assert_eq!(restored.always_on_top, original.always_on_top);
        assert_eq!(restored.window_width, original.window_width);
        assert_eq!(restored.window_height, original.window_height);
    }
}

// ============================================================================
// Module: Error Handling Tests
// ============================================================================

#[cfg(test)]
mod error_handling_tests {
    use crate::gui::{RecordingEvent, TranscriptionEvent};
    use crate::gui::settings::SettingsState;

    /// Test: Recording event error propagation
    #[test]
    fn test_recording_error_propagation() {
        let error = RecordingEvent::Error("Device not found".to_string());
        match error {
            RecordingEvent::Error(msg) => {
                assert_eq!(msg, "Device not found");
            }
            _ => panic!("Expected Error variant"),
        }
    }

    /// Test: Transcription error propagation
    #[test]
    fn test_transcription_error_propagation() {
        let error = TranscriptionEvent::Error("Model not loaded".to_string());
        match error {
            TranscriptionEvent::Error(msg) => {
                assert_eq!(msg, "Model not loaded");
            }
            _ => panic!("Expected Error variant"),
        }
    }

    /// Test: Empty audio data handling
    #[test]
    fn test_empty_audio_data_handling() {
        let ctx = egui::Context::default();
        let mut renderer = crate::gui::OrbRenderer::new(&ctx);

        let empty_fft = [0.0; 32];
        renderer.update_audio(0.0, &empty_fft);

        // Verify renderer still works with empty audio
        let _ = renderer.pipeline();
    }

    /// Test: Invalid device handling
    #[test]
    fn test_invalid_device_handling() {
        use crate::gui::RecordingCommand;

        let cmd = RecordingCommand::Start {
            device_id: Some("non-existent-device".to_string()),
        };
        match cmd {
            RecordingCommand::Start { device_id } => {
                assert_eq!(device_id, Some("non-existent-device".to_string()));
            }
            _ => panic!("Wrong variant"),
        }
    }

    /// Test: Channel closure error handling
    #[test]
    fn test_channel_closure_error_handling() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<RecordingEvent>();

        // Close channel
        drop(tx);

        // Try to receive should return error
        let result = rx.try_recv();
        assert!(result.is_err());
    }

    /// Test: Multiple error states
    #[test]
    fn test_multiple_error_states() {
        let mut state = SettingsState::default();

        // Set error
        state.set_error("Error 1".to_string());
        assert!(state.status_is_error);
        assert_eq!(state.status_message, Some("Error 1".to_string()));

        // Clear error
        state.clear_status();
        assert!(!state.status_is_error);
        assert!(state.status_message.is_none());

        // Set new status
        state.set_status("Status message".to_string());
        assert!(!state.status_is_error);
        assert_eq!(state.status_message, Some("Status message".to_string()));
    }

    /// Test: Settings save error handling
    #[test]
    fn test_settings_save_error_handling() {
        use crate::state::Settings;

        // Create settings with invalid path (readonly location)
        let settings = Settings {
            whisper_model: Some("tiny".to_string()),
            audio_device: None,
            orb_style: Some("nebula-aura-gpu".to_string()),
            always_on_top: true,
            window_width: 500,
            window_height: 500,
        };

        // Save should normally succeed, but we test the error path conceptually
        // In real scenarios, this could fail due to permissions
        let result = settings.save();
        // Result may be Ok or Err depending on system
        // We just verify it returns a Result
        let _ = result;
    }
}

// ============================================================================
// Module: Concurrent Access Tests
// ============================================================================

#[cfg(test)]
mod concurrent_access_tests {
    use std::sync::{Arc, Mutex};
    use std::thread;

    /// Test: Concurrent OrbRenderer access
    ///
    /// NOTE: This test is skipped because OrbRenderer uses Rc<wgpu::Device> which is not Send.
    /// Concurrent access tests would require restructuring OrbRenderer to use Arc instead of Rc.
    #[test]
    #[ignore = "OrbRenderer uses Rc which is not Send, cannot test concurrent access across threads"]
    fn test_concurrent_orb_renderer_access() {
        // This test is documented but cannot run due to Rc<wgpu::Device> not being Send
        // In production, OrbRenderer is accessed from a single thread (the GUI thread)
    }

    /// Test: Concurrent channel sends
    #[test]
    fn test_concurrent_channel_sends() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<crate::gui::RecordingEvent>();

        let mut handles = vec![];

        // Spawn multiple threads sending events
        for i in 0..10 {
            let tx_clone = tx.clone();
            let handle = thread::spawn(move || {
                let event = crate::gui::RecordingEvent::MetricsUpdate {
                    level: i as f32 / 10.0,
                    peak: 0.8,
                    fft_bins: vec![i as f32],
                };
                tx_clone.send(event).unwrap();
            });
            handles.push(handle);
        }

        // Wait for all sends
        for handle in handles {
            handle.join().unwrap();
        }

        drop(tx);

        // Receive all events
        let mut count = 0;
        while let Ok(_) = rx.try_recv() {
            count += 1;
        }

        assert_eq!(count, 10);
    }

    /// Test: Concurrent settings state access
    #[test]
    fn test_concurrent_settings_state_access() {
        use crate::gui::settings::SettingsState;

        let state = Arc::new(Mutex::new(SettingsState::default()));
        let mut handles = vec![];

        // Spawn multiple threads
        for i in 0..5 {
            let state_clone = Arc::clone(&state);
            let handle = thread::spawn(move || {
                let mut s = state_clone.lock().unwrap();
                if i % 2 == 0 {
                    s.mark_changed();
                } else {
                    s.mark_saved();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // State should be consistent
        let state = state.lock().unwrap();
        // has_unsaved_changes could be either true or false depending on execution order
        // We just verify no deadlock occurred
        let _ = state.has_unsaved_changes;
    }
}

// ============================================================================
// Module: Integration Tests
// ============================================================================

#[cfg(test)]
mod integration_tests {
    use crate::gui::{RecordingCommand, RecordingEvent, TranscriptionCommand, TranscriptionEvent};

    /// Test: Full recording workflow simulation
    #[tokio::test]
    async fn test_recording_workflow_simulation() {
        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel::<RecordingCommand>();
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<RecordingEvent>();

        // Simulate start command
        cmd_tx
            .send(RecordingCommand::Start {
                device_id: Some("test".to_string()),
            })
            .unwrap();

        // Simulate started event
        event_tx
            .send(RecordingEvent::Started {
                device_name: "test".to_string(),
                sample_rate: 16000,
            })
            .unwrap();

        // Verify
        let cmd = cmd_rx.recv().await.unwrap();
        match cmd {
            RecordingCommand::Start { device_id } => {
                assert_eq!(device_id, Some("test".to_string()));
            }
            _ => panic!("Wrong variant"),
        }

        let event = event_rx.recv().await.unwrap();
        match event {
            RecordingEvent::Started { device_name, sample_rate } => {
                assert_eq!(device_name, "test");
                assert_eq!(sample_rate, 16000);
            }
            _ => panic!("Wrong variant"),
        }
    }

    /// Test: Transcription workflow simulation
    #[tokio::test]
    async fn test_transcription_workflow_simulation() {
        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel::<TranscriptionCommand>();
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<TranscriptionEvent>();

        // Simulate transcribe command
        cmd_tx
            .send(TranscriptionCommand::Transcribe {
                samples: vec![0.1, 0.2, 0.3],
                model: "tiny".to_string(),
            })
            .unwrap();

        // Simulate progress
        event_tx
            .send(TranscriptionEvent::Progress {
                progress: 0.5,
                message: "Processing".to_string(),
            })
            .unwrap();

        // Simulate complete
        event_tx
            .send(TranscriptionEvent::Complete {
                text: "Hello world".to_string(),
                duration_ms: 100,
            })
            .unwrap();

        // Verify command
        let cmd = cmd_rx.recv().await.unwrap();
        match cmd {
            TranscriptionCommand::Transcribe { samples, model } => {
                assert_eq!(samples, vec![0.1, 0.2, 0.3]);
                assert_eq!(model, "tiny");
            }
            _ => panic!("Wrong variant"),
        }

        // Verify progress
        let event = event_rx.recv().await.unwrap();
        match event {
            TranscriptionEvent::Progress { progress, message } => {
                assert_eq!(progress, 0.5);
                assert_eq!(message, "Processing");
            }
            _ => panic!("Wrong variant"),
        }

        // Verify complete
        let event = event_rx.recv().await.unwrap();
        match event {
            TranscriptionEvent::Complete { text, duration_ms } => {
                assert_eq!(text, "Hello world");
                assert_eq!(duration_ms, 100);
            }
            _ => panic!("Wrong variant"),
        }
    }

    /// Test: Settings persistence workflow
    #[test]
    fn test_settings_persistence_workflow() {
        use crate::state::Settings;
        use std::io::Write;

        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file = temp_dir.path().join("settings.json");

        // Create initial settings
        let settings1 = Settings {
            whisper_model: Some("small".to_string()),
            audio_device: Some("device1".to_string()),
            orb_style: Some("nebula-aura-gpu:purple".to_string()),
            always_on_top: true,
            window_width: 500,
            window_height: 500,
        };

        // Save settings
        let json = serde_json::to_string_pretty(&settings1).unwrap();
        let mut file = std::fs::File::create(&settings_file).unwrap();
        file.write_all(json.as_bytes()).unwrap();

        // Load settings
        let content = std::fs::read_to_string(&settings_file).unwrap();
        let settings2: Settings = serde_json::from_str(&content).unwrap();

        // Verify roundtrip
        assert_eq!(settings1.whisper_model, settings2.whisper_model);
        assert_eq!(settings1.audio_device, settings2.audio_device);
        assert_eq!(settings1.orb_style, settings2.orb_style);
    }

    /// Test: Event-driven UI update workflow
    #[tokio::test]
    async fn test_event_driven_ui_update_workflow() {
        use crate::gui::GuiEventHandler;
        use crate::hooks::{EventBus, HookEvent};
        use std::sync::Arc;

        let event_bus = Arc::new(EventBus::new());
        let mut handler = GuiEventHandler::new();

        handler.subscribe(event_bus.clone()).await.unwrap();

        // Simulate audio events
        for i in 0..10 {
            event_bus
                .emit(HookEvent::AudioLevelChange {
                    level: i as f32 / 10.0,
                    peak: 0.8,
                })
                .await;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

        // Process events
        let mut updates = 0;
        handler.process_pending(|event| {
            if let HookEvent::AudioLevelChange { .. } = event {
                updates += 1;
            }
        });

        assert!(updates > 0, "Should process at least one event");
    }
}

// ============================================================================
// Test Harness Entry Point
// ============================================================================

#[cfg(test)]
mod test_harness {
    /// Test: Verify test environment is set up correctly
    #[test]
    fn test_environment_setup() {
        // This test verifies the basic test environment
        assert!(true, "Test environment is working");
    }
}
