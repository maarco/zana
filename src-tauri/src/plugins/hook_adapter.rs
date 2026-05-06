//! Plugin Hook Adapter
//!
//! Adapts the Plugin trait to the HookHandler trait, automatically
//! registering all plugins as hook handlers in the event system.

use super::traits::Plugin;
use crate::hooks::{
    HookEvent, HookEventType, HookHandler, HookResult, PluginType, Theme, TranscriptionSegmentData,
};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Adapts a Plugin to HookHandler trait
///
/// This adapter wraps any plugin and makes it a hook handler,
/// allowing plugins to receive and respond to hook events.
pub struct PluginHookAdapter {
    /// The wrapped plugin
    plugin: Arc<RwLock<dyn Plugin>>,

    /// Handler ID (generated from plugin ID)
    id: String,

    /// Human-readable name
    name: String,

    /// Priority (plugins default to 100-200 range)
    priority: i32,

    /// Event subscriptions
    subscriptions: Vec<HookEventType>,
}

impl PluginHookAdapter {
    /// Create a new adapter wrapping a plugin
    pub async fn new(plugin: Arc<RwLock<dyn Plugin>>) -> Self {
        // Get plugin metadata
        let plugin_guard = plugin.read().await;
        let manifest = plugin_guard.manifest();
        let id = manifest.plugin.id.clone();
        let name = manifest.plugin.name.clone();

        // Determine priority based on plugin type
        let priority = match manifest.plugin.plugin_type.kind {
            super::manifest::PluginKind::OrbStyle => 100,
            super::manifest::PluginKind::AudioProcessor => 110,
            super::manifest::PluginKind::PostProcessor => 120,
            super::manifest::PluginKind::Integration => 130,
        };

        // Determine subscriptions based on capabilities
        let mut subscriptions = Vec::new();
        let caps = &manifest.plugin.capabilities;

        // Audio events
        if caps.audio_level {
            subscriptions.push(HookEventType::AudioLevelChange);
        }
        if caps.audio_fft {
            subscriptions.push(HookEventType::AudioFftReady);
        }
        if caps.audio_buffer {
            subscriptions.push(HookEventType::AudioBufferReady);
        }

        // Transcription events
        if caps.transcription_events {
            subscriptions.extend([
                HookEventType::TranscriptionStart,
                HookEventType::TranscriptionSegment,
                HookEventType::TranscriptionComplete,
            ]);
        }

        // Settings events
        if caps.settings_read || caps.settings_write {
            subscriptions.extend([
                HookEventType::SettingChanged,
                HookEventType::ProfileChanged,
                HookEventType::ModelChanged,
            ]);
        }

        // Always subscribe to plugin lifecycle events
        subscriptions.extend([
            HookEventType::PluginEnabled,
            HookEventType::PluginDisabled,
            HookEventType::PluginConfigChanged,
        ]);
        drop(plugin_guard);

        Self {
            plugin,
            id,
            name,
            priority,
            subscriptions,
        }
    }

    /// Create adapter with custom priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Create adapter with custom subscriptions
    pub fn with_subscriptions(mut self, subscriptions: Vec<HookEventType>) -> Self {
        self.subscriptions = subscriptions;
        self
    }

    // =========================================================================
    // Audio Event Handlers
    // =========================================================================

    async fn on_audio_level(&self, level: f32, peak: f32) -> HookResult {
        log::trace!("Plugin {}: Audio level {} (peak {})", self.id, level, peak);
        HookResult::Continue
    }

    async fn on_audio_fft(&self, _bins: &[f32], bin_count: usize) -> HookResult {
        log::trace!("Plugin {}: FFT data ready ({} bins)", self.id, bin_count);
        HookResult::Continue
    }

    async fn on_audio_buffer(
        &self,
        sample_count: usize,
        sample_rate: u32,
        channels: u16,
    ) -> HookResult {
        log::trace!(
            "Plugin {}: Audio buffer ready ({} samples, {}Hz, {}ch)",
            self.id,
            sample_count,
            sample_rate,
            channels
        );
        HookResult::Continue
    }

    // =========================================================================
    // Transcription Event Handlers
    // =========================================================================

    async fn on_transcription_start(&self, model: &str, audio_duration_ms: u64) -> HookResult {
        log::debug!(
            "Plugin {}: Transcription started with model '{}' ({}ms audio)",
            self.id,
            model,
            audio_duration_ms
        );
        HookResult::Continue
    }

    async fn on_transcription_progress(&self, percent: f32) -> HookResult {
        log::trace!("Plugin {}: Transcription progress: {}%", self.id, percent);
        HookResult::Continue
    }

    async fn on_transcription_segment(&self, start_ms: i64, end_ms: i64, text: &str) -> HookResult {
        log::trace!(
            "Plugin {}: Transcription segment [{}-{}ms]: {}",
            self.id,
            start_ms,
            end_ms,
            text
        );
        HookResult::Continue
    }

    async fn on_transcription_complete(
        &self,
        text: &str,
        segments: &[TranscriptionSegmentData],
        processing_ms: u64,
    ) -> HookResult {
        log::debug!(
            "Plugin {}: Transcription complete ({} chars, {} segments, {}ms processing)",
            self.id,
            text.len(),
            segments.len(),
            processing_ms
        );
        HookResult::Continue
    }

    async fn on_transcription_error(&self, error: &str) -> HookResult {
        log::warn!("Plugin {}: Transcription error: {}", self.id, error);
        HookResult::Continue
    }

    // =========================================================================
    // Plugin Event Handlers
    // =========================================================================

    async fn on_plugin_loaded(
        &self,
        _id: &str,
        name: &str,
        version: &str,
        plugin_type: &PluginType,
    ) -> HookResult {
        log::debug!(
            "Plugin {}: Plugin loaded: {} v{} ({:?})",
            self.id,
            name,
            version,
            plugin_type
        );
        HookResult::Continue
    }

    async fn on_plugin_unloaded(&self, id: &str) -> HookResult {
        log::debug!("Plugin {}: Plugin unloaded: {}", self.id, id);
        HookResult::Continue
    }

    async fn on_plugin_error(&self, id: &str, error: &str) -> HookResult {
        log::warn!("Plugin {}: Plugin {} error: {}", self.id, id, error);
        HookResult::Continue
    }

    async fn on_plugin_config_changed(
        &self,
        id: &str,
        key: &str,
        value: &serde_json::Value,
    ) -> HookResult {
        log::debug!(
            "Plugin {}: Plugin {} config changed: {} = {:?}",
            self.id,
            id,
            key,
            value
        );

        // If this is our config change, notify the plugin
        if self.id == id || self.id.ends_with(&format!("-adapter-{}", id)) {
            let mut plugin = self.plugin.write().await;
            let mut config = std::collections::HashMap::new();
            config.insert(key.to_string(), value.clone());
            plugin.on_config_change(&config);
        }

        HookResult::Continue
    }

    async fn on_plugin_enabled(&self, id: &str) -> HookResult {
        log::debug!("Plugin {}: Plugin enabled: {}", self.id, id);
        HookResult::Continue
    }

    async fn on_plugin_disabled(&self, id: &str) -> HookResult {
        log::debug!("Plugin {}: Plugin disabled: {}", self.id, id);
        HookResult::Continue
    }

    // =========================================================================
    // UI Event Handlers
    // =========================================================================

    async fn on_orb_style_changed(
        &self,
        previous_style: Option<&str>,
        new_style: &str,
    ) -> HookResult {
        log::debug!(
            "Plugin {}: Orb style changed: {:?} -> {}",
            self.id,
            previous_style,
            new_style
        );
        HookResult::Continue
    }

    async fn on_theme_changed(&self, theme: &Theme) -> HookResult {
        log::trace!("Plugin {}: Theme changed to {:?}", self.id, theme);
        HookResult::Continue
    }

    async fn on_window_resized(&self, width: u32, height: u32) -> HookResult {
        log::trace!("Plugin {}: Window resized to {}x{}", self.id, width, height);
        HookResult::Continue
    }

    async fn on_record_button_pressed(&self) -> HookResult {
        log::debug!("Plugin {}: Record button pressed", self.id);
        HookResult::Continue
    }

    async fn on_settings_opened(&self) -> HookResult {
        log::trace!("Plugin {}: Settings opened", self.id);
        HookResult::Continue
    }

    async fn on_settings_closed(&self) -> HookResult {
        log::trace!("Plugin {}: Settings closed", self.id);
        HookResult::Continue
    }

    // =========================================================================
    // Settings Event Handlers
    // =========================================================================

    async fn on_setting_changed(
        &self,
        key: &str,
        old_value: &Option<serde_json::Value>,
        new_value: &serde_json::Value,
    ) -> HookResult {
        log::debug!(
            "Plugin {}: Setting changed: {} = {:?} (was {:?})",
            self.id,
            key,
            new_value,
            old_value
        );
        HookResult::Continue
    }

    async fn on_profile_changed(
        &self,
        previous_profile: Option<&str>,
        new_profile: &str,
    ) -> HookResult {
        log::debug!(
            "Plugin {}: Profile changed: {:?} -> {}",
            self.id,
            previous_profile,
            new_profile
        );
        HookResult::Continue
    }

    async fn on_model_changed(&self, previous_model: Option<&str>, new_model: &str) -> HookResult {
        log::debug!(
            "Plugin {}: Model changed: {:?} -> {}",
            self.id,
            previous_model,
            new_model
        );
        HookResult::Continue
    }

    // =========================================================================
    // System Event Handlers
    // =========================================================================

    async fn on_app_started(&self) -> HookResult {
        log::debug!("Plugin {}: Application started", self.id);
        HookResult::Continue
    }

    async fn on_app_shutdown(&self) -> HookResult {
        log::debug!("Plugin {}: Application shutting down", self.id);
        HookResult::Continue
    }

    async fn on_error(&self, code: &str, message: &str) -> HookResult {
        log::warn!("Plugin {}: Error [{}]: {}", self.id, code, message);
        HookResult::Continue
    }
}

#[async_trait]
impl HookHandler for PluginHookAdapter {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn priority(&self) -> i32 {
        self.priority
    }

    fn subscribed_events(&self) -> Vec<HookEventType> {
        self.subscriptions.clone()
    }

    async fn handle(&self, event: &mut HookEvent) -> HookResult {
        // Match event to plugin callback
        match event {
            // Audio Events
            HookEvent::AudioCaptureStart {
                device_id,
                sample_rate,
                channels,
            } => {
                log::trace!(
                    "Plugin {}: Audio capture started on {} ({}Hz, {}ch)",
                    self.id,
                    device_id,
                    sample_rate,
                    channels
                );
                HookResult::Continue
            }

            HookEvent::AudioCaptureStop { duration_ms } => {
                log::trace!(
                    "Plugin {}: Audio capture stopped after {}ms",
                    self.id,
                    duration_ms
                );
                HookResult::Continue
            }

            HookEvent::AudioLevelChange { level, peak } => self.on_audio_level(*level, *peak).await,

            HookEvent::AudioFftReady { bins, bin_count } => {
                self.on_audio_fft(bins, *bin_count).await
            }

            HookEvent::AudioBufferReady {
                sample_count,
                sample_rate,
                channels,
            } => {
                self.on_audio_buffer(*sample_count, *sample_rate, *channels)
                    .await
            }

            // Transcription Events
            HookEvent::TranscriptionStart {
                model,
                audio_duration_ms,
            } => self.on_transcription_start(model, *audio_duration_ms).await,

            HookEvent::TranscriptionProgress { percent } => {
                self.on_transcription_progress(*percent).await
            }

            HookEvent::TranscriptionSegment {
                start_ms,
                end_ms,
                text,
            } => {
                self.on_transcription_segment(*start_ms, *end_ms, text)
                    .await
            }

            HookEvent::TranscriptionComplete {
                text,
                segments,
                processing_ms,
            } => {
                self.on_transcription_complete(text, segments, *processing_ms)
                    .await
            }

            HookEvent::TranscriptionError { error } => self.on_transcription_error(error).await,

            // Plugin Events
            HookEvent::PluginLoaded {
                id,
                name,
                version,
                plugin_type,
            } => {
                // Don't handle our own load event
                if self.id.as_str() == id || self.id.ends_with(&format!("-adapter-{}", id)) {
                    HookResult::Skip
                } else {
                    self.on_plugin_loaded(id, name, version, plugin_type).await
                }
            }

            HookEvent::PluginUnloaded { id } => self.on_plugin_unloaded(id).await,

            HookEvent::PluginError { id, error } => self.on_plugin_error(id, error).await,

            HookEvent::PluginConfigChanged { id, key, value } => {
                self.on_plugin_config_changed(id, key, value).await
            }

            HookEvent::PluginEnabled { id } => self.on_plugin_enabled(id).await,

            HookEvent::PluginDisabled { id } => self.on_plugin_disabled(id).await,

            // UI Events
            HookEvent::OrbStyleChanged {
                previous_style,
                new_style,
            } => {
                self.on_orb_style_changed(previous_style.as_deref(), new_style)
                    .await
            }

            HookEvent::ThemeChanged { theme } => self.on_theme_changed(theme).await,

            HookEvent::WindowResized { width, height } => {
                self.on_window_resized(*width, *height).await
            }

            HookEvent::RecordButtonPressed => self.on_record_button_pressed().await,

            HookEvent::SettingsOpened => self.on_settings_opened().await,

            HookEvent::SettingsClosed => self.on_settings_closed().await,

            // Settings Events
            HookEvent::SettingChanged {
                key,
                old_value,
                new_value,
            } => self.on_setting_changed(key, old_value, new_value).await,

            HookEvent::ProfileChanged {
                previous_profile,
                new_profile,
            } => {
                self.on_profile_changed(previous_profile.as_deref(), new_profile)
                    .await
            }

            HookEvent::ModelChanged {
                previous_model,
                new_model,
            } => {
                self.on_model_changed(previous_model.as_deref(), new_model)
                    .await
            }

            // System Events
            HookEvent::AppStarted => self.on_app_started().await,

            HookEvent::AppShutdown => self.on_app_shutdown().await,

            HookEvent::Error { code, message } => self.on_error(code, message).await,
        }
    }

    fn is_enabled(&self) -> bool {
        // Plugins are enabled when registered
        // This can be enhanced to check plugin manifest
        true
    }
}

impl std::fmt::Debug for PluginHookAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginHookAdapter")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("priority", &self.priority)
            .field("subscriptions", &self.subscriptions.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::manifest::{
        PluginCapabilities, PluginKind, PluginManifest, PluginMeta, PluginTypeMeta,
    };

    struct TestPlugin {
        manifest: PluginManifest,
    }

    #[async_trait]
    impl Plugin for TestPlugin {
        fn manifest(&self) -> &PluginManifest {
            &self.manifest
        }

        async fn init(
            &mut self,
            _context: super::super::traits::PluginContext,
        ) -> anyhow::Result<()> {
            Ok(())
        }

        async fn shutdown(&mut self) -> anyhow::Result<()> {
            Ok(())
        }

        fn on_config_change(
            &mut self,
            _config: &std::collections::HashMap<String, serde_json::Value>,
        ) {
        }
    }

    fn create_test_manifest(id: &str, name: &str) -> PluginManifest {
        let capabilities = PluginCapabilities {
            audio_level: true,
            audio_fft: true,
            settings_read: true,
            ..PluginCapabilities::default()
        };

        PluginManifest {
            plugin: PluginMeta {
                id: id.to_string(),
                name: name.to_string(),
                version: "1.0.0".to_string(),
                description: "Test plugin".to_string(),
                author: "Test Author".to_string(),
                license: Some("MIT".to_string()),
                homepage: None,
                plugin_type: PluginTypeMeta {
                    kind: PluginKind::OrbStyle,
                },
                capabilities,
                ui: None,
                config: None,
                dev: None,
            },
        }
    }

    #[tokio::test]
    async fn test_adapter_creation() {
        let manifest = create_test_manifest("test-plugin", "Test Plugin");
        let plugin = TestPlugin { manifest };

        let plugin_arc = Arc::new(RwLock::new(plugin));
        let adapter = PluginHookAdapter::new(plugin_arc).await;

        assert_eq!(adapter.id(), "test-plugin");
        assert_eq!(adapter.name(), "Test Plugin");
        assert_eq!(adapter.priority(), 100); // OrbStyle priority
    }

    #[tokio::test]
    async fn test_adapter_subscriptions() {
        let manifest = create_test_manifest("test-plugin", "Test Plugin");
        let plugin = TestPlugin { manifest };

        let plugin_arc = Arc::new(RwLock::new(plugin));
        let adapter = PluginHookAdapter::new(plugin_arc).await;

        let subs = adapter.subscribed_events();
        assert!(subs.contains(&HookEventType::AudioLevelChange));
        assert!(subs.contains(&HookEventType::AudioFftReady));
        assert!(subs.contains(&HookEventType::SettingChanged));
    }

    #[tokio::test]
    async fn test_adapter_handle() {
        let manifest = create_test_manifest("test-plugin", "Test Plugin");
        let plugin = TestPlugin { manifest };

        let plugin_arc = Arc::new(RwLock::new(plugin));
        let adapter = PluginHookAdapter::new(plugin_arc).await;

        let mut event = HookEvent::AudioLevelChange {
            level: 0.5,
            peak: 0.8,
        };
        let result = adapter.handle(&mut event).await;

        assert_eq!(result, HookResult::Continue);
    }
}
