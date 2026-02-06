//! Plugin Loader
//!
//! Discovers, validates, and loads plugins from the plugins directory.

use super::manifest::PluginManifest;
use super::registry::PluginRegistry;
use super::traits::Plugin;
use crate::hooks::EventBus;
use async_trait::async_trait;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Result of plugin discovery
#[derive(Debug)]
pub struct DiscoveredPlugin {
    /// Path to plugin directory
    pub plugin_dir: PathBuf,
    /// Path to plugin manifest
    pub manifest_path: PathBuf,
    /// Parsed manifest
    pub manifest: PluginManifest,
}

/// Plugin loading error
#[derive(Debug, thiserror::Error)]
pub enum PluginLoadError {
    #[error("Plugin directory not found: {0}\n\nHow to fix:\n  - Create the plugins directory\n  - Or reinstall the application")]
    DirectoryNotFound(PathBuf),

    #[error("Failed to read plugin directory '{path}': {reason}\n\nHow to fix:\n  - Check directory permissions\n  - Ensure the directory exists")]
    DirectoryReadError { path: PathBuf, reason: String },

    #[error("Plugin '{plugin_id}': Manifest not found at {path}\n\nHow to fix:\n  - Ensure plugin.toml exists in the plugin directory\n  - Verify the plugin is properly installed")]
    ManifestNotFound { plugin_id: String, path: PathBuf },

    #[error("Plugin '{plugin_id}': Failed to parse manifest: {reason}\n\nHow to fix:\n  - Check that plugin.toml is valid TOML format\n  - Verify all required fields are present\n  - See the plugin documentation for the correct format")]
    ManifestParseError { plugin_id: String, reason: String },

    #[error("Plugin '{plugin_id}': Invalid manifest: {reason}\n\nHow to fix:\n  - Fix the validation error in plugin.toml\n  - Ensure all required fields have valid values")]
    ValidationError { plugin_id: String, reason: String },

    #[error("Plugin '{plugin_id}': Instantiation failed: {reason}\n\nHow to fix:\n  - Ensure the plugin is compatible with this version\n  - Check the plugin logs for more details\n  - Contact the plugin author")]
    InstantiationFailed { plugin_id: String, reason: String },

    #[error("Plugin '{plugin_id}' is disabled.\n\nHow to fix:\n  - Enable the plugin in settings\n  - Or remove the plugin from the plugins directory")]
    PluginDisabled { plugin_id: String },

    #[error("Plugin '{plugin_id}': Dependencies not met: {dependencies}\n\nHow to fix:\n  - Install the required dependencies\n  - Check the plugin documentation for requirements")]
    DependenciesNotMet { plugin_id: String, dependencies: String },

    #[error("Plugin '{plugin_id}': Initialization failed: {reason}\n\nHow to fix:\n  - Check the plugin configuration\n  - See the plugin documentation for setup instructions\n  - Review the application logs for details")]
    InitFailed { plugin_id: String, reason: String },
}

/// Plugin loader that discovers and loads plugins
pub struct PluginLoader {
    /// Plugins directory
    plugins_dir: PathBuf,
    /// Event bus for plugins
    event_bus: Arc<EventBus>,
    /// Base data directory for plugins
    data_dir: PathBuf,
}

impl PluginLoader {
    /// Create a new plugin loader
    pub fn new(
        plugins_dir: impl AsRef<Path>,
        event_bus: Arc<EventBus>,
        data_dir: impl AsRef<Path>,
    ) -> Self {
        let plugins_dir = plugins_dir.as_ref().to_path_buf();
        let data_dir = data_dir.as_ref().to_path_buf();

        log::debug!("Creating PluginLoader: plugins_dir={:?}, data_dir={:?}", plugins_dir, data_dir);

        Self {
            plugins_dir,
            event_bus,
            data_dir,
        }
    }

    /// Discover all plugins in the plugins directory
    ///
    /// Returns a list of discovered plugins with their manifests.
    pub fn discover(&self) -> Result<Vec<DiscoveredPlugin>, PluginLoadError> {
        log::debug!("Discovering plugins in {:?}", self.plugins_dir);
        let mut discovered = Vec::new();

        // Check if plugins directory exists
        if !self.plugins_dir.exists() {
            log::warn!(
                "Plugins directory does not exist: {:?}",
                self.plugins_dir
            );
            return Ok(discovered);
        }

        // Read plugin directories
        let entries = fs::read_dir(&self.plugins_dir).map_err(|e| {
            log::error!("Failed to read plugins directory {:?}: {}", self.plugins_dir, e);
            PluginLoadError::DirectoryReadError {
                path: self.plugins_dir.clone(),
                reason: format!("Failed to read directory: {}", e),
            }
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                log::error!("Failed to read directory entry: {}", e);
                PluginLoadError::DirectoryReadError {
                    path: self.plugins_dir.clone(),
                    reason: format!("Failed to read directory entry: {}", e),
                }
            })?;

            let path = entry.path();

            // Skip files, only process directories
            if !path.is_dir() {
                log::trace!("Skipping file: {:?}", path);
                continue;
            }

            log::trace!("Checking directory: {:?}", path);

            // Look for plugin.toml
            let manifest_path = path.join("plugin.toml");
            if !manifest_path.exists() {
                log::debug!("No plugin.toml found in {:?}, skipping", path);
                continue;
            }

            log::debug!("Found plugin.toml at {:?}", manifest_path);

            // Load manifest
            let manifest = PluginManifest::from_file(&manifest_path).map_err(|e| {
                let plugin_id = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                log::error!("Failed to parse manifest for plugin '{}': {}", plugin_id, e);
                PluginLoadError::ManifestParseError {
                    plugin_id,
                    reason: format!("Failed to parse plugin.toml: {}", e),
                }
            })?;

            let plugin_id = manifest.plugin.id.clone();

            // Validate manifest
            manifest.validate().map_err(|e| {
                log::error!("Failed to validate manifest for plugin '{}': {}", plugin_id, e);
                PluginLoadError::ValidationError {
                    plugin_id: plugin_id.clone(),
                    reason: format!("Validation failed: {}", e),
                }
            })?;

            log::info!(
                "Discovered plugin: {} v{} ({})",
                manifest.plugin.id,
                manifest.plugin.version,
                manifest.plugin.name
            );

            discovered.push(DiscoveredPlugin {
                plugin_dir: path,
                manifest_path,
                manifest,
            });
        }

        log::info!("Discovered {} plugin(s)", discovered.len());
        Ok(discovered)
    }

    /// Validate a plugin's dependencies
    ///
    /// Currently this is a placeholder for future dependency checking.
    fn validate_dependencies(&self, _manifest: &PluginManifest) -> Result<(), PluginLoadError> {
        // TODO: Implement dependency checking when plugin dependencies are defined
        Ok(())
    }

    /// Load a single plugin
    ///
    /// This creates a plugin instance and registers it in the registry.
    pub async fn load_plugin(
        &self,
        discovered: DiscoveredPlugin,
        registry: &mut PluginRegistry,
    ) -> Result<(), PluginLoadError> {
        let manifest = &discovered.manifest;
        let plugin_id = &manifest.plugin.id;

        log::debug!("Loading plugin: {} ({})", plugin_id, manifest.plugin.name);

        // Validate dependencies
        self.validate_dependencies(manifest)?;

        // Check if plugin already registered
        if registry.has(plugin_id) {
            log::warn!("Plugin {} already registered, skipping", plugin_id);
            return Ok(());
        }

        // Create plugin instance based on type
        log::trace!("Instantiating plugin: {}", plugin_id);
        let plugin = self.instantiate_plugin(&discovered).await?;

        // Register plugin
        log::trace!("Registering plugin: {}", plugin_id);
        registry.register(plugin, manifest.clone());

        log::info!("Successfully loaded plugin: {} v{}", plugin_id, manifest.plugin.version);
        Ok(())
    }

    /// Instantiate a plugin based on its manifest
    ///
    /// Currently only supports built-in GPU plugins through mock implementations.
    /// Future: Load dynamic libraries or WASM modules.
    async fn instantiate_plugin(
        &self,
        discovered: &DiscoveredPlugin,
    ) -> Result<Arc<RwLock<dyn Plugin>>, PluginLoadError> {
        let manifest = &discovered.manifest;

        log::trace!(
            "Instantiating plugin '{}' of type {:?}",
            manifest.plugin.id,
            manifest.plugin.plugin_type.kind
        );

        // For now, create a mock plugin instance
        // In the future, this would:
        // 1. Load dynamic library (.so, .dll, .dylib)
        // 2. Load WASM module
        // 3. Connect to external plugin process
        let plugin = MockPlugin::new(manifest.clone());

        log::trace!("Plugin '{}' instantiated successfully", manifest.plugin.id);
        Ok(Arc::new(RwLock::new(plugin)))
    }

    /// Load all discovered plugins
    ///
    /// Returns the number of successfully loaded plugins.
    pub async fn load_all(
        &self,
        registry: &mut PluginRegistry,
    ) -> Result<usize, PluginLoadError> {
        log::info!("Loading all plugins from {:?}", self.plugins_dir);

        let discovered = self.discover()?;

        let mut loaded_count = 0;
        let total = discovered.len();

        if total == 0 {
            log::warn!("No plugins discovered in {:?}", self.plugins_dir);
            return Ok(0);
        }

        log::info!("Found {} plugin(s) to load", total);

        for discovery in discovered {
            let plugin_id = discovery.manifest.plugin.id.clone();
            match self.load_plugin(discovery, registry).await {
                Ok(()) => {
                    loaded_count += 1;
                    log::debug!("Successfully loaded plugin {}", plugin_id);
                }
                Err(e) => {
                    log::error!("Failed to load plugin '{}': {}", plugin_id, e);
                    // Continue loading other plugins
                }
            }
        }

        if loaded_count == total {
            log::info!("Successfully loaded all {} plugin(s)", loaded_count);
        } else {
            log::warn!("Loaded {} out of {} plugins ({} failed)", loaded_count, total, total - loaded_count);
        }

        Ok(loaded_count)
    }

    /// Get plugin data directory
    pub fn plugin_data_dir(&self, plugin_id: &str) -> PathBuf {
        self.data_dir.join(plugin_id)
    }
}

/// Mock plugin for testing
///
/// This will be replaced with actual plugin loading in the future.
pub struct MockPlugin {
    manifest: PluginManifest,
    config: HashMap<String, serde_json::Value>,
}

impl MockPlugin {
    fn new(manifest: PluginManifest) -> Self {
        log::trace!("Creating MockPlugin instance for {}", manifest.plugin.id);
        let config = manifest.default_config();
        Self { manifest, config }
    }
}

#[async_trait]
impl Plugin for MockPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn init(&mut self, _context: super::traits::PluginContext) -> anyhow::Result<()> {
        log::info!("Initializing plugin: {}", self.manifest.plugin.id);
        Ok(())
    }

    async fn shutdown(&mut self) -> anyhow::Result<()> {
        log::info!("Shutting down plugin: {}", self.manifest.plugin.id);
        Ok(())
    }

    fn on_config_change(&mut self, config: &HashMap<String, serde_json::Value>) {
        log::debug!("Plugin {} config changed: {} keys", self.manifest.plugin.id, config.len());
        self.config = config.clone();
    }
}

/// Plugin loading status
#[derive(Debug, Clone)]
pub struct PluginLoadStatus {
    /// Total plugins discovered
    pub discovered: usize,
    /// Successfully loaded
    pub loaded: usize,
    /// Failed to load
    pub failed: usize,
    /// Plugin-specific errors
    pub errors: Vec<String>,
}

/// Plugin manager that coordinates loading and lifecycle
pub struct PluginManager {
    /// Plugin loader
    loader: PluginLoader,
    /// Plugin registry
    registry: PluginRegistry,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new(
        plugins_dir: impl AsRef<Path>,
        event_bus: Arc<EventBus>,
        data_dir: impl AsRef<Path>,
    ) -> Self {
        log::debug!("Creating PluginManager");
        Self {
            loader: PluginLoader::new(plugins_dir, event_bus, data_dir),
            registry: PluginRegistry::new(),
        }
    }

    /// Load all plugins
    ///
    /// Returns loading status with counts and errors.
    pub async fn load_all(&mut self) -> PluginLoadStatus {
        log::info!("PluginManager: Loading all plugins");

        let discovered = match self.loader.discover() {
            Ok(d) => d,
            Err(e) => {
                log::error!("Failed to discover plugins: {}", e);
                return PluginLoadStatus {
                    discovered: 0,
                    loaded: 0,
                    failed: 0,
                    errors: vec![e.to_string()],
                };
            }
        };

        let total = discovered.len();
        let mut loaded = 0;
        let mut errors = Vec::new();

        log::debug!("Discovered {} plugins to load", total);

        for discovery in discovered {
            let plugin_id = discovery.manifest.plugin.id.clone();

            match self.loader.load_plugin(discovery, &mut self.registry).await {
                Ok(()) => loaded += 1,
                Err(e) => {
                    let error_msg = format!("{}: {}", plugin_id, e);
                    log::error!("Plugin loading error: {}", error_msg);
                    errors.push(error_msg);
                }
            }
        }

        let status = PluginLoadStatus {
            discovered: total,
            loaded,
            failed: errors.len(),
            errors,
        };

        log::info!(
            "Plugin loading complete: {} discovered, {} loaded, {} failed",
            status.discovered,
            status.loaded,
            status.failed
        );

        status
    }

    /// Get the plugin registry
    pub fn registry(&self) -> &PluginRegistry {
        &self.registry
    }

    /// Get mutable plugin registry
    pub fn registry_mut(&mut self) -> &mut PluginRegistry {
        &mut self.registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::manifest::PluginKind;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    /// Create a test plugin manifest
    fn create_test_plugin(dir: &Path, id: &str, kind: PluginKind) -> PathBuf {
        let plugin_dir = dir.join(id);
        fs::create_dir_all(&plugin_dir).unwrap();

        // Convert PluginKind to kebab-case string for TOML
        let kind_str = match kind {
            PluginKind::OrbStyle => "orb-style",
            PluginKind::AudioProcessor => "audio-processor",
            PluginKind::PostProcessor => "post-processor",
            PluginKind::Integration => "integration",
        };

        let manifest_content = format!(
            r#"
[plugin]
id = "{}"
name = "Test Plugin {}"
version = "1.0.0"
description = "A test plugin"
author = "Test Author"

[plugin.type]
kind = "{}"

[plugin.capabilities]
audio_level = true
audio_fft = true
"#,
            id, id, kind_str
        );

        let manifest_path = plugin_dir.join("plugin.toml");
        let mut file = File::create(&manifest_path).unwrap();
        file.write_all(manifest_content.as_bytes()).unwrap();

        manifest_path
    }

    /// Create a test plugin with invalid manifest
    fn create_invalid_plugin(dir: &Path, id: &str) -> PathBuf {
        let plugin_dir = dir.join(id);
        fs::create_dir_all(&plugin_dir).unwrap();

        let manifest_path = plugin_dir.join("plugin.toml");
        let mut file = File::create(&manifest_path).unwrap();
        file.write_all(b"invalid toml content [[[[").unwrap();

        manifest_path
    }

    /// Create a test plugin with empty ID (validation failure)
    fn create_invalid_validation_plugin(dir: &Path, id: &str) -> PathBuf {
        let plugin_dir = dir.join(id);
        fs::create_dir_all(&plugin_dir).unwrap();

        let manifest_content = r#"
[plugin]
id = ""
name = "Invalid Plugin"
version = "1.0.0"
description = "A plugin with empty ID"
author = "Test Author"

[plugin.type]
kind = "orb-style"
"#;

        let manifest_path = plugin_dir.join("plugin.toml");
        let mut file = File::create(&manifest_path).unwrap();
        file.write_all(manifest_content.as_bytes()).unwrap();

        manifest_path
    }

    #[test]
    fn test_discover_plugins() {
        let temp_dir = TempDir::new().unwrap();
        let plugins_dir = temp_dir.path();

        // Create test plugins
        create_test_plugin(plugins_dir, "test-plugin-1", PluginKind::OrbStyle);
        create_test_plugin(plugins_dir, "test-plugin-2", PluginKind::AudioProcessor);

        // Create a directory without plugin.toml (should be skipped)
        let empty_dir = plugins_dir.join("empty-plugin");
        fs::create_dir_all(&empty_dir).unwrap();

        // Create a file (should be skipped)
        let file_path = plugins_dir.join("not-a-directory.txt");
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"test").unwrap();

        // Create loader
        let event_bus = Arc::new(EventBus::new());
        let loader = PluginLoader::new(plugins_dir, event_bus, temp_dir.path());

        // Discover plugins
        let discovered = loader.discover().unwrap();

        // Should find 2 valid plugins
        assert_eq!(discovered.len(), 2);
        assert_eq!(discovered[0].manifest.plugin.id, "test-plugin-1");
        assert_eq!(discovered[1].manifest.plugin.id, "test-plugin-2");
    }

    #[test]
    fn test_discover_nonexistent_directory() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent_dir = temp_dir.path().join("nonexistent");

        let event_bus = Arc::new(EventBus::new());
        let loader = PluginLoader::new(&nonexistent_dir, event_bus, temp_dir.path());

        // Should return empty list, not error
        let discovered = loader.discover().unwrap();
        assert_eq!(discovered.len(), 0);
    }

    #[test]
    fn test_discover_invalid_manifest() {
        let temp_dir = TempDir::new().unwrap();
        let plugins_dir = temp_dir.path();

        // Create valid plugin
        create_test_plugin(plugins_dir, "valid-plugin", PluginKind::OrbStyle);

        // Create plugin with invalid TOML
        create_invalid_plugin(plugins_dir, "invalid-toml");

        // Create plugin with validation error
        create_invalid_validation_plugin(plugins_dir, "invalid-validation");

        let event_bus = Arc::new(EventBus::new());
        let loader = PluginLoader::new(plugins_dir, event_bus, temp_dir.path());

        // Should return error for invalid manifests
        let result = loader.discover();
        assert!(result.is_err());

        // Error should mention parse or validation failure
        let err_str = result.unwrap_err().to_string();
        assert!(
            err_str.contains("parse") || err_str.contains("validation"),
            "Expected parse or validation error, got: {}",
            err_str
        );
    }

    #[test]
    fn test_plugin_data_dir() {
        let temp_dir = TempDir::new().unwrap();
        let data_base = temp_dir.path().join("data");

        let event_bus = Arc::new(EventBus::new());
        let loader = PluginLoader::new(temp_dir.path(), event_bus, &data_base);

        let plugin_dir = loader.plugin_data_dir("test-plugin");
        assert_eq!(plugin_dir, data_base.join("test-plugin"));
    }

    #[tokio::test]
    async fn test_load_all_plugins() {
        let temp_dir = TempDir::new().unwrap();
        let plugins_dir = temp_dir.path();
        let data_dir = temp_dir.path().join("data");

        // Create test plugins
        create_test_plugin(plugins_dir, "plugin-1", PluginKind::OrbStyle);
        create_test_plugin(plugins_dir, "plugin-2", PluginKind::AudioProcessor);
        create_test_plugin(plugins_dir, "plugin-3", PluginKind::PostProcessor);

        let event_bus = Arc::new(EventBus::new());
        let loader = PluginLoader::new(plugins_dir, event_bus, &data_dir);

        let mut registry = PluginRegistry::new();
        let loaded = loader.load_all(&mut registry).await.unwrap();

        // All plugins should be loaded
        assert_eq!(loaded, 3);
        assert_eq!(registry.count(), 3);

        // Check plugins are registered
        assert!(registry.has("plugin-1"));
        assert!(registry.has("plugin-2"));
        assert!(registry.has("plugin-3"));

        // Check type counts
        assert_eq!(registry.count_by_type(PluginKind::OrbStyle), 1);
        assert_eq!(registry.count_by_type(PluginKind::AudioProcessor), 1);
        assert_eq!(registry.count_by_type(PluginKind::PostProcessor), 1);
    }

    #[tokio::test]
    async fn test_plugin_manager_load_all() {
        let temp_dir = TempDir::new().unwrap();
        let plugins_dir = temp_dir.path();
        let data_dir = temp_dir.path().join("data");

        // Create test plugins
        create_test_plugin(plugins_dir, "manager-test-1", PluginKind::OrbStyle);
        create_test_plugin(plugins_dir, "manager-test-2", PluginKind::Integration);

        let event_bus = Arc::new(EventBus::new());
        let mut manager = PluginManager::new(plugins_dir, event_bus, data_dir);

        let status = manager.load_all().await;

        assert_eq!(status.discovered, 2);
        assert_eq!(status.loaded, 2);
        assert_eq!(status.failed, 0);
        assert!(status.errors.is_empty());

        // Check registry
        assert_eq!(manager.registry().count(), 2);
        assert!(manager.registry().has("manager-test-1"));
        assert!(manager.registry().has("manager-test-2"));
    }

    #[test]
    fn test_mock_plugin() {
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = create_test_plugin(temp_dir.path(), "mock-test", PluginKind::OrbStyle);
        let manifest = PluginManifest::from_file(&manifest_path).unwrap();

        let plugin = MockPlugin::new(manifest.clone());

        assert_eq!(plugin.manifest().plugin.id, "mock-test");
        // Config will be empty since test manifest has no config section
        assert!(plugin.config.is_empty());
    }

    #[tokio::test]
    async fn test_mock_plugin_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = create_test_plugin(temp_dir.path(), "lifecycle-test", PluginKind::OrbStyle);
        let manifest = PluginManifest::from_file(&manifest_path).unwrap();

        let mut plugin = MockPlugin::new(manifest);

        // Test init
        let context = crate::plugins::traits::PluginContext {
            event_bus: Arc::new(EventBus::new()),
            data_dir: temp_dir.path().to_path_buf(),
            config: HashMap::new(),
            width: 500,
            height: 500,
        };

        assert!(plugin.init(context.clone()).await.is_ok());

        // Test config change
        let mut new_config = HashMap::new();
        new_config.insert("test".to_string(), serde_json::json!(42));
        plugin.on_config_change(&new_config);
        assert_eq!(plugin.config.get("test"), Some(&serde_json::json!(42)));

        // Test shutdown
        assert!(plugin.shutdown().await.is_ok());
    }
}
