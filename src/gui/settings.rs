//! Settings Panel
//!
//! UI for configuring kVoice settings.

use crate::audio::AudioCapture;
use crate::state::{AppState, Settings};
use crate::stt::WhisperModel;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Available orb style plugins
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrbStyle {
    /// Plugin ID
    pub id: String,
    /// Display name
    pub name: String,
    /// Available color schemes
    pub color_schemes: Vec<String>,
}

impl OrbStyle {
    /// Get available orb styles from plugins directory
    pub fn discover_styles() -> Vec<Self> {
        // For now, return hardcoded styles based on available plugins
        vec![
            OrbStyle {
                id: "nebula-aura-gpu".to_string(),
                name: "Nebula Aura (GPU)".to_string(),
                color_schemes: vec![
                    "purple".to_string(),
                    "cyan".to_string(),
                    "fire".to_string(),
                    "aurora".to_string(),
                    "cosmic".to_string(),
                ],
            },
            OrbStyle {
                id: "nebula-aura".to_string(),
                name: "Nebula Aura".to_string(),
                color_schemes: vec![
                    "purple".to_string(),
                    "cyan".to_string(),
                    "fire".to_string(),
                ],
            },
        ]
    }
}

/// Settings state for the UI
#[derive(Debug, Clone)]
pub struct SettingsState {
    /// Current settings (may have unsaved changes)
    pub settings: Settings,
    /// Available audio devices
    pub audio_devices: Vec<String>,
    /// Downloaded Whisper models
    pub downloaded_models: Vec<WhisperModel>,
    /// Available orb styles
    pub orb_styles: Vec<OrbStyle>,
    /// Currently downloading model (if any)
    pub downloading_model: Option<WhisperModel>,
    /// Download progress (0.0 - 1.0)
    pub download_progress: f32,
    /// Whether settings have unsaved changes
    pub has_unsaved_changes: bool,
    /// Status message to display
    pub status_message: Option<String>,
    /// Whether status is an error
    pub status_is_error: bool,
    /// Selected color scheme for orb style
    pub selected_color_scheme: String,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            audio_devices: Vec::new(),
            downloaded_models: Vec::new(),
            orb_styles: OrbStyle::discover_styles(),
            downloading_model: None,
            download_progress: 0.0,
            has_unsaved_changes: false,
            status_message: None,
            status_is_error: false,
            selected_color_scheme: "purple".to_string(),
        }
    }
}

impl SettingsState {
    /// Create new settings state from loaded settings
    pub fn from_settings(settings: Settings) -> Self {
        let orb_styles = OrbStyle::discover_styles();

        // Extract color scheme from orb_style if set
        let selected_color_scheme = settings
            .orb_style
            .as_ref()
            .and_then(|style| {
                // Parse format: "plugin-id:scheme"
                style.split(':').nth(1).map(String::from)
            })
            .unwrap_or_else(|| "purple".to_string());

        Self {
            settings,
            orb_styles,
            selected_color_scheme,
            ..Default::default()
        }
    }

    /// Update audio devices list
    pub fn update_audio_devices(&mut self) {
        match AudioCapture::list_devices() {
            Ok(devices) => {
                self.audio_devices = devices.into_iter().map(|d| d.id).collect();
            }
            Err(e) => {
                log::warn!("Failed to list audio devices: {}", e);
                self.audio_devices = Vec::new();
            }
        }
    }

    /// Update downloaded models list
    pub fn update_downloaded_models(&mut self, models_dir: &std::path::Path) {
        self.downloaded_models = vec![
            WhisperModel::Tiny,
            WhisperModel::Base,
            WhisperModel::Small,
            WhisperModel::Medium,
            WhisperModel::Large,
        ]
        .into_iter()
        .filter(|m| models_dir.join(m.filename()).exists())
        .collect();
    }

    /// Check if a model is downloaded
    pub fn is_model_downloaded(&self, model: WhisperModel) -> bool {
        self.downloaded_models.contains(&model)
    }

    /// Get the formatted orb style string (plugin-id:scheme)
    pub fn get_orb_style_string(&self) -> Option<String> {
        if let Some(plugin_id) = &self.settings.orb_style {
            // Check if it already contains a colon (already has scheme)
            if plugin_id.contains(':') {
                return Some(plugin_id.clone());
            }
            // Otherwise, append the selected color scheme
            return Some(format!("{}:{}", plugin_id, self.selected_color_scheme));
        }
        None
    }

    /// Parse orb style string to extract plugin ID and scheme
    pub fn parse_orb_style(&self, style_str: &str) -> (String, String) {
        if let Some((plugin, scheme)) = style_str.split_once(':') {
            (plugin.to_string(), scheme.to_string())
        } else {
            (style_str.to_string(), "purple".to_string())
        }
    }

    /// Mark settings as changed
    pub fn mark_changed(&mut self) {
        self.has_unsaved_changes = true;
    }

    /// Clear unsaved changes flag
    pub fn mark_saved(&mut self) {
        self.has_unsaved_changes = false;
    }

    /// Set status message (info)
    pub fn set_status(&mut self, msg: String) {
        self.status_message = Some(msg);
        self.status_is_error = false;
    }

    /// Set error message
    pub fn set_error(&mut self, msg: String) {
        self.status_message = Some(msg);
        self.status_is_error = true;
    }

    /// Clear status message
    pub fn clear_status(&mut self) {
        self.status_message = None;
        self.status_is_error = false;
    }
}

/// Settings panel UI
pub struct SettingsPanel {
    /// Settings state
    state: SettingsState,
    /// Shared download progress (0-100)
    download_progress_shared: Arc<std::sync::atomic::AtomicU32>,
    /// Download complete flag
    download_complete: Arc<std::sync::atomic::AtomicBool>,
    /// Download error message (if any)
    download_error: Arc<std::sync::RwLock<Option<String>>>,
}

impl SettingsPanel {
    /// Create a new settings panel
    pub fn new() -> Self {
        Self {
            state: SettingsState::default(),
            download_progress_shared: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            download_complete: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            download_error: Arc::new(std::sync::RwLock::new(None)),
        }
    }

    /// Load settings from app state
    pub fn load_settings(&mut self, settings: Settings) {
        self.state = SettingsState::from_settings(settings);
    }

    /// Show the settings panel
    /// Returns false if the window was closed (X clicked)
    pub fn show(&mut self, ctx: &egui::Context, app_state: &Arc<AppState>) -> bool {
        // Poll download progress from shared atomics
        if self.state.downloading_model.is_some() {
            let progress = self.download_progress_shared.load(std::sync::atomic::Ordering::SeqCst);
            self.state.download_progress = progress as f32 / 100.0;

            // Check for completion
            if self.download_complete.load(std::sync::atomic::Ordering::SeqCst) {
                // Check for errors
                if let Some(error) = self.download_error.read().unwrap().clone() {
                    self.state.set_error(format!("Download failed: {}", error));
                } else {
                    self.state.set_status("Download complete!".to_string());
                }
                self.state.downloading_model = None;

                // Refresh downloaded models list
                if let Ok(models_dir) = crate::stt::WhisperEngine::get_models_dir() {
                    self.state.update_downloaded_models(&models_dir);
                }
            }

            // Request repaint to update progress bar
            ctx.request_repaint();
        }

        // Update audio devices list on first show or periodically
        if self.state.audio_devices.is_empty() {
            self.state.update_audio_devices();
        }

        // Update downloaded models
        if let Ok(models_dir) = crate::stt::WhisperEngine::get_models_dir() {
            self.state.update_downloaded_models(&models_dir);
        }

        let mut keep_open = true;

        egui::Window::new("Settings")
            .collapsible(false)
            .resizable(true)
            .default_width(550.0)
            .open(&mut keep_open)
            .show(ctx, |ui| {
                self.show_audio_section(ui);
                ui.separator();

                self.show_transcription_section(ui, app_state);
                ui.separator();

                self.show_visualization_section(ui);
                ui.separator();

                self.show_about_section(ui);
                ui.separator();

                self.show_status_message(ui);
                ui.separator();

                self.show_save_button(ui, app_state);
            });

        keep_open
    }

    /// Show audio settings section
    fn show_audio_section(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Audio", |ui| {
            ui.label("Audio input device:");

            if self.state.audio_devices.is_empty() {
                ui.label(egui::RichText::new("No audio devices found").color(egui::Color32::GRAY));
                if ui.button("Refresh").clicked() {
                    self.state.update_audio_devices();
                }
            } else {
                let mut selected_device = self.state.settings.audio_device.clone().unwrap_or_default();
                let previous_device = selected_device.clone();

                egui::ComboBox::from_id_salt("audio_device")
                    .width(250.0)
                    .selected_text(&selected_device)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut selected_device, String::new(), "Default");
                        for device in &self.state.audio_devices {
                            ui.selectable_value(&mut selected_device, device.clone(), device);
                        }
                    });

                if selected_device != previous_device {
                    self.state.settings.audio_device = if selected_device.is_empty() {
                        None
                    } else {
                        Some(selected_device)
                    };
                    self.state.mark_changed();
                }

                ui.add_space(5.0);

                // Device info
                if let Some(device) = &self.state.settings.audio_device {
                    ui.label(format!("Selected: {}", device));
                } else {
                    ui.label(egui::RichText::new("Using system default").italics());
                }
            }
        });
    }

    /// Show transcription settings section
    fn show_transcription_section(&mut self, ui: &mut egui::Ui, app_state: &Arc<AppState>) {
        ui.collapsing("Transcription", |ui| {
            ui.label("Whisper model:");

            let models = [
                WhisperModel::Tiny,
                WhisperModel::Base,
                WhisperModel::Small,
                WhisperModel::Medium,
                WhisperModel::Large,
            ];

            let current_model = self
                .state
                .settings
                .whisper_model
                .as_ref()
                .and_then(|m| m.parse::<WhisperModel>().ok())
                .unwrap_or_default();

            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("whisper_model")
                    .width(150.0)
                    .selected_text(current_model.name())
                    .show_ui(ui, |ui| {
                        for model in models {
                            let is_downloaded = self.state.is_model_downloaded(model);
                            let label = if is_downloaded {
                                format!("{} ({})", model.name(), model.filename())
                            } else {
                                format!("{} (Not downloaded)", model.name())
                            };

                            ui.selectable_value(
                                &mut self.state.settings.whisper_model,
                                Some(model.filename().to_string()),
                                label,
                            );
                        }
                    });

                // Model size info
                ui.label(format!("{} MB", current_model.size_mb()));
            });

            ui.add_space(5.0);

            // Download status and button
            if self.state.is_model_downloaded(current_model) {
                ui.label(egui::RichText::new("Model downloaded").color(egui::Color32::DARK_GREEN));
            } else {
                ui.label(egui::RichText::new("Model not downloaded").color(egui::Color32::GRAY));

                if let Some(downloading) = self.state.downloading_model {
                    if downloading == current_model {
                        ui.add_space(5.0);

                        // Spinner and progress bar
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(format!(
                                "Downloading... {:.0}%",
                                self.state.download_progress * 100.0
                            ));
                        });

                        // Progress bar
                        ui.add_space(5.0);
                        ui.add_sized(
                            [250.0, 20.0],
                            egui::ProgressBar::new(self.state.download_progress)
                                .show_percentage()
                                .fill(egui::Color32::BLUE)
                        );

                        // Size info
                        let downloaded_mb = (current_model.size_mb() as f32 * self.state.download_progress) as u64;
                        ui.label(
                            egui::RichText::new(format!(
                                "{}/{} MB downloaded",
                                downloaded_mb,
                                current_model.size_mb()
                            ))
                            .small()
                            .weak()
                        );
                    } else if ui.button("Download").clicked() {
                        self.start_model_download(current_model, app_state);
                    }
                } else if ui.button("Download Model").clicked() {
                    self.start_model_download(current_model, app_state);
                }

                ui.label(format!("Size: {} MB", current_model.size_mb()));
            }

            ui.add_space(5.0);
            ui.label(egui::RichText::new("Models are stored in:").small().weak());
            if let Ok(models_dir) = crate::stt::WhisperEngine::get_models_dir() {
                ui.label(egui::RichText::new(models_dir.display().to_string()).small().monospace());
            }
        });
    }

    /// Show visualization settings section
    fn show_visualization_section(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Visualization", |ui| {
            ui.label("Orb style plugin:");

            if self.state.orb_styles.is_empty() {
                ui.label(egui::RichText::new("No orb styles found").color(egui::Color32::GRAY));
            } else {
                // Get current plugin ID
                let current_plugin_id = self
                    .state
                    .settings
                    .orb_style
                    .as_ref()
                    .and_then(|s| self.state.parse_orb_style(s).0.into())
                    .unwrap_or_else(|| "nebula-aura-gpu".to_string());

                let mut selected_plugin = current_plugin_id.clone();

                egui::ComboBox::from_id_salt("orb_style_plugin")
                    .width(250.0)
                    .selected_text(
                        self.state
                            .orb_styles
                            .iter()
                            .find(|s| s.id == selected_plugin)
                            .map(|s| s.name.as_str())
                            .unwrap_or("Unknown"),
                    )
                    .show_ui(ui, |ui| {
                        for style in &self.state.orb_styles {
                            ui.selectable_value(&mut selected_plugin, style.id.clone(), &style.name);
                        }
                    });

                if selected_plugin != current_plugin_id {
                    self.state.settings.orb_style = Some(selected_plugin.clone());
                    self.state.mark_changed();
                }

                ui.add_space(5.0);

                // Color scheme selector
                if let Some(style) = self.state.orb_styles.iter().find(|s| s.id == selected_plugin) {
                    ui.label("Color scheme:");

                    let mut current_scheme = self.state.selected_color_scheme.clone();

                    egui::ComboBox::from_id_salt("orb_color_scheme")
                        .width(200.0)
                        .selected_text(&current_scheme)
                        .show_ui(ui, |ui| {
                            for scheme in &style.color_schemes {
                                ui.selectable_value(
                                    &mut current_scheme,
                                    scheme.clone(),
                                    scheme.to_uppercase(),
                                );
                            }
                        });

                    if current_scheme != self.state.selected_color_scheme {
                        self.state.selected_color_scheme = current_scheme;
                        self.state.mark_changed();
                    }
                }
            }
        });
    }

    /// Show about section
    fn show_about_section(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("About", |ui| {
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("kVoice").size(18.0).strong());
                ui.label("Cross-platform speech-to-text");
                ui.label("Version 0.1.0");
                ui.add_space(5.0);
                ui.hyperlink_to("https://github.com/kvoice/kvoice", "https://github.com/kvoice/kvoice");
            });
        });
    }

    /// Show status message
    fn show_status_message(&mut self, ui: &mut egui::Ui) {
        if let Some(ref msg) = self.state.status_message {
            let color = if self.state.status_is_error {
                egui::Color32::RED
            } else {
                egui::Color32::DARK_GREEN
            };
            ui.label(egui::RichText::new(msg).color(color));
        }
    }

    /// Show save button
    fn show_save_button(&mut self, ui: &mut egui::Ui, app_state: &Arc<AppState>) {
        ui.horizontal_centered(|ui| {
            ui.add_space(10.0);

            // Show unsaved changes indicator
            if self.state.has_unsaved_changes {
                ui.label(egui::RichText::new("You have unsaved changes").color(egui::Color32::YELLOW));
                ui.add_space(10.0);
            }

            if ui.button("Save Settings").clicked() {
                self.save_settings(app_state);
            }
        });
    }

    /// Start model download
    fn start_model_download(&mut self, model: WhisperModel, app_state: &Arc<AppState>) {
        // Get runtime handle from AppState
        let runtime_handle = {
            let handle_guard = app_state.runtime_handle.read().unwrap();
            match handle_guard.as_ref() {
                Some(h) => h.clone(),
                None => {
                    log::error!("No runtime handle available for model download");
                    self.state.set_error("Internal error: runtime not available".to_string());
                    return;
                }
            }
        };

        // Reset shared state
        self.download_progress_shared.store(0, std::sync::atomic::Ordering::SeqCst);
        self.download_complete.store(false, std::sync::atomic::Ordering::SeqCst);
        {
            let mut error = self.download_error.write().unwrap();
            *error = None;
        }

        let app_state = app_state.clone();
        self.state.downloading_model = Some(model);
        self.state.download_progress = 0.0;
        self.state.set_status(format!("Downloading {} model...", model.name()));

        // Clone shared atomics for the async task
        let progress_shared = self.download_progress_shared.clone();
        let complete_shared = self.download_complete.clone();
        let error_shared = self.download_error.clone();

        // Spawn download task using runtime handle
        runtime_handle.spawn(async move {
            let engine = app_state.whisper_engine.lock().await;

            // Create progress callback that updates shared atomic
            let progress_clone = progress_shared.clone();
            let result = engine
                .download_model(model, Some(move |downloaded: u64, total: u64| {
                    let percent = if total > 0 {
                        ((downloaded as f64 / total as f64) * 100.0) as u32
                    } else {
                        0
                    };
                    progress_clone.store(percent, std::sync::atomic::Ordering::SeqCst);
                    log::debug!("Download progress: {}% ({}/{})", percent, downloaded, total);
                }))
                .await;

            match result {
                Ok(path) => {
                    log::info!("Model downloaded to {:?}", path);
                    progress_shared.store(100, std::sync::atomic::Ordering::SeqCst);
                }
                Err(e) => {
                    log::error!("Failed to download model: {}", e);
                    let mut error = error_shared.write().unwrap();
                    *error = Some(e.to_string());
                }
            }

            complete_shared.store(true, std::sync::atomic::Ordering::SeqCst);
        });
    }

    /// Save settings
    fn save_settings(&mut self, app_state: &Arc<AppState>) {
        // Update orb_style with color scheme
        if let Some(plugin_id) = &self.state.settings.orb_style {
            self.state.settings.orb_style =
                Some(format!("{}:{}", plugin_id, self.state.selected_color_scheme));
        }

        // Check if settings file already exists (warn before overwriting)
        let settings_path = crate::state::Settings::settings_path();
        let file_exists = settings_path.exists();

        if file_exists {
            // Show warning that existing settings will be overwritten
            self.state.set_status(
                "Warning: Overwriting existing settings. Click again to confirm.".to_string()
            );
            self.state.status_is_error = true;

            // For now, just log and continue. A proper modal dialog would require
            // passing DialogState to this method or using a callback approach.
            log::warn!("Overwriting existing settings at {:?}", settings_path);
        }

        // Get runtime handle from AppState
        let runtime_handle = {
            let handle_guard = app_state.runtime_handle.read().unwrap();
            match handle_guard.as_ref() {
                Some(h) => h.clone(),
                None => {
                    log::error!("No runtime handle available for saving settings");
                    self.state.set_error("Internal error: runtime not available".to_string());
                    return;
                }
            }
        };

        let settings = self.state.settings.clone();
        let app_state = app_state.clone();

        // Spawn save task using runtime handle
        runtime_handle.spawn(async move {
            let mut state_lock = app_state.settings.write().await;
            *state_lock = settings.clone();

            match state_lock.save() {
                Ok(()) => {
                    log::info!("Settings saved successfully");
                }
                Err(e) => {
                    log::error!("Failed to save settings: {}", e);
                }
            }
        });

        self.state.mark_saved();
        self.state.set_status("Settings saved!".to_string());
    }
}

impl Default for SettingsPanel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_state_default() {
        let state = SettingsState::default();
        assert!(!state.has_unsaved_changes);
        assert!(state.status_message.is_none());
        assert_eq!(state.selected_color_scheme, "purple");
    }

    #[test]
    fn test_settings_state_mark_changed() {
        let mut state = SettingsState::default();
        assert!(!state.has_unsaved_changes);
        state.mark_changed();
        assert!(state.has_unsaved_changes);
    }

    #[test]
    fn test_settings_state_mark_saved() {
        let mut state = SettingsState::default();
        state.mark_changed();
        state.mark_saved();
        assert!(!state.has_unsaved_changes);
    }

    #[test]
    fn test_settings_state_status_messages() {
        let mut state = SettingsState::default();

        state.set_status("Test message".to_string());
        assert_eq!(state.status_message, Some("Test message".to_string()));
        assert!(!state.status_is_error);

        state.set_error("Error message".to_string());
        assert_eq!(state.status_message, Some("Error message".to_string()));
        assert!(state.status_is_error);

        state.clear_status();
        assert!(state.status_message.is_none());
    }

    #[test]
    fn test_orb_style_discovery() {
        let styles = OrbStyle::discover_styles();
        assert!(!styles.is_empty());
        assert!(styles.iter().any(|s| s.id == "nebula-aura-gpu"));

        let nebula = styles.iter().find(|s| s.id == "nebula-aura-gpu").unwrap();
        assert!(nebula.color_schemes.contains(&"purple".to_string()));
        assert!(nebula.color_schemes.contains(&"cyan".to_string()));
        assert!(nebula.color_schemes.contains(&"fire".to_string()));
        assert!(nebula.color_schemes.contains(&"aurora".to_string()));
        assert!(nebula.color_schemes.contains(&"cosmic".to_string()));
    }

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

    #[test]
    fn test_parse_orb_style() {
        let state = SettingsState::default();

        // Test with scheme
        let (plugin, scheme) = state.parse_orb_style("nebula-aura-gpu:fire");
        assert_eq!(plugin, "nebula-aura-gpu");
        assert_eq!(scheme, "fire");

        // Test without scheme
        let (plugin, scheme) = state.parse_orb_style("nebula-aura");
        assert_eq!(plugin, "nebula-aura");
        assert_eq!(scheme, "purple"); // default
    }

    #[test]
    fn test_is_model_downloaded() {
        let mut state = SettingsState::default();
        state.downloaded_models = vec![WhisperModel::Tiny, WhisperModel::Small];

        assert!(state.is_model_downloaded(WhisperModel::Tiny));
        assert!(state.is_model_downloaded(WhisperModel::Small));
        assert!(!state.is_model_downloaded(WhisperModel::Base));
        assert!(!state.is_model_downloaded(WhisperModel::Medium));
        assert!(!state.is_model_downloaded(WhisperModel::Large));
    }

    #[test]
    fn test_settings_serialization() {
        let settings = Settings {
            whisper_model: Some("small".to_string()),
            audio_device: Some("Default".to_string()),
            orb_style: Some("nebula-aura-gpu:purple".to_string()),
            always_on_top: true,
            window_width: 500,
            window_height: 500,
        };

        // Test serialization
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("small"));
        assert!(json.contains("nebula-aura-gpu"));

        // Test deserialization
        let deserialized: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.whisper_model, settings.whisper_model);
        assert_eq!(deserialized.audio_device, settings.audio_device);
        assert_eq!(deserialized.orb_style, settings.orb_style);
        assert_eq!(deserialized.always_on_top, settings.always_on_top);
        assert_eq!(deserialized.window_width, settings.window_width);
        assert_eq!(deserialized.window_height, settings.window_height);
    }

    #[test]
    fn test_whisper_model_from_str() {
        assert_eq!(WhisperModel::from_str("tiny"), Some(WhisperModel::Tiny));
        assert_eq!(WhisperModel::from_str("base"), Some(WhisperModel::Base));
        assert_eq!(WhisperModel::from_str("small"), Some(WhisperModel::Small));
        assert_eq!(WhisperModel::from_str("medium"), Some(WhisperModel::Medium));
        assert_eq!(WhisperModel::from_str("large"), Some(WhisperModel::Large));
        assert_eq!(WhisperModel::from_str("large-v3"), Some(WhisperModel::Large));
        assert_eq!(WhisperModel::from_str("invalid"), None);
    }
}
