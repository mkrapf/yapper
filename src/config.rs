use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Application configuration, loadable from TOML.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub defaults: DefaultsConfig,
    pub display: DisplayConfig,
    pub behavior: BehaviorConfig,
    pub logging: LoggingConfig,
    pub history: HistoryConfig,
    pub quicksend: QuicksendConfig,
    pub connection: ConnectionConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct QuicksendConfig {
    pub recent: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnectionConfig {
    /// Last connected port name (e.g. "COM3" or "/dev/ttyUSB0").
    pub last_port: Option<String>,
    /// Whether to auto-connect to last_port on startup.
    pub auto_connect: bool,
    /// Per-port remembered serial settings keyed by exact port identifier.
    pub port_profiles: BTreeMap<String, DefaultsConfig>,
}

pub struct ConfigLoad {
    pub config: AppConfig,
    pub warning: Option<String>,
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
            port_profiles: BTreeMap::new(),
        }
    }
}

impl AppConfig {
    /// Load config from the default XDG path, falling back to defaults.
    pub fn load() -> Self {
        Self::load_with_diagnostics().config
    }

    pub fn load_with_diagnostics() -> ConfigLoad {
        let Some(config_dir) = dirs::config_dir() else {
            return ConfigLoad {
                config: Self::default(),
                warning: Some("Config warning: could not determine config directory".to_string()),
            };
        };

        let config_path = config_dir.join("yapper").join("config.toml");
        if !config_path.exists() {
            return ConfigLoad {
                config: Self::default(),
                warning: None,
            };
        }

        match Self::load_from_path(&config_path) {
            Ok(config) => ConfigLoad {
                config,
                warning: None,
            },
            Err(err) => ConfigLoad {
                config: Self::default(),
                warning: Some(format!("Config warning: {}; using defaults", err)),
            },
        }
    }

    pub fn load_from_path(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|err| format!("failed to read {}: {}", path.display(), err))?;
        toml::from_str(&content)
            .map_err(|err| format!("failed to parse {}: {}", path.display(), err))
    }

    /// Save config to the default XDG path.
    pub fn save(&self) -> Result<(), String> {
        let config_dir =
            dirs::config_dir().ok_or_else(|| "could not determine config directory".to_string())?;
        self.save_to_path(&config_dir.join("yapper").join("config.toml"))
    }

    pub fn save_to_path(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create {}: {}", parent.display(), err))?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|err| format!("failed to encode config: {}", err))?;
        std::fs::write(path, content)
            .map_err(|err| format!("failed to write {}: {}", path.display(), err))
    }
}

impl DefaultsConfig {
    pub fn to_serial_config(&self) -> crate::serial::config::SerialConfig {
        crate::serial::config::SerialConfig {
            baud_rate: self.baud_rate,
            data_bits: parse_data_bits(self.data_bits),
            parity: parse_parity(&self.parity),
            stop_bits: parse_stop_bits(self.stop_bits),
            flow_control: parse_flow_control(&self.flow_control),
        }
    }

    pub fn to_line_ending(&self) -> String {
        line_ending_from_config(&self.line_ending)
    }

    pub fn from_runtime(
        serial_config: &crate::serial::config::SerialConfig,
        line_ending: &str,
    ) -> Self {
        Self {
            baud_rate: serial_config.baud_rate,
            data_bits: match serial_config.data_bits {
                serialport::DataBits::Five => 5,
                serialport::DataBits::Six => 6,
                serialport::DataBits::Seven => 7,
                serialport::DataBits::Eight => 8,
            },
            parity: match serial_config.parity {
                serialport::Parity::None => "none".to_string(),
                serialport::Parity::Odd => "odd".to_string(),
                serialport::Parity::Even => "even".to_string(),
            },
            stop_bits: match serial_config.stop_bits {
                serialport::StopBits::One => 1,
                serialport::StopBits::Two => 2,
            },
            flow_control: match serial_config.flow_control {
                serialport::FlowControl::None => "none".to_string(),
                serialport::FlowControl::Software => "software".to_string(),
                serialport::FlowControl::Hardware => "hardware".to_string(),
            },
            line_ending: line_ending_to_config(line_ending),
        }
    }
}

pub fn line_ending_from_config(value: &str) -> String {
    match value {
        "lf" => "\n".to_string(),
        "cr" => "\r".to_string(),
        _ => "\r\n".to_string(),
    }
}

pub fn line_ending_to_config(value: &str) -> String {
    match value {
        "\n" => "lf".to_string(),
        "\r" => "cr".to_string(),
        _ => "crlf".to_string(),
    }
}

fn parse_data_bits(bits: u8) -> serialport::DataBits {
    match bits {
        5 => serialport::DataBits::Five,
        6 => serialport::DataBits::Six,
        7 => serialport::DataBits::Seven,
        _ => serialport::DataBits::Eight,
    }
}

fn parse_parity(parity: &str) -> serialport::Parity {
    match parity.to_lowercase().as_str() {
        "odd" => serialport::Parity::Odd,
        "even" => serialport::Parity::Even,
        _ => serialport::Parity::None,
    }
}

fn parse_stop_bits(bits: u8) -> serialport::StopBits {
    match bits {
        2 => serialport::StopBits::Two,
        _ => serialport::StopBits::One,
    }
}

fn parse_flow_control(fc: &str) -> serialport::FlowControl {
    match fc.to_lowercase().as_str() {
        "software" | "sw" | "xon" => serialport::FlowControl::Software,
        "hardware" | "hw" | "rts" => serialport::FlowControl::Hardware,
        _ => serialport::FlowControl::None,
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
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_path(prefix: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("yapper-config-test-{}-{}", prefix, suffix))
    }

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

    #[test]
    fn test_quicksend_and_port_profiles_round_trip() {
        let mut config = AppConfig::default();
        config.quicksend.recent = vec!["AT".to_string(), "RST".to_string()];
        config.connection.port_profiles.insert(
            "/dev/ttyUSB0".to_string(),
            DefaultsConfig {
                baud_rate: 9600,
                data_bits: 7,
                parity: "even".to_string(),
                stop_bits: 2,
                flow_control: "hardware".to_string(),
                line_ending: "lf".to_string(),
            },
        );

        let encoded = toml::to_string(&config).unwrap();
        let decoded: AppConfig = toml::from_str(&encoded).unwrap();

        assert_eq!(decoded.quicksend.recent, vec!["AT", "RST"]);
        assert_eq!(
            decoded
                .connection
                .port_profiles
                .get("/dev/ttyUSB0")
                .unwrap()
                .baud_rate,
            9600
        );
        assert_eq!(
            decoded
                .connection
                .port_profiles
                .get("/dev/ttyUSB0")
                .unwrap()
                .line_ending,
            "lf"
        );
    }

    #[test]
    fn test_load_from_path_reports_malformed_config() {
        let path = unique_temp_path("bad.toml");
        std::fs::write(&path, "[defaults\nbaud_rate = 9600").unwrap();

        let error = AppConfig::load_from_path(&path).unwrap_err();

        assert!(error.contains("failed to parse"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_save_to_path_reports_failed_write() {
        let path = unique_temp_path("dir");
        std::fs::create_dir_all(&path).unwrap();

        let error = AppConfig::default().save_to_path(&path).unwrap_err();

        assert!(error.contains("failed to write"));
        let _ = std::fs::remove_dir_all(path);
    }
}
