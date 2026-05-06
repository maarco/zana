//! Plugin Manager
//!
//! Handles plugin discovery, loading, unloading, and lifecycle management.

use super::hook_adapter::PluginHookAdapter;
use super::manifest::PluginManifest;
use super::registry::PluginRegistry;
use super::traits::Plugin;
use crate::hooks::{HookEvent, HookHandler, PluginType};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

/// Plugin loading error
#[derive(Debug, thiserror::Error)]
pub enum PluginLoadError {
    #[error("Plugin directory not found: {0}")]
    DirectoryNotFound(PathBuf),

    #[error("plugin.toml not found in: {0}")]
    ManifestNotFound(PathBuf),

    #[error("Failed to parse plugin.toml: {0}")]
    InvalidManifest(String),

    #[error("Plugin validation failed: {0}")]
    ValidationFailed(String),

    #[error("Plugin initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Plugin with ID '{0}' is already loaded")]
    AlreadyLoaded(String),
}

/// Manages plugin lifecycle
pub struct PluginManager {
    /// Plugin registry
    registry: Arc<RwLock<PluginRegistry>>,

    /// Event bus for emitting plugin events
    event_bus: Arc<crate::hooks::EventBus>,

    /// Plugins directory
    plugins_dir: PathBuf,

    /// Tracked plugin handler IDs (for hook unregister)
    handler_ids: Mutex<HashMap<String, Vec<String>>>,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new(
        registry: Arc<RwLock<PluginRegistry>>,
        event_bus: Arc<crate::hooks::EventBus>,
        plugins_dir: PathBuf,
    ) -> Self {
        Self {
            registry,
            event_bus,
            plugins_dir,
            handler_ids: Mutex::new(HashMap::new()),
        }
    }

    /// Load all plugins from the plugins directory
    ///
    /// Returns the number of successfully loaded plugins.
    pub async fn load_all(&self) -> Result<usize> {
        log::info!("Loading plugins from: {:?}", self.plugins_dir);

        // Check if plugins directory exists
        if !self.plugins_dir.exists() {
            log::warn!("Plugins directory does not exist: {:?}", self.plugins_dir);
            fs::create_dir_all(&self.plugins_dir).with_context(|| {
                format!("Failed to create plugins directory: {:?}", self.plugins_dir)
            })?;
            return Ok(0);
        }

        // Find all subdirectories containing plugin.toml
        let entries = fs::read_dir(&self.plugins_dir)
            .with_context(|| format!("Failed to read plugins directory: {:?}", self.plugins_dir))?;

        let mut loaded_count = 0;

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    log::warn!("Failed to read directory entry: {}", e);
                    continue;
                }
            };

            let path = entry.path();

            // Skip if not a directory
            if !path.is_dir() {
                continue;
            }

            // Try to load plugin from this directory
            match self.load_plugin(&path).await {
                Ok(_) => loaded_count += 1,
                Err(e) => {
                    log::warn!("Failed to load plugin from {:?}: {}", path, e);
                    // Continue loading other plugins
                }
            }
        }

        log::info!("Loaded {} plugin(s)", loaded_count);
        Ok(loaded_count)
    }

    /// Load a single plugin from a directory
    ///
    /// The directory must contain a valid plugin.toml file.
    pub async fn load_plugin(&self, plugin_dir: &Path) -> Result<()> {
        // Check if directory exists
        if !plugin_dir.exists() {
            return Err(PluginLoadError::DirectoryNotFound(plugin_dir.to_path_buf()).into());
        }

        // Find plugin.toml
        let manifest_path = plugin_dir.join("plugin.toml");
        if !manifest_path.exists() {
            return Err(PluginLoadError::ManifestNotFound(plugin_dir.to_path_buf()).into());
        }

        // Parse manifest
        let manifest = PluginManifest::from_file(&manifest_path)
            .with_context(|| format!("Failed to parse manifest: {:?}", manifest_path))?;

        // Validate manifest
        manifest.validate().with_context(|| {
            format!(
                "Plugin manifest validation failed for: {}",
                manifest.plugin.id
            )
        })?;

        let plugin_id = manifest.plugin.id.clone();

        // Check if already loaded
        {
            let registry = self.registry.read().await;
            if registry.has(&plugin_id) {
                return Err(PluginLoadError::AlreadyLoaded(plugin_id).into());
            }
        }

        log::info!(
            "Loading plugin: {} v{} ({:?})",
            plugin_id,
            manifest.plugin.version,
            manifest.plugin.plugin_type.kind
        );

        // Create plugin instance
        // TODO: For now, we create a placeholder plugin instance
        // In the future, this will dynamically load the actual plugin code
        let plugin = self.create_plugin_instance(&manifest, plugin_dir)?;

        // Register plugin
        {
            let mut registry = self.registry.write().await;
            registry.register(plugin.clone(), manifest.clone());
        }

        // Wrap plugin in hook adapter and register as handler
        let adapter = PluginHookAdapter::new(plugin.clone()).await;
        let handler_id = adapter.id().to_string();

        self.event_bus
            .register(Arc::new(adapter))
            .await
            .with_context(|| {
                format!("Failed to register hook handler for plugin: {}", plugin_id)
            })?;

        // Store handler ID for later unregister
        let mut handler_ids = self.handler_ids.lock().await;
        handler_ids
            .entry(plugin_id.clone())
            .or_insert_with(Vec::new)
            .push(handler_id.clone());

        log::debug!(
            "Registered hook handler: {} for plugin: {}",
            handler_id,
            plugin_id
        );

        // Emit PluginLoaded event
        let plugin_type = match manifest.plugin.plugin_type.kind {
            super::manifest::PluginKind::OrbStyle => PluginType::OrbStyle,
            super::manifest::PluginKind::AudioProcessor => PluginType::AudioProcessor,
            super::manifest::PluginKind::PostProcessor => PluginType::PostProcessor,
            super::manifest::PluginKind::Integration => PluginType::Integration,
        };

        let event = HookEvent::PluginLoaded {
            id: plugin_id.clone(),
            name: manifest.plugin.name.clone(),
            version: manifest.plugin.version.clone(),
            plugin_type,
        };

        self.event_bus.emit(event).await;

        log::info!("Successfully loaded plugin: {}", plugin_id);
        Ok(())
    }

    /// Unload a plugin by ID
    pub async fn unload_plugin(&self, id: &str) -> Result<()> {
        log::info!("Unloading plugin: {}", id);

        // Check if plugin exists
        let exists = {
            let registry = self.registry.read().await;
            registry.has(id)
        };

        if !exists {
            log::warn!("Plugin not found: {}", id);
            return Ok(());
        }

        // Unregister hook handlers
        let mut handler_ids = self.handler_ids.lock().await;
        if let Some(handlers) = handler_ids.remove(id) {
            for handler_id in handlers {
                if let Err(e) = self.event_bus.unregister(&handler_id).await {
                    log::warn!(
                        "Failed to unregister handler {} for plugin {}: {}",
                        handler_id,
                        id,
                        e
                    );
                }
            }
        }

        // Remove from registry
        let entry = {
            let mut registry = self.registry.write().await;
            registry.unregister(id)
        };

        if entry.is_some() {
            // Emit PluginUnloaded event
            let event = HookEvent::PluginUnloaded { id: id.to_string() };
            self.event_bus.emit(event).await;

            log::info!("Successfully unloaded plugin: {}", id);
        }

        Ok(())
    }

    /// Reload a plugin by ID
    ///
    /// Unloads and then reloads the plugin from disk.
    pub async fn reload_plugin(&self, id: &str) -> Result<()> {
        log::info!("Reloading plugin: {}", id);

        // Get plugin manifest before unloading
        let plugin_dir = {
            let registry = self.registry.read().await;
            let entry = registry.get(id);
            if let Some(entry) = entry {
                // Reconstruct the plugin directory path
                self.plugins_dir.join(&entry.manifest.plugin.id)
            } else {
                return Err(anyhow::anyhow!("Plugin not found: {}", id));
            }
        };

        // Unload the plugin
        self.unload_plugin(id).await?;

        // Load it again
        self.load_plugin(&plugin_dir).await?;

        log::info!("Successfully reloaded plugin: {}", id);
        Ok(())
    }

    /// Get all plugin manifests
    pub async fn get_manifests(&self) -> Vec<PluginManifest> {
        let registry = self.registry.read().await;
        registry.manifests().into_iter().cloned().collect()
    }

    /// Create a plugin instance from manifest
    ///
    /// This is a placeholder implementation that creates a minimal plugin wrapper.
    /// In the future, this will dynamically load the actual plugin code.
    fn create_plugin_instance(
        &self,
        manifest: &PluginManifest,
        plugin_dir: &Path,
    ) -> Result<Arc<RwLock<dyn Plugin>>> {
        // For now, create a placeholder plugin that stores the manifest
        // This allows the plugin system to function without dynamic code loading
        let plugin = PlaceholderPlugin::new(manifest.clone(), plugin_dir.to_path_buf());
        Ok(Arc::new(RwLock::new(plugin)))
    }
}

/// Placeholder plugin implementation
///
/// This is a minimal implementation that stores the manifest and provides
/// basic functionality. In the future, this will be replaced with dynamically
/// loaded plugin code.
struct PlaceholderPlugin {
    manifest: PluginManifest,
    _plugin_dir: PathBuf, // Reserved for future dynamic plugin loading
    config: std::collections::HashMap<String, serde_json::Value>,
}

impl PlaceholderPlugin {
    fn new(manifest: PluginManifest, plugin_dir: PathBuf) -> Self {
        let config = manifest.default_config();
        Self {
            manifest,
            _plugin_dir: plugin_dir,
            config,
        }
    }
}

#[async_trait::async_trait]
impl Plugin for PlaceholderPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn init(&mut self, _context: super::traits::PluginContext) -> Result<()> {
        log::debug!(
            "Initializing placeholder plugin: {}",
            self.manifest.plugin.id
        );
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        log::debug!(
            "Shutting down placeholder plugin: {}",
            self.manifest.plugin.id
        );
        Ok(())
    }

    fn on_config_change(&mut self, config: &std::collections::HashMap<String, serde_json::Value>) {
        self.config = config.clone();
        log::debug!("Config changed for plugin: {}", self.manifest.plugin.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::create_dir_all;
    use tempfile::TempDir;

    fn create_test_plugin(dir: &Path, id: &str, name: &str) -> PathBuf {
        let plugin_dir = dir.join(id);
        create_dir_all(&plugin_dir).unwrap();

        let manifest_content = format!(
            r#"
[plugin]
id = "{}"
name = "{}"
version = "1.0.0"
description = "Test plugin"
author = "Test Author"

[plugin.type]
kind = "orb-style"

[plugin.capabilities]
audio_level = true
audio_fft = true
"#,
            id, name
        );

        fs::write(plugin_dir.join("plugin.toml"), manifest_content).unwrap();
        plugin_dir
    }

    #[tokio::test]
    async fn test_load_all_plugins() {
        let temp_dir = TempDir::new().unwrap();
        let plugins_dir = temp_dir.path().join("plugins");
        create_dir_all(&plugins_dir).unwrap();

        // Create test plugins
        create_test_plugin(&plugins_dir, "test-plugin-1", "Test Plugin 1");
        create_test_plugin(&plugins_dir, "test-plugin-2", "Test Plugin 2");

        // Create manager
        let registry = Arc::new(RwLock::new(PluginRegistry::new()));
        let event_bus = Arc::new(crate::hooks::EventBus::new());
        let manager = PluginManager::new(registry, event_bus, plugins_dir);

        // Load all plugins
        let count = manager.load_all().await.unwrap();
        assert_eq!(count, 2);

        // Verify plugins are loaded
        let manifests = manager.get_manifests().await;
        assert_eq!(manifests.len(), 2);
    }

    #[tokio::test]
    async fn test_load_single_plugin() {
        let temp_dir = TempDir::new().unwrap();
        let plugins_dir = temp_dir.path().join("plugins");
        create_dir_all(&plugins_dir).unwrap();

        let plugin_dir = create_test_plugin(&plugins_dir, "test-plugin", "Test Plugin");

        // Create manager
        let registry = Arc::new(RwLock::new(PluginRegistry::new()));
        let event_bus = Arc::new(crate::hooks::EventBus::new());
        let manager = PluginManager::new(registry, event_bus, plugins_dir);

        // Load plugin
        manager.load_plugin(&plugin_dir).await.unwrap();

        // Verify plugin is loaded
        let manifests = manager.get_manifests().await;
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].plugin.id, "test-plugin");
    }

    #[tokio::test]
    async fn test_unload_plugin() {
        let temp_dir = TempDir::new().unwrap();
        let plugins_dir = temp_dir.path().join("plugins");
        create_dir_all(&plugins_dir).unwrap();

        let plugin_dir = create_test_plugin(&plugins_dir, "test-plugin", "Test Plugin");

        // Create manager
        let registry = Arc::new(RwLock::new(PluginRegistry::new()));
        let event_bus = Arc::new(crate::hooks::EventBus::new());
        let manager = PluginManager::new(registry, event_bus, plugins_dir);

        // Load plugin
        manager.load_plugin(&plugin_dir).await.unwrap();
        assert_eq!(manager.get_manifests().await.len(), 1);

        // Unload plugin
        manager.unload_plugin("test-plugin").await.unwrap();
        assert_eq!(manager.get_manifests().await.len(), 0);
    }

    #[tokio::test]
    async fn test_reload_plugin() {
        let temp_dir = TempDir::new().unwrap();
        let plugins_dir = temp_dir.path().join("plugins");
        create_dir_all(&plugins_dir).unwrap();

        let plugin_dir = create_test_plugin(&plugins_dir, "test-plugin", "Test Plugin");

        // Create manager
        let registry = Arc::new(RwLock::new(PluginRegistry::new()));
        let event_bus = Arc::new(crate::hooks::EventBus::new());
        let manager = PluginManager::new(registry, event_bus, plugins_dir);

        // Load plugin
        manager.load_plugin(&plugin_dir).await.unwrap();
        assert_eq!(manager.get_manifests().await.len(), 1);

        // Reload plugin
        manager.reload_plugin("test-plugin").await.unwrap();
        assert_eq!(manager.get_manifests().await.len(), 1);
    }
}
