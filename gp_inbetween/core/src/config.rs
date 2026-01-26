use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Failed to parse config: {0}")]
    ParseError(#[from] toml::de::Error),

    #[error("Failed to serialize config: {0}")]
    SerializeError(#[from] toml::ser::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Confidence threshold for auto-accepting frames (0.0 - 1.0)
    pub auto_accept_threshold: f32,

    /// Path to feedback log file (optional, uses default if None)
    pub feedback_log_path: Option<String>,

    /// API configuration
    pub api: ApiConfig,

    /// Preprocessing options
    pub preprocessing: PreprocessingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Backend type: "replicate", "local", "serverless"
    pub backend: String,

    /// API endpoint URL (for local/serverless backends)
    pub endpoint: String,

    /// API key (required for Replicate)
    pub api_key: Option<String>,

    /// Replicate model version (for Replicate backend)
    pub replicate_model: Option<String>,

    /// Style strength (0.0 - 1.0)
    pub style_strength: f32,

    /// Request timeout in seconds
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreprocessingConfig {
    /// Enable stroke cleanup (merge duplicates, remove small strokes)
    pub cleanup_enabled: bool,

    /// Target resolution for processing (frames will be scaled to this)
    pub target_resolution: u32,

    /// Normalize frames to square aspect ratio with padding
    pub normalize_resolution: bool,

    /// Minimum stroke length in pixels (strokes shorter than this are removed)
    pub min_stroke_length: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auto_accept_threshold: 0.85,
            feedback_log_path: None,
            api: ApiConfig {
                backend: "replicate".to_string(),
                endpoint: "http://localhost:8000/generate".to_string(),
                api_key: None,
                replicate_model: Some(
                    "fofr/tooncrafter:0d5c6b3a4e0d6b8a9b8e7d6c5b4a3f2e1d0c9b8a".to_string(),
                ),
                style_strength: 0.8,
                timeout_secs: 180,
            },
            preprocessing: PreprocessingConfig {
                cleanup_enabled: true,
                target_resolution: 1024,
                normalize_resolution: true,
                min_stroke_length: 5.0,
            },
        }
    }
}

impl Config {
    /// Load configuration from a TOML file
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Save configuration to a TOML file
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let toml = toml::to_string_pretty(self)?;
        std::fs::write(path, toml)?;
        Ok(())
    }

    /// Get the default config path (~/.config/gp_ai_inbetween/config.toml)
    pub fn default_path() -> Option<std::path::PathBuf> {
        dirs::config_dir().map(|p| p.join("gp_ai_inbetween").join("config.toml"))
    }

    /// Load from default path, or return default config if not found
    pub fn load_or_default() -> Self {
        Self::default_path()
            .and_then(|p| Self::load(&p).ok())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.api.backend, "replicate");
        assert!(config.auto_accept_threshold > 0.0);
        assert!(config.auto_accept_threshold <= 1.0);
    }

    #[test]
    fn test_config_roundtrip() {
        let config = Config::default();
        let toml = toml::to_string(&config).unwrap();
        let parsed: Config = toml::from_str(&toml).unwrap();
        assert_eq!(parsed.api.backend, config.api.backend);
    }
}
