use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Application configuration, loadable from TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub defaults: DefaultsConfig,
    pub display: DisplayConfig,
    pub behavior: BehaviorConfig,
    pub logging: LoggingConfig,
    pub history: HistoryConfig,
    pub connection: ConnectionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DefaultsConfig {
    pub baud_rate: u32,
    pub data_bits: u8,
    pub parity: String,
    pub stop_bits: u8,
    pub flow_control: String,
    pub line_ending: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayConfig {
    pub timestamps: bool,
    pub timestamp_format: String,
    pub color_log_levels: bool,
    pub show_line_endings: bool,
    pub hex_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BehaviorConfig {
    pub auto_reconnect: bool,
    pub reconnect_delay_ms: u64,
    pub scrollback_lines: usize,
    pub follow_output: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    pub auto_log: bool,
    pub log_directory: String,
    pub log_format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HistoryConfig {
    pub max_entries: usize,
    pub file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnectionConfig {
    /// Last connected port name (e.g. "COM3" or "/dev/ttyUSB0").
    pub last_port: Option<String>,
    /// Whether to auto-connect to last_port on startup.
    pub auto_connect: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            defaults: DefaultsConfig::default(),
            display: DisplayConfig::default(),
            behavior: BehaviorConfig::default(),
            logging: LoggingConfig::default(),
            history: HistoryConfig::default(),
            connection: ConnectionConfig::default(),
        }
    }
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            baud_rate: 115200,
            data_bits: 8,
            parity: "none".to_string(),
            stop_bits: 1,
            flow_control: "none".to_string(),
            line_ending: "crlf".to_string(),
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            timestamps: true,
            timestamp_format: "%H:%M:%S%.3f".to_string(),
            color_log_levels: true,
            show_line_endings: false,
            hex_mode: false,
        }
    }
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            auto_reconnect: true,
            reconnect_delay_ms: 1000,
            scrollback_lines: 10000,
            follow_output: true,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            auto_log: false,
            log_directory: "~/.local/share/yapper/logs".to_string(),
            log_format: "timestamped".to_string(),
        }
    }
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            max_entries: 500,
            file: "~/.local/share/yapper/history".to_string(),
        }
    }
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            last_port: None,
            auto_connect: true,
        }
    }
}

impl AppConfig {
    /// Load config from the default XDG path, falling back to defaults.
    pub fn load() -> Self {
        if let Some(config_dir) = dirs::config_dir() {
            let config_path = config_dir.join("yapper").join("config.toml");
            if config_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&config_path) {
                    if let Ok(config) = toml::from_str(&content) {
                        return config;
                    }
                }
            }
        }
        Self::default()
    }

    /// Save config to the default XDG path.
    pub fn save(&self) {
        if let Some(config_dir) = dirs::config_dir() {
            let config_path = config_dir.join("yapper").join("config.toml");
            if let Some(parent) = config_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(content) = toml::to_string_pretty(self) {
                let _ = std::fs::write(config_path, content);
            }
        }
    }
}

/// Expand a config path, resolving a leading `~/` against the current home
/// directory. Empty paths are treated as unset.
pub fn expand_path(path: &str) -> Option<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed == "~" {
        return dirs::home_dir();
    }

    if let Some(rest) = trimmed.strip_prefix("~/") {
        return dirs::home_dir().map(|home| home.join(rest));
    }

    Some(PathBuf::from(trimmed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.defaults.baud_rate, 115200);
        assert_eq!(config.defaults.data_bits, 8);
        assert_eq!(config.behavior.scrollback_lines, 10000);
        assert!(config.display.timestamps);
        assert!(!config.display.hex_mode);
    }

    #[test]
    fn test_deserialize_partial_config() {
        let toml_str = r#"
            [defaults]
            baud_rate = 9600

            [display]
            timestamps = false
        "#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.defaults.baud_rate, 9600);
        assert!(!config.display.timestamps);
        // Defaults should be preserved for unset fields
        assert_eq!(config.behavior.scrollback_lines, 10000);
    }

    #[test]
    fn test_expand_path_with_tilde() {
        let expanded = expand_path("~/tmp").unwrap();
        assert!(expanded.ends_with("tmp"));
        assert!(expanded.is_absolute());
    }

    #[test]
    fn test_expand_path_empty_is_none() {
        assert!(expand_path("   ").is_none());
    }
}
