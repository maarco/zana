//! Plugin Manifest
//!
//! Defines the plugin.toml manifest format for kVoice plugins.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Plugin manifest (plugin.toml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Core plugin metadata
    pub plugin: PluginMeta,
}

/// Core plugin metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMeta {
    /// Unique plugin identifier (e.g., "nebula-aura")
    pub id: String,

    /// Display name
    pub name: String,

    /// Semantic version (e.g., "1.0.0")
    pub version: String,

    /// Short description
    pub description: String,

    /// Author name
    pub author: String,

    /// License (e.g., "MIT")
    #[serde(default)]
    pub license: Option<String>,

    /// Homepage URL
    #[serde(default)]
    pub homepage: Option<String>,

    /// Plugin type information
    #[serde(rename = "type")]
    pub plugin_type: PluginTypeMeta,

    /// Required capabilities
    #[serde(default)]
    pub capabilities: PluginCapabilities,

    /// UI configuration (for orb-style plugins)
    #[serde(default)]
    pub ui: Option<PluginUiConfig>,

    /// User-configurable options
    #[serde(default)]
    pub config: Option<PluginConfigSchema>,

    /// Development settings
    #[serde(default)]
    pub dev: Option<PluginDevConfig>,
}

/// Plugin type metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginTypeMeta {
    /// Plugin kind
    pub kind: PluginKind,
}

/// Plugin kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginKind {
    /// Audio visualization style
    OrbStyle,

    /// Audio processor (modifies audio before STT)
    AudioProcessor,

    /// Post-processor (modifies transcription output)
    PostProcessor,

    /// External service integration
    Integration,
}

/// Plugin capabilities (permissions)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginCapabilities {
    /// Access to audio level data
    #[serde(default)]
    pub audio_level: bool,

    /// Access to FFT data
    #[serde(default)]
    pub audio_fft: bool,

    /// Access to raw audio buffer
    #[serde(default)]
    pub audio_buffer: bool,

    /// Access to transcription events
    #[serde(default)]
    pub transcription_events: bool,

    /// Access to transcription text (for modification)
    #[serde(default)]
    pub transcription_modify: bool,

    /// Read settings
    #[serde(default)]
    pub settings_read: bool,

    /// Write settings
    #[serde(default)]
    pub settings_write: bool,

    /// Network access
    #[serde(default)]
    pub network: bool,

    /// File system access
    #[serde(default)]
    pub filesystem: bool,
}

/// UI configuration for orb-style plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginUiConfig {
    /// Default window width in pixels
    #[serde(default = "default_width")]
    pub default_width: u32,

    /// Default window height in pixels
    #[serde(default = "default_height")]
    pub default_height: u32,

    /// Whether window should be transparent
    #[serde(default = "default_transparent")]
    pub transparent: bool,

    /// Whether window is resizable
    #[serde(default = "default_resizable")]
    pub resizable: bool,

    /// Minimum width
    #[serde(default)]
    pub min_width: Option<u32>,

    /// Minimum height
    #[serde(default)]
    pub min_height: Option<u32>,

    /// Maximum width
    #[serde(default)]
    pub max_width: Option<u32>,

    /// Maximum height
    #[serde(default)]
    pub max_height: Option<u32>,
}

fn default_width() -> u32 {
    400
}

fn default_height() -> u32 {
    400
}

fn default_transparent() -> bool {
    true
}

fn default_resizable() -> bool {
    true
}

impl Default for PluginUiConfig {
    fn default() -> Self {
        Self {
            default_width: default_width(),
            default_height: default_height(),
            transparent: default_transparent(),
            resizable: default_resizable(),
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
        }
    }
}

/// Plugin configuration schema
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginConfigSchema {
    /// Configuration options
    #[serde(default)]
    pub options: Vec<ConfigOption>,
}

/// A single configuration option
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigOption {
    /// Option key (used in config object)
    pub key: String,

    /// Display label
    pub label: String,

    /// Description
    #[serde(default)]
    pub description: Option<String>,

    /// Option type
    #[serde(rename = "type")]
    pub option_type: ConfigOptionType,

    /// Default value
    pub default: serde_json::Value,

    /// Minimum value (for numbers)
    #[serde(default)]
    pub min: Option<f64>,

    /// Maximum value (for numbers)
    #[serde(default)]
    pub max: Option<f64>,

    /// Available options (for select type)
    #[serde(default)]
    pub options: Option<Vec<String>>,
}

/// Configuration option types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigOptionType {
    Number,
    String,
    Boolean,
    Color,
    Select,
}

/// Development configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginDevConfig {
    /// Enable debug mode
    #[serde(default)]
    pub debug: bool,

    /// Log level
    #[serde(default)]
    pub log_level: Option<String>,
}

impl PluginManifest {
    /// Load manifest from a TOML file
    pub fn from_file(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let manifest: PluginManifest = toml::from_str(&content)?;
        Ok(manifest)
    }

    /// Load manifest from a TOML string
    pub fn from_str(content: &str) -> anyhow::Result<Self> {
        let manifest: PluginManifest = toml::from_str(content)?;
        Ok(manifest)
    }

    /// Validate the manifest
    pub fn validate(&self) -> anyhow::Result<()> {
        // ID must be valid
        if self.plugin.id.is_empty() {
            anyhow::bail!("Plugin ID cannot be empty");
        }
        if !self.plugin.id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            anyhow::bail!("Plugin ID must only contain alphanumeric characters, hyphens, and underscores");
        }

        // Version must be semver-ish
        if self.plugin.version.is_empty() {
            anyhow::bail!("Plugin version cannot be empty");
        }

        // Name must not be empty
        if self.plugin.name.is_empty() {
            anyhow::bail!("Plugin name cannot be empty");
        }

        // Validate config options
        if let Some(ref config) = self.plugin.config {
            for option in &config.options {
                self.validate_config_option(option)?;
            }
        }

        Ok(())
    }

    fn validate_config_option(&self, option: &ConfigOption) -> anyhow::Result<()> {
        // Key must be valid
        if option.key.is_empty() {
            anyhow::bail!("Config option key cannot be empty");
        }

        // Type-specific validation
        match option.option_type {
            ConfigOptionType::Number => {
                if let Some(min) = option.min {
                    if let Some(max) = option.max {
                        if min > max {
                            anyhow::bail!(
                                "Config option '{}': min ({}) > max ({})",
                                option.key,
                                min,
                                max
                            );
                        }
                    }
                }
            }
            ConfigOptionType::Select => {
                if option.options.as_ref().map(|o| o.is_empty()).unwrap_or(true) {
                    anyhow::bail!(
                        "Config option '{}': select type requires non-empty options array",
                        option.key
                    );
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Get default configuration values
    pub fn default_config(&self) -> HashMap<String, serde_json::Value> {
        let mut config = HashMap::new();

        if let Some(ref schema) = self.plugin.config {
            for option in &schema.options {
                config.insert(option.key.clone(), option.default.clone());
            }
        }

        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_MANIFEST: &str = r#"
[plugin]
id = "nebula-aura"
name = "Nebula Aura"
version = "1.0.0"
description = "Cosmic nebula visualization"
author = "kVoice Team"
license = "MIT"

[plugin.type]
kind = "orb-style"

[plugin.capabilities]
audio_level = true
audio_fft = true

[plugin.ui]
default_width = 500
default_height = 500
transparent = true
resizable = true

[[plugin.config.options]]
key = "particle_density"
label = "Particle Density"
type = "number"
default = 1.0
min = 0.5
max = 2.0

"#;

    #[test]
    fn test_parse_manifest() {
        let manifest = PluginManifest::from_str(VALID_MANIFEST).unwrap();

        assert_eq!(manifest.plugin.id, "nebula-aura");
        assert_eq!(manifest.plugin.name, "Nebula Aura");
        assert_eq!(manifest.plugin.version, "1.0.0");
        assert_eq!(manifest.plugin.plugin_type.kind, PluginKind::OrbStyle);
        assert!(manifest.plugin.capabilities.audio_level);
        assert!(manifest.plugin.capabilities.audio_fft);
        assert!(!manifest.plugin.capabilities.network);
    }

    #[test]
    fn test_validate_manifest() {
        let manifest = PluginManifest::from_str(VALID_MANIFEST).unwrap();
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn test_default_config() {
        let manifest = PluginManifest::from_str(VALID_MANIFEST).unwrap();
        let config = manifest.default_config();

        assert_eq!(config.get("particle_density"), Some(&serde_json::json!(1.0)));
    }
}
