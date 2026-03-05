//! Zana Main Application
//!
//! Implements the egui application with audio recording, transcription, and orb visualization.

use crate::audio::{AudioCapture, AudioDevice, AudioMetrics};
use crate::gui::{DialogState, GuiEventHandler, NotificationManager, OrbRenderer, SettingsPanel, ShortcutHandler, ShortcutAction};
use crate::hooks::{EventBus, HookEvent, HookEventType};
use crate::state::AppState;
use crate::stt::{TranscriptionResult, WhisperModel};
use eframe::egui;
use std::sync::Arc;

/// Main Zana application
pub struct ZanaApp {
    /// Core application state
    state: Arc<AppState>,

    /// Orb renderer for GPU visualization
    orb_renderer: OrbRenderer,

    /// Settings panel
    settings_panel: SettingsPanel,

    /// Event handler for EventBus integration
    event_handler: GuiEventHandler,

    /// UI state
    recording_state: RecordingState,
    transcription_state: TranscriptionState,
    ui_state: UIState,

    /// Audio metrics for visualization
    audio_metrics: AudioMetrics,

    /// Channel bridge for async communication
    channels: GuiChannels,

    /// Task spawner for background operations
    task_spawner: AsyncTaskSpawner,

    /// Tokio runtime for async operations
    _runtime: tokio::runtime::Runtime,

    /// Notification system
    notification_manager: NotificationManager,

    /// Dialog system
    dialog_state: DialogState,

    /// Keyboard shortcut handler
    shortcut_handler: ShortcutHandler,

    /// Fn key event receiver for push-to-talk
    fn_key_rx: Option<std::sync::mpsc::Receiver<crate::fn_key_monitor::FnKeyEvent>>,

    /// Track when Fn key was pressed (for minimum hold duration)
    fn_press_time: Option<std::time::Instant>,

    /// Flag to configure overlay window once on first frame
    overlay_configured: bool,
}

/// Recording state
#[derive(Default)]
struct RecordingState {
    is_recording: bool,
    selected_device: Option<String>,
    devices: Vec<AudioDevice>,
    last_error: Option<String>,
}

/// Transcription state
#[derive(Default)]
struct TranscriptionState {
    last_result: Option<TranscriptionResult>,
    is_transcribing: bool,
    progress: f32,
    models: Vec<ModelInfo>,
    current_model: String,
    last_error: Option<String>,
}

/// Model info for UI
#[derive(Debug, Clone)]
struct ModelInfo {
    pub id: String,
    pub name: String,
    pub size_mb: u64,
    pub downloaded: bool,
}

/// UI state
#[derive(Default)]
struct UIState {
    show_settings: bool,
    show_transcription: bool,
    auto_transcribe: bool,
}

impl ZanaApp {
    /// Create a new ZanaApp
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Create tokio runtime FIRST - before any async operations
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime");

        // Create application state
        let state = Arc::new(AppState::new().expect("Failed to initialize app state"));

        // Set runtime handle on AppState for use by other components (e.g., settings panel)
        {
            let mut handle = state.runtime_handle.write().unwrap();
            *handle = Some(runtime.handle().clone());
        }

        // Initialize event handler and subscribe to events
        let event_handler = GuiEventHandler::new();
        let event_bus = state.event_bus.clone();
        let handler_sender = event_handler.sender();

        // Spawn background task to subscribe to events and forward to GUI
        runtime.spawn(async move {
            if let Err(e) = crate::gui::event_handler::subscribe_to_events(event_bus, handler_sender).await {
                log::error!("Failed to subscribe to events: {}", e);
            }
        });

        // Emit app started event
        let event_bus = state.event_bus.clone();
        runtime.spawn(async move {
            event_bus.emit(HookEvent::AppStarted).await;
        });

        // Initialize orb renderer
        let orb_renderer = OrbRenderer::new(&cc.egui_ctx)
            .expect("Failed to initialize GPU orb renderer. Check that your GPU supports WebGPU, WebGL2, or has updated graphics drivers.");

        // Initialize settings panel
        let settings_panel = SettingsPanel::new();

        // Load initial state
        let recording_state = RecordingState {
            devices: AudioCapture::list_devices().unwrap_or_default(),
            ..Default::default()
        };

        let transcription_state = {
            let models = [
                WhisperModel::Tiny,
                WhisperModel::Base,
                WhisperModel::Small,
                WhisperModel::Medium,
                WhisperModel::Large,
            ];

            let model_infos: Vec<ModelInfo> = models
                .iter()
                .map(|m| ModelInfo {
                    id: format!("{:?}", m).to_lowercase(),
                    name: m.name().to_string(),
                    size_mb: m.size_mb(),
                    downloaded: false, // Will check asynchronously
                })
                .collect();

            TranscriptionState {
                models: model_infos,
                current_model: "small".to_string(),
                ..Default::default()
            }
        };

        // Spawn background task to check model availability
        let state_clone = state.clone();
        runtime.spawn(async move {
            let _engine = state_clone.whisper_engine.lock().await;
            log::info!("Whisper engine initialized");
        });

        // Initialize channel bridge for async communication
        let (channels, worker_channels) = GuiChannels::new();

        // Create task spawner with runtime handle
        let task_spawner = AsyncTaskSpawner::new(runtime.handle().clone(), state.event_bus.clone());

        // Spawn recording handler
        task_spawner.spawn_recording_handler(
            worker_channels.recording_cmd_rx,
            worker_channels.recording_event_tx,
            state.clone(),
        );

        // Spawn transcription handler
        task_spawner.spawn_transcription_handler(
            worker_channels.transcription_cmd_rx,
            worker_channels.transcription_event_tx,
            state.clone(),
        );

        // Setup Fn key monitor for push-to-talk
        let fn_key_rx = match crate::fn_key_monitor::setup_fn_key_monitor() {
            Ok(rx) => {
                log::info!("Fn key monitor active - hold Fn to record, release to transcribe");
                Some(rx)
            }
            Err(e) => {
                log::warn!("Fn key monitor not available: {}", e);
                None
            }
        };


        Self {
            state,
            orb_renderer,
            settings_panel,
            event_handler,
            recording_state,
            transcription_state,
            ui_state: UIState::default(),
            audio_metrics: AudioMetrics::default(),
            channels,
            task_spawner,
            _runtime: runtime,
            notification_manager: NotificationManager::new(),
            dialog_state: DialogState::new(),
            shortcut_handler: ShortcutHandler::new(),
            fn_key_rx,
            fn_press_time: None,
            overlay_configured: false,
        }
    }

    /// Toggle recording
    async fn toggle_recording(&mut self) -> Result<(), String> {
        if self.recording_state.is_recording {
            self.stop_recording().await
        } else {
            self.start_recording().await
        }
    }

    /// Start recording
    async fn start_recording(&mut self) -> Result<(), String> {
        let capture = self.state.audio_capture.lock().await;
        let device_id = self.recording_state.selected_device.as_deref();

        capture
            .start(device_id)
            .await
            .map_err(|e| format!("Failed to start recording: {}", e))?;

        self.recording_state.is_recording = true;
        self.recording_state.last_error = None;
        log::info!("Recording started");
        Ok(())
    }

    /// Stop recording
    async fn stop_recording(&mut self) -> Result<(), String> {
        let audio = {
            let capture = self.state.audio_capture.lock().await;
            capture
                .stop()
                .await
                .map_err(|e| format!("Failed to stop recording: {}", e))?
        };

        // Store captured audio
        *self.state.captured_audio.lock().await = Some(audio.clone());

        self.recording_state.is_recording = false;
        log::info!("Recording stopped: {} samples", audio.samples.len());

        // Auto-transcribe if enabled
        if self.ui_state.auto_transcribe {
            self.transcribe().await?;
        }

        Ok(())
    }

    /// Transcribe captured audio
    async fn transcribe(&mut self) -> Result<(), String> {
        // Check if we have captured audio
        let audio = {
            let audio_guard = self.state.captured_audio.lock().await;
            audio_guard.clone()
        };

        if audio.is_none() {
            return Err("No audio captured. Record first.".to_string());
        }

        let audio = audio.unwrap();

        // Get model
        let model = self.transcription_state.current_model
            .parse::<WhisperModel>()
            .ok()
            .unwrap_or(WhisperModel::Small);

        // Check if model is downloaded
        {
            let engine = self.state.whisper_engine.lock().await;
            if !engine.is_model_downloaded(model) {
                return Err(format!(
                    "Model '{}' not downloaded. Please download it first.",
                    model.name()
                ));
            }
        }

        // Run transcription
        self.transcription_state.is_transcribing = true;
        self.transcription_state.progress = 0.0;

        let engine = self.state.whisper_engine.lock().await;

        match engine.transcribe(&audio.samples, model).await {
            Ok(result) => {
                self.transcription_state.last_result = Some(result);
                self.transcription_state.is_transcribing = false;
                self.transcription_state.last_error = None;
                log::info!("Transcription complete");
                Ok(())
            }
            Err(e) => {
                self.transcription_state.is_transcribing = false;
                self.transcription_state.last_error = Some(e.to_string());
                Err(e.to_string())
            }
        }
    }

    /// Update audio metrics
    async fn update_metrics(&mut self) {
        let capture = self.state.audio_capture.lock().await;
        self.audio_metrics = capture.get_metrics().await;
        self.recording_state.is_recording = capture.is_recording();
    }

    /// Handle Fn key events for push-to-talk
    fn handle_fn_key_events(&mut self, ctx: &egui::Context) {
        use crate::fn_key_monitor::FnKeyEvent;

        let Some(rx) = &self.fn_key_rx else { return };

        // Poll for Fn key events (non-blocking)
        while let Ok(event) = rx.try_recv() {
            match event {
                FnKeyEvent::Pressed => {
                    log::info!("Fn key pressed - starting recording");
                    self.fn_press_time = Some(std::time::Instant::now());

                    // Show window as overlay
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));

                    // Start recording if not already
                    if !self.recording_state.is_recording {
                        let cmd = RecordingCommand::Start {
                            device_id: self.recording_state.selected_device.clone(),
                        };
                        let _ = self.channels.recording_cmd_tx.send(cmd);
                    }
                }
                FnKeyEvent::Released => {
                    // Check minimum hold duration (300ms)
                    let min_hold_ms = 300;
                    let held_long_enough = self.fn_press_time
                        .map(|t| t.elapsed().as_millis() >= min_hold_ms)
                        .unwrap_or(false);

                    if !held_long_enough {
                        log::info!("Fn key released too quickly, ignoring");
                        self.fn_press_time = None;
                        return;
                    }

                    log::info!("Fn key released - stopping and transcribing");
                    self.fn_press_time = None;

                    // Stop recording
                    if self.recording_state.is_recording {
                        let _ = self.channels.recording_cmd_tx.send(RecordingCommand::Stop);
                    }

                    // Set flag to auto-transcribe and paste after recording stops
                    self.ui_state.auto_transcribe = true;
                }
            }
        }
    }

    /// Render the main orb visualization - transparent floating orb like kollabor
    fn render_orb(&mut self, ctx: &egui::Context) {
        // Convert Vec<f32> to fixed-size array for orb renderer
        let mut fft_array = [0.0f32; 32];
        for (i, val) in self.audio_metrics.fft_bins.iter().take(32).enumerate() {
            fft_array[i] = *val;
        }

        // Update orb renderer with audio data
        self.orb_renderer.update_audio(self.audio_metrics.level, &fft_array);

        // Dark background for orb
        let frame = egui::Frame::new()
            .fill(egui::Color32::from_rgb(15, 15, 20));

        egui::CentralPanel::default()
            .frame(frame)
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let center = rect.center();
                let base_radius = rect.width().min(rect.height()) * 0.35;
                let time = ctx.input(|i| i.time) as f32;

                // Get audio level for effects
                let level = self.audio_metrics.level;
                let peak = self.audio_metrics.peak;
                let pulse = 1.0 + level * 0.4;

                let painter = ui.painter();

                // === OUTER GLOW LAYERS (multiple for soft effect) ===
                for i in 0..6 {
                    let glow_mult = 1.0 + (i as f32 * 0.15);
                    let glow_radius = base_radius * pulse * glow_mult;
                    let alpha = (30.0 - i as f32 * 5.0).max(5.0) * (0.3 + level * 0.7);

                    painter.circle_filled(
                        center,
                        glow_radius,
                        egui::Color32::from_rgba_unmultiplied(147, 80, 200, alpha as u8),
                    );
                }

                // === MAIN ORB - multiple layers for gradient effect ===
                // Outer purple
                painter.circle_filled(
                    center,
                    base_radius * pulse,
                    egui::Color32::from_rgba_unmultiplied(120, 60, 180, 180),
                );

                // Middle magenta
                painter.circle_filled(
                    center,
                    base_radius * pulse * 0.75,
                    egui::Color32::from_rgba_unmultiplied(180, 100, 220, 200),
                );

                // Inner bright
                painter.circle_filled(
                    center,
                    base_radius * pulse * 0.5,
                    egui::Color32::from_rgba_unmultiplied(220, 180, 255, 220),
                );

                // Core white glow
                let core_pulse = 1.0 + (time * 2.0).sin() * 0.1 + level * 0.3;
                painter.circle_filled(
                    center,
                    base_radius * 0.25 * core_pulse,
                    egui::Color32::from_rgba_unmultiplied(255, 240, 255, 250),
                );

                // === ORBITING PARTICLES ===
                let particle_count = 12;
                for i in 0..particle_count {
                    let fi = i as f32;
                    let golden = 1.618033988749_f32;

                    // Orbital motion
                    let base_angle = fi * golden * std::f32::consts::PI * 2.0;
                    let orbit_speed = 0.3 + (fi % 3.0) * 0.1;
                    let angle = base_angle + time * orbit_speed;

                    // Distance from center - expands with audio
                    let base_dist = base_radius * (0.6 + (fi % 5.0) * 0.1);
                    let dist = base_dist * (1.0 + level * 0.4);

                    let particle_pos = center + egui::vec2(angle.cos(), angle.sin()) * dist;

                    // Twinkle effect
                    let twinkle = (time * (2.0 + fi * 0.3) + fi).sin();
                    let particle_alpha = (150.0 + twinkle * 50.0 + level * 50.0) as u8;
                    let particle_size = 3.0 + twinkle * 1.5 + level * 3.0;

                    // Glow around particle
                    painter.circle_filled(
                        particle_pos,
                        particle_size * 2.0,
                        egui::Color32::from_rgba_unmultiplied(200, 150, 255, particle_alpha / 3),
                    );

                    // Particle core
                    painter.circle_filled(
                        particle_pos,
                        particle_size,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, particle_alpha),
                    );
                }

                // === AUDIO-REACTIVE RING ===
                if level > 0.05 {
                    let ring_radius = base_radius * (0.1 + level * 0.9);
                    let ring_alpha = ((level - 0.05) * 150.0).min(100.0) as u8;

                    painter.circle_stroke(
                        center,
                        ring_radius * pulse,
                        (2.0 + level * 2.0, egui::Color32::from_rgba_unmultiplied(180, 140, 255, ring_alpha)),
                    );
                }

                // === LENS FLARE (subtle) ===
                if level > 0.1 {
                    let flare_offset = egui::vec2(-base_radius * 0.3, -base_radius * 0.3);
                    let flare_pos = center + flare_offset;
                    let flare_alpha = ((level - 0.1) * 80.0).min(40.0) as u8;

                    painter.circle_filled(
                        flare_pos,
                        base_radius * 0.15,
                        egui::Color32::from_rgba_unmultiplied(255, 100, 150, flare_alpha),
                    );
                }

                // === PEAK INDICATOR RING ===
                if peak > 0.3 {
                    let peak_ring_radius = base_radius * (1.0 + peak * 0.3);
                    let peak_alpha = ((peak - 0.3) * 100.0).min(60.0) as u8;

                    painter.circle_stroke(
                        center,
                        peak_ring_radius,
                        (1.5, egui::Color32::from_rgba_unmultiplied(255, 200, 255, peak_alpha)),
                    );
                }
            });
    }


    /// Render the control panel
    fn render_controls(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::new()
            .fill(egui::Color32::from_rgb(30, 30, 35))
            .inner_margin(egui::Margin::same(8));

        egui::TopBottomPanel::top("controls")
            .frame(frame)
            .max_height(80.0)  // Fixed max height - prevents expansion
            .show(ctx, |ui| {
            ui.add_space(4.0);

            ui.horizontal_centered(|ui| {
                // Record/Stop button
                let button_text = if self.recording_state.is_recording {
                    "⏹ Stop"
                } else {
                    "🎤 Record"
                };

                let button_color = if self.recording_state.is_recording {
                    egui::Color32::RED
                } else {
                    egui::Color32::GREEN
                };

                let tooltip = if self.recording_state.is_recording {
                    "Press S to stop recording"
                } else {
                    "Press R to start recording"
                };

                if ui.add_sized([120.0, 40.0], egui::Button::new(button_text).fill(button_color))
                    .on_hover_text(tooltip)
                    .clicked()
                {
                    // Send recording command via channel
                    let cmd = if self.recording_state.is_recording {
                        RecordingCommand::Stop
                    } else {
                        RecordingCommand::Start {
                            device_id: self.recording_state.selected_device.clone(),
                        }
                    };
                    let _ = self.channels.recording_cmd_tx.send(cmd);
                }

                ui.add_space(16.0);

                // Settings button
                if ui.add_sized([100.0, 40.0], egui::Button::new("⚙ Settings"))
                    .on_hover_text("Press , for settings")
                    .clicked()
                {
                    self.ui_state.show_settings = !self.ui_state.show_settings;
                }

                ui.add_space(16.0);

                // Transcription button - disabled while recording or transcribing
                let transcribe_enabled = !self.recording_state.is_recording && !self.transcription_state.is_transcribing;
                let button_text = if self.transcription_state.is_transcribing {
                    "⏳ Working..."
                } else {
                    "📝 Transcribe"
                };

                if ui.add_enabled(
                    transcribe_enabled,
                    egui::Button::new(button_text).min_size(egui::vec2(120.0, 40.0))
                )
                    .on_hover_text("Press T to transcribe")
                    .clicked()
                {
                    // Get captured audio samples from state
                    if let Ok(audio_guard) = self.state.captured_audio.try_lock() {
                        if let Some(audio) = audio_guard.as_ref() {
                            // Start transcription
                            self.transcription_state.is_transcribing = true;
                            self.transcription_state.progress = 0.0;
                            self.transcription_state.last_error = None;
                            self.ui_state.show_transcription = true;

                            let cmd = TranscriptionCommand::Transcribe {
                                samples: audio.samples.clone(),
                                model: self.transcription_state.current_model.clone(),
                            };
                            let _ = self.channels.transcription_cmd_tx.send(cmd);
                            log::info!("Started transcription of {} samples", audio.samples.len());
                        } else {
                            // No captured audio - show error
                            self.notification_manager.warning("No audio captured. Record first.");
                        }
                    } else {
                        // Lock failed
                        self.notification_manager.error("System busy. Try again.");
                    }
                }
            });

                // Audio level indicator (inline with buttons when recording)
                if self.recording_state.is_recording {
                    ui.add_space(16.0);
                    ui.label("Level:");
                    let level = self.audio_metrics.level;
                    let color = if level < 0.5 {
                        egui::Color32::from_rgb(50, 200, 50)
                    } else {
                        egui::Color32::from_rgb(255, 150, 50)
                    };
                    ui.add_sized([100.0, 20.0], egui::ProgressBar::new(level).fill(color));
                }
        });
    }

    /// Render the transcription panel
    fn render_transcription(&mut self, ctx: &egui::Context) {
        let mut keep_open = true;

        egui::Window::new("Transcription")
            .collapsible(true)
            .resizable(true)
            .default_width(400.0)
            .open(&mut keep_open)
            .show(ctx, |ui| {
                if let Some(result) = &self.transcription_state.last_result {
                    // Selectable/copyable text
                    ui.add(egui::TextEdit::multiline(&mut result.text.as_str())
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Body));

                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        // Copy button
                        if ui.button("📋 Copy").clicked() {
                            ctx.copy_text(result.text.clone());
                            log::info!("Copied transcription to clipboard");
                        }

                        // Show processing time
                        if result.processing_ms > 0 {
                            ui.label(
                                egui::RichText::new(format!(
                                    "({:.2}s)",
                                    result.processing_ms as f64 / 1000.0
                                ))
                                .small()
                                .weak()
                            );
                        }
                    });
                } else if self.transcription_state.is_transcribing {
                    ui.vertical_centered(|ui| {
                        ui.add_space(8.0);

                        // Spinner
                        ui.spinner();

                        ui.add_space(8.0);

                        // Progress bar
                        if self.transcription_state.progress > 0.0 {
                            ui.add_sized(
                                [300.0, 20.0],
                                egui::ProgressBar::new(self.transcription_state.progress)
                                    .show_percentage()
                            );
                            ui.add_space(4.0);
                        }

                        // Status message
                        let status = if self.transcription_state.progress > 0.0 {
                            format!("Transcribing... {:.0}%", self.transcription_state.progress * 100.0)
                        } else {
                            "Transcribing...".to_string()
                        };

                        ui.label(status);
                    });
                } else if let Some(error) = &self.transcription_state.last_error {
                    ui.colored_label(egui::Color32::RED, error);
                } else {
                    ui.label("No transcription yet. Record and transcribe to see results.");
                }
            });

        // Handle window close
        if !keep_open {
            self.ui_state.show_transcription = false;
        }
    }

    /// Render the settings panel
    fn render_settings_panel(&mut self, ctx: &egui::Context) {
        if self.ui_state.show_settings {
            // show() returns false if X was clicked
            if !self.settings_panel.show(ctx, &self.state) {
                self.ui_state.show_settings = false;
            }
        }
    }

    /// Process events from the EventBus and update UI state
    fn process_events_from_bus(&mut self, ctx: &egui::Context) {
        self.event_handler.process_pending(|event| {
            match event {
                // AudioLevelChange -> update audio metrics for visualization
                HookEvent::AudioLevelChange { level, peak } => {
                    self.audio_metrics.level = *level;
                    self.audio_metrics.peak = *peak;
                    ctx.request_repaint();
                }

                // AudioFftReady -> update FFT data for orb visualization
                HookEvent::AudioFftReady { bins, .. } => {
                    self.audio_metrics.fft_bins = bins.clone();
                    ctx.request_repaint();
                }

                // TranscriptionProgress -> update progress UI
                HookEvent::TranscriptionProgress { percent } => {
                    self.transcription_state.is_transcribing = true;
                    self.transcription_state.progress = *percent;
                    ctx.request_repaint();
                }

                // TranscriptionComplete -> display transcription text
                HookEvent::TranscriptionComplete { text, .. } => {
                    self.transcription_state.is_transcribing = false;
                    self.transcription_state.last_result = Some(TranscriptionResult {
                        text: text.clone(),
                        processing_ms: 0,
                        segments: vec![],
                    });
                    self.transcription_state.last_error = None;
                    self.ui_state.show_transcription = true;
                    ctx.request_repaint();
                }

                // TranscriptionError -> show error
                HookEvent::TranscriptionError { error } => {
                    self.transcription_state.is_transcribing = false;
                    self.transcription_state.last_error = Some(error.clone());
                    ctx.request_repaint();
                }

                // Error events -> show error dialog
                HookEvent::Error { code, message } => {
                    log::error!("Error [{}]: {}", code, message);
                    self.recording_state.last_error = Some(format!("{}: {}", code, message));
                    ctx.request_repaint();
                }

                _ => {
                    // Other events not handled by GUI
                }
            }
        });
    }
}

impl eframe::App for ZanaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Configure overlay window once on first frame (must run on main thread)
        if !self.overlay_configured {
            self.overlay_configured = true;
            crate::fn_key_monitor::configure_overlay_window();
        }

        // Handle Fn key events (push-to-talk)
        self.handle_fn_key_events(ctx);

        // Handle keyboard shortcuts
        let action = self.shortcut_handler.handle_input(ctx);
        match action {
            ShortcutAction::Record => {
                if !self.recording_state.is_recording {
                    let cmd = RecordingCommand::Start {
                        device_id: self.recording_state.selected_device.clone(),
                    };
                    let _ = self.channels.recording_cmd_tx.send(cmd);
                }
            }
            ShortcutAction::Stop => {
                if self.recording_state.is_recording {
                    let _ = self.channels.recording_cmd_tx.send(RecordingCommand::Stop);
                }
            }
            ShortcutAction::Transcribe => {
                // Don't transcribe if already transcribing or recording
                if self.transcription_state.is_transcribing || self.recording_state.is_recording {
                    return;
                }

                if let Ok(audio_guard) = self.state.captured_audio.try_lock() {
                    if let Some(audio) = audio_guard.as_ref() {
                        // Start transcription
                        self.transcription_state.is_transcribing = true;
                        self.transcription_state.progress = 0.0;
                        self.transcription_state.last_error = None;
                        self.ui_state.show_transcription = true;

                        let cmd = TranscriptionCommand::Transcribe {
                            samples: audio.samples.clone(),
                            model: self.transcription_state.current_model.clone(),
                        };
                        let _ = self.channels.transcription_cmd_tx.send(cmd);
                        log::info!("Started transcription via shortcut");
                    } else {
                        self.notification_manager.warning("No audio captured. Record first.");
                    }
                }
            }
            ShortcutAction::Settings => {
                self.ui_state.show_settings = !self.ui_state.show_settings;
            }
            ShortcutAction::Hide => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            ShortcutAction::None => {}
        }

        // Show notifications
        self.notification_manager.show(ctx);

        // Show modal dialogs
        self.dialog_state.show(ctx);

        // Poll recording events
        while let Ok(event) = self.channels.recording_event_rx.try_recv() {
            match event {
                RecordingEvent::Started { device_name, sample_rate } => {
                    self.recording_state.is_recording = true;
                    self.recording_state.last_error = None;
                    log::info!("Recording started: {} @ {}Hz", device_name, sample_rate);
                }
                RecordingEvent::Stopped { sample_count, duration_ms } => {
                    self.recording_state.is_recording = false;
                    log::info!("Recording stopped: {} samples, {}ms", sample_count, duration_ms);

                    // Auto-transcribe if flag is set (from Fn key release)
                    if self.ui_state.auto_transcribe && sample_count > 0 {
                        // Start transcription
                        if let Ok(audio_guard) = self.state.captured_audio.try_lock() {
                            if let Some(audio) = audio_guard.as_ref() {
                                self.transcription_state.is_transcribing = true;
                                self.transcription_state.progress = 0.0;
                                self.transcription_state.last_error = None;

                                let cmd = TranscriptionCommand::Transcribe {
                                    samples: audio.samples.clone(),
                                    model: self.transcription_state.current_model.clone(),
                                };
                                let _ = self.channels.transcription_cmd_tx.send(cmd);
                                log::info!("Auto-transcribing {} samples", audio.samples.len());
                            }
                        }
                    }
                }
                RecordingEvent::MetricsUpdate { level, peak, fft_bins } => {
                    self.audio_metrics.level = level;
                    self.audio_metrics.peak = peak;
                    self.audio_metrics.fft_bins = fft_bins;
                }
                RecordingEvent::Error(err) => {
                    self.recording_state.last_error = Some(err.clone());
                    self.recording_state.is_recording = false;

                    // Show error dialog
                    self.dialog_state.show_error_dialog(
                        "Recording Error",
                        err,
                    );
                }
            }
        }

        // Poll transcription events
        while let Ok(event) = self.channels.transcription_event_rx.try_recv() {
            match event {
                TranscriptionEvent::Progress { progress, message } => {
                    self.transcription_state.is_transcribing = true;
                    self.transcription_state.progress = progress;
                    log::debug!("Transcription progress: {:.0}% - {}", progress * 100.0, message);
                }
                TranscriptionEvent::Complete { text, duration_ms } => {
                    self.transcription_state.is_transcribing = false;
                    self.transcription_state.last_result = Some(TranscriptionResult {
                        text: text.clone(),
                        processing_ms: duration_ms as u64,
                        segments: vec![],
                    });
                    self.transcription_state.last_error = None;

                    log::info!("Transcription complete: {}ms", duration_ms);

                    // If auto-transcribe mode (from Fn key), paste and hide
                    if self.ui_state.auto_transcribe {
                        self.ui_state.auto_transcribe = false;

                        // Paste text to active input
                        if !text.trim().is_empty() {
                            if let Err(e) = crate::fn_key_monitor::paste_text(&text) {
                                log::error!("Failed to paste: {}", e);
                                self.notification_manager.error(format!("Paste failed: {}", e));
                            } else {
                                log::info!("Pasted {} chars to active input", text.len());
                            }
                        }

                        // Hide window (not minimize - avoids dock animation)
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                    } else {
                        // Normal mode - show transcription panel
                        self.ui_state.show_transcription = true;

                        // Show success notification with text preview
                        let preview = if text.len() > 100 {
                            format!("{}...", &text[..100])
                        } else {
                            text.clone()
                        };
                        self.notification_manager.success(format!(
                            "Transcription complete in {:.1}s\n{}",
                            duration_ms as f64 / 1000.0,
                            preview
                        ));
                    }
                }
                TranscriptionEvent::Error(err) => {
                    self.transcription_state.is_transcribing = false;
                    self.transcription_state.last_error = Some(err.clone());

                    // Show error dialog
                    self.dialog_state.show_error_dialog(
                        "Transcription Failed",
                        err,
                    );
                }
            }
        }

        // Process events from EventBus
        self.process_events_from_bus(ctx);

        // Update audio metrics
        ctx.request_repaint();

        // Render settings panel (window)
        self.render_settings_panel(ctx);

        // Render transcription panel (window)
        if self.ui_state.show_transcription {
            self.render_transcription(ctx);
        }

        // Render orb visualization (full window)
        self.render_orb(ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // Stop any ongoing recording
        if self.recording_state.is_recording {
            let _ = self.channels.recording_cmd_tx.send(RecordingCommand::Stop);
            log::info!("Stopped recording on shutdown");
        }

        // Cancel any ongoing transcription
        if self.transcription_state.is_transcribing {
            let _ = self.channels.transcription_cmd_tx.send(TranscriptionCommand::Cancel);
            log::info!("Cancelled transcription on shutdown");
        }

        // Save settings
        if let Ok(settings) = self.state.settings.try_read() {
            if let Ok(json) = serde_json::to_string(&*settings) {
                storage.set_string("settings", json);
                log::info!("Settings saved");
            }
        }

        // Note: Tokio runtime and background tasks will be automatically cleaned up
        // when ZanaApp is dropped. The channel senders will be dropped, causing
        // the receiver loops in background tasks to end.
    }
}

// ============================================================================
// Channel Bridge Types - GUI to Async Communication
// ============================================================================

/// Commands sent from GUI to async recording task
#[derive(Debug, Clone)]
pub enum RecordingCommand {
    /// Start audio recording
    Start { device_id: Option<String> },
    /// Stop audio recording
    Stop,
    /// Query recording status
    QueryStatus,
}

/// Events sent from async recording task to GUI
#[derive(Debug, Clone)]
pub enum RecordingEvent {
    /// Recording started successfully
    Started { device_name: String, sample_rate: u32 },
    /// Recording stopped with captured audio
    Stopped { sample_count: usize, duration_ms: u64 },
    /// Real-time audio metrics update
    MetricsUpdate { level: f32, peak: f32, fft_bins: Vec<f32> },
    /// Recording error occurred
    Error(String),
}

/// Commands sent from GUI to async transcription task
#[derive(Debug, Clone)]
pub enum TranscriptionCommand {
    /// Transcribe audio data
    Transcribe { samples: Vec<f32>, model: String },
    /// Cancel ongoing transcription
    Cancel,
}

/// Events sent from async transcription task to GUI
#[derive(Debug, Clone)]
pub enum TranscriptionEvent {
    /// Transcription progress update
    Progress { progress: f32, message: String },
    /// Transcription completed successfully
    Complete { text: String, duration_ms: u32 },
    /// Transcription error occurred
    Error(String),
}

/// Channel bundle for GUI-async communication
///
/// The GUI app holds this struct to send commands and receive events.
pub struct GuiChannels {
    /// Recording command sender (GUI -> async)
    pub recording_cmd_tx: tokio::sync::mpsc::UnboundedSender<RecordingCommand>,
    /// Recording event receiver (async -> GUI)
    pub recording_event_rx: tokio::sync::mpsc::UnboundedReceiver<RecordingEvent>,
    /// Transcription command sender (GUI -> async)
    pub transcription_cmd_tx: tokio::sync::mpsc::UnboundedSender<TranscriptionCommand>,
    /// Transcription event receiver (async -> GUI)
    pub transcription_event_rx: tokio::sync::mpsc::UnboundedReceiver<TranscriptionEvent>,
}

impl GuiChannels {
    /// Create new channel bundle
    /// Returns (GUI side, async worker side)
    pub fn new() -> (Self, AsyncWorkerChannels) {
        // Recording channels: GUI sends commands, worker sends events
        let (recording_cmd_tx, recording_cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        let (recording_event_tx, recording_event_rx) = tokio::sync::mpsc::unbounded_channel();

        // Transcription channels: GUI sends commands, worker sends events
        let (transcription_cmd_tx, transcription_cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        let (transcription_event_tx, transcription_event_rx) = tokio::sync::mpsc::unbounded_channel();

        let gui_channels = Self {
            recording_cmd_tx,
            recording_event_rx,
            transcription_cmd_tx,
            transcription_event_rx,
        };

        let worker_channels = AsyncWorkerChannels {
            recording_cmd_rx,
            recording_event_tx,
            transcription_cmd_rx,
            transcription_event_tx,
        };

        (gui_channels, worker_channels)
    }
}

/// Channel bundle for async worker tasks
///
/// Workers receive commands and send events back to GUI.
pub struct AsyncWorkerChannels {
    /// Recording command receiver
    pub recording_cmd_rx: tokio::sync::mpsc::UnboundedReceiver<RecordingCommand>,
    /// Recording event sender
    pub recording_event_tx: tokio::sync::mpsc::UnboundedSender<RecordingEvent>,
    /// Transcription command receiver
    pub transcription_cmd_rx: tokio::sync::mpsc::UnboundedReceiver<TranscriptionCommand>,
    /// Transcription event sender
    pub transcription_event_tx: tokio::sync::mpsc::UnboundedSender<TranscriptionEvent>,
}

/// Task spawner for running async operations from GUI
pub struct AsyncTaskSpawner {
    /// Runtime handle for spawning tasks
    runtime: tokio::runtime::Handle,
    /// Event bus for emitting events
    event_bus: Arc<EventBus>,
}

impl AsyncTaskSpawner {
    /// Create new task spawner
    pub fn new(runtime: tokio::runtime::Handle, event_bus: Arc<EventBus>) -> Self {
        Self { runtime, event_bus }
    }

    /// Spawn recording command handler
    pub fn spawn_recording_handler(
        &self,
        mut rx: tokio::sync::mpsc::UnboundedReceiver<RecordingCommand>,
        tx: tokio::sync::mpsc::UnboundedSender<RecordingEvent>,
        state: Arc<AppState>,
    ) {
        self.runtime.spawn(async move {
            log::info!("Recording handler task started");

            while let Some(cmd) = rx.recv().await {
                match cmd {
                    RecordingCommand::Start { device_id } => {
                        log::debug!("Received Start command: {:?}", device_id);

                        let capture = state.audio_capture.lock().await;
                        let result = capture.start(device_id.as_deref()).await;

                        match result {
                            Ok(_) => {
                                let _ = tx.send(RecordingEvent::Started {
                                    device_name: device_id.unwrap_or_else(|| "default".to_string()),
                                    sample_rate: 16000,
                                });
                            }
                            Err(e) => {
                                let _ = tx.send(RecordingEvent::Error(e.to_string()));
                            }
                        }
                    }
                    RecordingCommand::Stop => {
                        log::debug!("Received Stop command");

                        let capture = state.audio_capture.lock().await;
                        let result = capture.stop().await;

                        match result {
                            Ok(audio) => {
                                let sample_count = audio.samples.len();
                                let duration_ms = audio.duration_ms;

                                // Store the captured audio for transcription
                                {
                                    let mut captured = state.captured_audio.lock().await;
                                    *captured = Some(audio);
                                    log::info!("Stored {} samples for transcription", sample_count);
                                }

                                let _ = tx.send(RecordingEvent::Stopped {
                                    sample_count,
                                    duration_ms,
                                });
                            }
                            Err(e) => {
                                let _ = tx.send(RecordingEvent::Error(e.to_string()));
                            }
                        }
                    }
                    RecordingCommand::QueryStatus => {
                        // Status is polled via get_metrics, nothing to do here
                    }
                }
            }

            log::info!("Recording handler task ended");
        });
    }

    /// Spawn transcription command handler
    pub fn spawn_transcription_handler(
        &self,
        mut rx: tokio::sync::mpsc::UnboundedReceiver<TranscriptionCommand>,
        tx: tokio::sync::mpsc::UnboundedSender<TranscriptionEvent>,
        state: Arc<AppState>,
    ) {
        self.runtime.spawn(async move {
            log::info!("Transcription handler task started");

            while let Some(cmd) = rx.recv().await {
                match cmd {
                    TranscriptionCommand::Transcribe { samples, model } => {
                        log::debug!("Received Transcribe command: {} samples", samples.len());

                        let engine = state.whisper_engine.lock().await;
                        let model_enum = model.parse::<WhisperModel>()
                            .ok()
                            .unwrap_or(WhisperModel::Small);

                        // Emit progress start
                        let _ = tx.send(TranscriptionEvent::Progress {
                            progress: 0.0,
                            message: "Starting transcription...".to_string(),
                        });

                        match engine.transcribe(&samples, model_enum).await {
                            Ok(result) => {
                                let _ = tx.send(TranscriptionEvent::Complete {
                                    text: result.text,
                                    duration_ms: result.processing_ms as u32,
                                });
                            }
                            Err(e) => {
                                let _ = tx.send(TranscriptionEvent::Error(e.to_string()));
                            }
                        }
                    }
                    TranscriptionCommand::Cancel => {
                        log::debug!("Received Cancel command");
                        // Cancel logic would go here
                    }
                }
            }

            log::info!("Transcription handler task ended");
        });
    }
}

// ============================================================================
// Tests - TDD Approach
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_recording_command_start() {
        let cmd = RecordingCommand::Start {
            device_id: Some("test-device".to_string()),
        };
        match cmd {
            RecordingCommand::Start { device_id } => {
                assert_eq!(device_id, Some("test-device".to_string()));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_recording_command_stop() {
        let cmd = RecordingCommand::Stop;
        match cmd {
            RecordingCommand::Stop => {}
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_recording_event_started() {
        let event = RecordingEvent::Started {
            device_name: "test".to_string(),
            sample_rate: 16000,
        };
        match event {
            RecordingEvent::Started { device_name, sample_rate } => {
                assert_eq!(device_name, "test");
                assert_eq!(sample_rate, 16000);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_recording_event_stopped() {
        let event = RecordingEvent::Stopped {
            sample_count: 1000,
            duration_ms: 500,
        };
        match event {
            RecordingEvent::Stopped { sample_count, duration_ms } => {
                assert_eq!(sample_count, 1000);
                assert_eq!(duration_ms, 500);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_recording_event_metrics() {
        let fft_bins = vec![0.1, 0.2, 0.3];
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
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_recording_event_error() {
        let event = RecordingEvent::Error("test error".to_string());
        match event {
            RecordingEvent::Error(msg) => {
                assert_eq!(msg, "test error");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_transcription_command_transcribe() {
        let samples = vec![0.1, 0.2, 0.3];
        let cmd = TranscriptionCommand::Transcribe {
            samples: samples.clone(),
            model: "small".to_string(),
        };
        match cmd {
            TranscriptionCommand::Transcribe { samples: s, model } => {
                assert_eq!(s, samples);
                assert_eq!(model, "small");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_transcription_command_cancel() {
        let cmd = TranscriptionCommand::Cancel;
        match cmd {
            TranscriptionCommand::Cancel => {}
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_transcription_event_progress() {
        let event = TranscriptionEvent::Progress {
            progress: 0.5,
            message: "Processing...".to_string(),
        };
        match event {
            TranscriptionEvent::Progress { progress, message } => {
                assert_eq!(progress, 0.5);
                assert_eq!(message, "Processing...");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_transcription_event_complete() {
        let event = TranscriptionEvent::Complete {
            text: "Hello world".to_string(),
            duration_ms: 100,
        };
        match event {
            TranscriptionEvent::Complete { text, duration_ms } => {
                assert_eq!(text, "Hello world");
                assert_eq!(duration_ms, 100);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_transcription_event_error() {
        let event = TranscriptionEvent::Error("transcription failed".to_string());
        match event {
            TranscriptionEvent::Error(msg) => {
                assert_eq!(msg, "transcription failed");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_channel_send_receive_recording() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<RecordingCommand>();

        let cmd = RecordingCommand::Start {
            device_id: Some("test".to_string()),
        };
        tx.send(cmd).unwrap();

        let received = rx.blocking_recv().unwrap();
        match received {
            RecordingCommand::Start { device_id } => {
                assert_eq!(device_id, Some("test".to_string()));
            }
            _ => panic!("Wrong variant received"),
        }
    }

    #[test]
    fn test_channel_send_receive_transcription() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<TranscriptionEvent>();

        let event = TranscriptionEvent::Complete {
            text: "test".to_string(),
            duration_ms: 100,
        };
        tx.send(event).unwrap();

        let received = rx.blocking_recv().unwrap();
        match received {
            TranscriptionEvent::Complete { text, duration_ms } => {
                assert_eq!(text, "test");
                assert_eq!(duration_ms, 100);
            }
            _ => panic!("Wrong variant received"),
        }
    }

    #[test]
    fn test_gui_channels_creation() {
        let (channels, _receiver) = GuiChannels::new();

        // Verify channels are created
        assert!(channels.recording_cmd_tx.send(RecordingCommand::QueryStatus).is_ok());
        assert!(channels.transcription_cmd_tx.send(TranscriptionCommand::Cancel).is_ok());
    }

    #[test]
    fn test_recording_event_clone() {
        let event = RecordingEvent::Started {
            device_name: "test".to_string(),
            sample_rate: 16000,
        };
        let event_clone = event.clone();
        match event_clone {
            RecordingEvent::Started { device_name, sample_rate } => {
                assert_eq!(device_name, "test");
                assert_eq!(sample_rate, 16000);
            }
            _ => panic!("Clone failed"),
        }
    }

    #[test]
    fn test_transcription_event_clone() {
        let event = TranscriptionEvent::Progress {
            progress: 0.75,
            message: "Almost done".to_string(),
        };
        let event_clone = event.clone();
        match event_clone {
            TranscriptionEvent::Progress { progress, message } => {
                assert_eq!(progress, 0.75);
                assert_eq!(message, "Almost done");
            }
            _ => panic!("Clone failed"),
        }
    }
}

