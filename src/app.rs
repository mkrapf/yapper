use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::buffer::ScrollbackBuffer;
use crate::config::{AppConfig, DefaultsConfig};
use crate::filter::LineFilter;
use crate::history::CommandHistory;
use crate::logging::SessionLogger;
use crate::macros::MacroManager;
use crate::mouse::{LayoutRegions, TextSelection};
use crate::search::Search;
use crate::serial::config::SerialConfig;
use crate::serial::connection::SerialEvent;
use crate::serial::detector::PortInfo;
use crate::transport::{RealTransport, Transport, TransportConnection};

mod connection_flow;
mod filters;
mod input_editing;
mod macro_runner;
mod scroll_search;
mod settings_flow;

/// The application mode determines how keyboard input is handled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    /// Normal mode: scroll, search, toggle settings.
    Normal,
    /// Input mode: typing commands to send.
    Input,
    /// Search mode: typing search query.
    Search,
    /// Port selector popup is open.
    PortSelect,
    /// UART settings popup is open.
    Settings,
    /// Help overlay is shown.
    Help,
    /// Macro selector popup is open.
    MacroSelect,
    /// Filter manager popup is open.
    Filter,
}

impl Mode {
    fn is_overlay(self) -> bool {
        matches!(
            self,
            Self::Search
                | Self::PortSelect
                | Self::Settings
                | Self::Help
                | Self::MacroSelect
                | Self::Filter
        )
    }
}

/// Connection state for display purposes.
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connected(String),
    Reconnecting(String),
    Error(String),
}

#[derive(Debug, Clone)]
struct PendingMacroCommand {
    text: String,
    ready_at: Instant,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SendSource {
    Manual,
    Quicksend,
    Macro,
}

const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(10);

/// Central application state.
pub struct App {
    /// Current input mode.
    pub mode: Mode,
    /// Mode to restore when the current overlay closes.
    return_mode: Mode,
    /// Whether the app should quit.
    pub should_quit: bool,
    /// Scrollback buffer containing all received lines.
    pub buffer: ScrollbackBuffer,
    /// The text currently being typed in the input bar.
    pub input_text: String,
    /// Cursor position within input_text.
    pub input_cursor: usize,
    /// Line ending to append when sending commands.
    pub line_ending: String,
    /// Serial port configuration.
    pub serial_config: SerialConfig,
    /// Current connection state.
    pub connection_state: ConnectionState,
    /// Active serial connection (if connected).
    connection: Option<Box<dyn TransportConnection>>,
    /// Runtime used to list ports and open serial-like connections.
    transport: Arc<dyn Transport>,
    /// Receiver for serial events from the reader thread.
    serial_rx: Option<Receiver<SerialEvent>>,
    /// Sender end — kept to pass to new connections.
    serial_tx: Option<Sender<SerialEvent>>,
    /// Scroll offset (0 = bottom/latest, higher = scrolled up).
    pub scroll_offset: usize,
    /// Whether to auto-follow new output.
    pub follow_output: bool,
    /// Total RX bytes (persisted across reconnects).
    pub rx_bytes: u64,
    /// Total TX bytes (persisted across reconnects).
    pub tx_bytes: u64,
    /// Available ports for the port selector.
    pub available_ports: Vec<PortInfo>,
    /// Selected index in the port selector.
    pub port_select_index: usize,
    /// Whether timestamps are enabled.
    pub show_timestamps: bool,
    /// strftime format string for timestamp rendering.
    pub timestamp_format: String,
    /// Whether hex view mode is enabled.
    pub hex_mode: bool,
    /// Whether to show line ending indicators.
    pub show_line_endings: bool,
    /// Whether severity/log-level coloring is enabled.
    pub color_log_levels: bool,
    /// Command history.
    pub history: CommandHistory,
    /// Search state.
    pub search: Search,
    /// Session logger.
    pub logger: SessionLogger,
    /// Auto-reconnect enabled.
    pub auto_reconnect: bool,
    /// Port name for auto-reconnect.
    reconnect_port: Option<String>,
    /// Base reconnect delay.
    reconnect_delay: Duration,
    /// Delay currently scheduled for the next reconnect attempt.
    reconnect_current_delay: Duration,
    /// When the next reconnect attempt should happen.
    reconnect_next_attempt: Option<Instant>,
    /// Number of failed reconnect attempts so far.
    reconnect_attempts: usize,
    /// Status message (shown temporarily in status bar).
    pub status_message: Option<(String, Instant)>,
    /// Line filter (regex-based include/exclude).
    pub filter: LineFilter,
    /// Macro manager.
    pub macros: MacroManager,
    /// Selected macro index (for macro selector popup).
    pub macro_select_index: usize,
    /// Currently selected field in settings popup (0-4).
    pub settings_field: usize,
    /// Serial config snapshot from when settings were opened.
    settings_original_config: Option<SerialConfig>,
    /// Line ending snapshot from when settings were opened.
    settings_original_line_ending: Option<String>,
    /// Layout regions for mouse click detection.
    pub layout: LayoutRegions,
    /// Text selection state for click-drag-copy.
    pub selection: TextSelection,
    /// Ghost auto-complete suggestion from history.
    pub ghost_suggestion: Option<String>,
    /// Application config (for persistence).
    pub app_config: AppConfig,
    /// Timestamp of the last sent command (for response timing).
    pub last_command_sent: Option<Instant>,
    /// Duration of the last command round-trip.
    pub last_response_time: Option<Duration>,
    /// Quick-send commands in persisted MRU order.
    pub quicksend: Vec<String>,
    /// Whether to display sent messages in the terminal view.
    pub show_sent: bool,
    /// Input text for the filter popup.
    pub filter_input: String,
    /// Whether filter input mode is exclude (true) vs include (false).
    pub filter_mode_is_exclude: bool,
    /// Selected filter index for deletion.
    pub filter_select_index: usize,
    /// Whether hex input mode is active.
    pub hex_input_mode: bool,
    /// Pending macro commands waiting to be sent on future ticks.
    pending_macro_commands: VecDeque<PendingMacroCommand>,
    /// Name of the macro currently being executed.
    active_macro_name: Option<String>,
    /// Name of the last successfully started macro.
    last_executed_macro: Option<String>,
    /// Scroll offset within the help overlay.
    pub help_scroll: u16,
    /// Maximum scroll offset available within the help overlay.
    pub help_scroll_max: u16,
}

impl App {
    pub fn new(serial_config: SerialConfig, line_ending: String, app_config: AppConfig) -> Self {
        Self::new_with_transport(
            serial_config,
            line_ending,
            app_config,
            Arc::new(RealTransport::new()),
        )
    }

    pub fn new_with_transport(
        serial_config: SerialConfig,
        line_ending: String,
        app_config: AppConfig,
        transport: Arc<dyn Transport>,
    ) -> Self {
        let mut history =
            CommandHistory::from_config(app_config.history.max_entries, &app_config.history.file);
        let history_warning = history.take_last_error();
        let quicksend = Self::sanitize_quicksend(app_config.quicksend.recent.clone());
        let logger = SessionLogger::from_config(
            &app_config.logging.log_directory,
            &app_config.logging.log_format,
        );
        let mut macros = MacroManager::new();
        let macro_warning = macros.take_last_error();
        let mut app = Self::build(
            serial_config,
            line_ending,
            app_config,
            history,
            logger,
            macros,
            quicksend,
            transport,
        );
        if let Some(warning) = history_warning {
            app.add_status_warning(format!("History warning: {}", warning));
        }
        if let Some(warning) = macro_warning {
            app.add_status_warning(format!("Macro warning: {}", warning));
        }
        app
    }

    fn build(
        serial_config: SerialConfig,
        line_ending: String,
        app_config: AppConfig,
        history: CommandHistory,
        logger: SessionLogger,
        macros: MacroManager,
        quicksend: Vec<String>,
        transport: Arc<dyn Transport>,
    ) -> Self {
        let mut app_config = app_config;
        app_config.quicksend.recent = quicksend.clone();
        Self {
            mode: Mode::Input,
            return_mode: Mode::Input,
            should_quit: false,
            buffer: ScrollbackBuffer::new(app_config.behavior.scrollback_lines),
            input_text: String::new(),
            input_cursor: 0,
            line_ending,
            serial_config,
            connection_state: ConnectionState::Disconnected,
            connection: None,
            transport,
            serial_rx: None,
            serial_tx: None,
            scroll_offset: 0,
            follow_output: app_config.behavior.follow_output,
            rx_bytes: 0,
            tx_bytes: 0,
            available_ports: Vec::new(),
            port_select_index: 0,
            show_timestamps: app_config.display.timestamps,
            timestamp_format: app_config.display.timestamp_format.clone(),
            hex_mode: app_config.display.hex_mode,
            show_line_endings: app_config.display.show_line_endings,
            color_log_levels: app_config.display.color_log_levels,
            history,
            search: Search::new(),
            logger,
            auto_reconnect: app_config.behavior.auto_reconnect,
            reconnect_port: None,
            reconnect_delay: Duration::from_millis(app_config.behavior.reconnect_delay_ms),
            reconnect_current_delay: Duration::from_millis(app_config.behavior.reconnect_delay_ms)
                .min(MAX_RECONNECT_DELAY),
            reconnect_next_attempt: None,
            reconnect_attempts: 0,
            status_message: None,
            filter: LineFilter::new(),
            macros,
            macro_select_index: 0,
            settings_field: 0,
            settings_original_config: None,
            settings_original_line_ending: None,
            layout: LayoutRegions::default(),
            selection: TextSelection::new(),
            ghost_suggestion: None,
            app_config,
            last_command_sent: None,
            last_response_time: None,
            quicksend,
            show_sent: true,
            filter_input: String::new(),
            filter_mode_is_exclude: false,
            filter_select_index: 0,
            hex_input_mode: false,
            pending_macro_commands: VecDeque::new(),
            active_macro_name: None,
            last_executed_macro: None,
            help_scroll: 0,
            help_scroll_max: 0,
        }
    }

    fn sanitize_quicksend(entries: Vec<String>) -> Vec<String> {
        let mut sanitized = Vec::new();
        for entry in entries {
            let trimmed = entry.trim();
            if trimmed.is_empty() || sanitized.iter().any(|existing| existing == trimmed) {
                continue;
            }
            sanitized.push(trimmed.to_string());
            if sanitized.len() == 8 {
                break;
            }
        }
        sanitized
    }

    fn profile_for_port(&self, port_name: &str) -> DefaultsConfig {
        self.app_config
            .connection
            .port_profiles
            .get(port_name)
            .cloned()
            .unwrap_or_else(|| self.app_config.defaults.clone())
    }

    fn apply_defaults_profile(&mut self, profile: &DefaultsConfig) {
        self.serial_config = profile.to_serial_config();
        self.line_ending = profile.to_line_ending();
    }

    pub fn load_port_profile(&mut self, port_name: &str) {
        let profile = self.profile_for_port(port_name);
        self.apply_defaults_profile(&profile);
    }

    fn current_defaults_profile(&self) -> DefaultsConfig {
        DefaultsConfig::from_runtime(&self.serial_config, &self.line_ending)
    }

    fn save_global_defaults(&mut self) {
        self.app_config.defaults = self.current_defaults_profile();
        self.save_app_config();
    }

    fn save_port_profile(&mut self, port_name: &str) {
        self.app_config
            .connection
            .port_profiles
            .insert(port_name.to_string(), self.current_defaults_profile());
        self.save_app_config();
    }

    fn save_app_config(&mut self) {
        if let Err(err) = self.app_config.save() {
            self.set_status(format!("Config save error: {}", err));
        }
    }

    fn promote_quicksend(&mut self, command: &str) {
        let command = command.trim();
        if command.is_empty() {
            return;
        }

        self.quicksend.retain(|existing| existing != command);
        self.quicksend.insert(0, command.to_string());
        self.quicksend.truncate(8);
        self.app_config.quicksend.recent = self.quicksend.clone();
        self.save_app_config();
    }

    pub fn rerun_last_macro(&mut self) {
        if self.active_macro_name.is_some() {
            self.set_status("A macro is already running".to_string());
            return;
        }

        if let Some(name) = self.last_executed_macro.clone() {
            self.execute_macro(&name);
        } else {
            self.set_status("No macro has been run yet".to_string());
        }
    }

    pub fn reload_macros(&mut self) {
        if self.active_macro_name.is_some() {
            self.set_status("Cannot reload macros while a macro is running".to_string());
            return;
        }

        match self.macros.reload() {
            Ok(()) => {
                let macro_count = self.macros.list().len();
                if macro_count == 0 {
                    self.macro_select_index = 0;
                } else if self.macro_select_index >= macro_count {
                    self.macro_select_index = macro_count - 1;
                }
                self.set_status("Macros reloaded".to_string());
            }
            Err(err) => {
                self.set_status(format!("Macro reload error: {}", err));
            }
        }
    }

    fn open_overlay(&mut self, overlay_mode: Mode) {
        self.return_mode = if self.mode.is_overlay() {
            self.return_mode
        } else {
            self.mode
        };
        self.mode = overlay_mode;
    }

    pub fn restore_mode(&mut self) {
        self.mode = self.return_mode;
    }

    /// Send a command string over the serial port.
    pub fn send_command(&mut self) {
        if self.input_text.is_empty() {
            return;
        }

        let text = self.input_text.clone();

        // Hex input mode: parse space-separated hex bytes, send raw binary
        if self.hex_input_mode {
            match Self::parse_hex_bytes(&text) {
                Ok(bytes) => {
                    if self.show_sent {
                        self.buffer.push_sent_line(format!("HEX: {}", text));
                    }
                    if let Some(conn) = self.connection.as_mut() {
                        match conn.write(&bytes) {
                            Ok(written) => {
                                self.tx_bytes += written as u64;
                                self.last_command_sent = Some(Instant::now());
                            }
                            Err(err) => self.handle_write_error(err.to_string()),
                        }
                    }
                    self.record_history(text);
                }
                Err(e) => {
                    self.set_status(format!("Hex parse error: {}", e));
                    return; // Don't clear input on error
                }
            }
        } else {
            self.send_plain_text(&text, SendSource::Manual);
        }

        self.input_text.clear();
        self.input_cursor = 0;
        self.ghost_suggestion = None;
    }

    /// Send a quick-send command by index (0-based).
    pub fn send_quicksend(&mut self, index: usize) {
        if let Some(cmd) = self.quicksend.get(index).cloned() {
            self.send_plain_text(&cmd, SendSource::Quicksend);
        }
    }

    fn send_plain_text(&mut self, text: &str, source: SendSource) {
        let line_ending = self.line_ending.clone();
        let data = format!("{}{}", text, line_ending);

        if self.show_sent {
            self.buffer.push_sent_line(text.to_string());
        }

        if let Some(conn) = self.connection.as_mut() {
            match conn.write(data.as_bytes()) {
                Ok(written) => {
                    self.tx_bytes += written as u64;
                    self.last_command_sent = Some(Instant::now());
                }
                Err(err) => self.handle_write_error(err.to_string()),
            }
        }

        if source != SendSource::Macro {
            self.record_history(text.to_string());
        }

        if source == SendSource::Manual {
            self.promote_quicksend(text);
        }
    }

    // ── Toggles ─────────────────────────────────────────

    pub fn toggle_hex_mode(&mut self) {
        self.hex_mode = !self.hex_mode;
    }

    pub fn toggle_line_endings(&mut self) {
        self.show_line_endings = !self.show_line_endings;
    }

    pub fn toggle_logging(&mut self) {
        match self.logger.toggle() {
            Ok(Some(path)) => {
                self.set_status(format!("Logging to {}", path.display()));
            }
            Ok(None) => {
                self.set_status("Logging stopped".to_string());
            }
            Err(e) => {
                self.set_status(format!("Log error: {}", e));
            }
        }
    }

    /// Clear the scrollback buffer.
    pub fn clear_buffer(&mut self) {
        self.buffer.clear();
        self.scroll_offset = 0;
        self.follow_output = true;
        self.search.deactivate();
        self.set_status("Buffer cleared".to_string());
    }

    // ── Status ──────────────────────────────────────────

    fn set_status(&mut self, msg: String) {
        self.status_message = Some((msg, Instant::now()));
    }

    pub fn set_status_pub(&mut self, msg: String) {
        self.set_status(msg);
    }

    pub fn add_status_warning(&mut self, msg: String) {
        let combined = match self.status_message.take() {
            Some((current, _)) => format!("{}; {}", current, msg),
            None => msg,
        };
        self.set_status(combined);
    }

    fn record_history(&mut self, command: String) {
        if let Err(err) = self.history.push(command) {
            self.set_status(format!("History save error: {}", err));
        }
        self.history.reset_navigation();
    }

    pub fn total_rx_bytes(&self) -> u64 {
        self.rx_bytes
    }

    pub fn total_tx_bytes(&self) -> u64 {
        self.tx_bytes
    }

    pub fn is_connected(&self) -> bool {
        matches!(self.connection_state, ConnectionState::Connected(_))
    }

    pub fn is_reconnecting(&self) -> bool {
        matches!(self.connection_state, ConnectionState::Reconnecting(_))
    }

    // ── Hex input ───────────────────────────────────────

    /// Toggle hex input mode.
    pub fn toggle_hex_input(&mut self) {
        self.hex_input_mode = !self.hex_input_mode;
        if self.hex_input_mode {
            self.set_status("Hex input mode ON — type space-separated hex bytes".to_string());
        } else {
            self.set_status("Hex input mode OFF".to_string());
        }
    }

    /// Parse a hex string into raw bytes.
    /// Accepts space-separated hex pairs: "01 FF A0" or "01FFA0"
    fn parse_hex_bytes(input: &str) -> Result<Vec<u8>, String> {
        let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
        if cleaned.is_empty() {
            return Err("Empty hex input".to_string());
        }
        if cleaned.len() & 1 != 0 {
            return Err("Odd number of hex digits".to_string());
        }
        let mut bytes = Vec::with_capacity(cleaned.len() / 2);
        for i in (0..cleaned.len()).step_by(2) {
            let byte_str = &cleaned[i..i + 2];
            match u8::from_str_radix(byte_str, 16) {
                Ok(b) => bytes.push(b),
                Err(_) => return Err(format!("Invalid hex byte: {}", byte_str)),
            }
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::handle_key_event;
    use crate::logging::LogFormat;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use serialport::{DataBits, FlowControl, Parity, StopBits};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn unique_temp_path(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "yapper-test-{}-{}-{}",
            prefix,
            std::process::id(),
            TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn test_app_with_config(app_config: AppConfig) -> App {
        let mut app_config = app_config;
        let default_config = AppConfig::default();
        if app_config.history.file == default_config.history.file {
            app_config.history.file = unique_temp_path("history").display().to_string();
        }
        if app_config.logging.log_directory == default_config.logging.log_directory {
            app_config.logging.log_directory = unique_temp_path("logs").display().to_string();
        }

        let history =
            CommandHistory::from_config(app_config.history.max_entries, &app_config.history.file);
        let quicksend = App::sanitize_quicksend(app_config.quicksend.recent.clone());
        let logger = SessionLogger::from_config(
            &app_config.logging.log_directory,
            &app_config.logging.log_format,
        );
        let macros = MacroManager::new_in_memory();

        App::build(
            SerialConfig::default(),
            "\r\n".to_string(),
            app_config,
            history,
            logger,
            macros,
            quicksend,
            Arc::new(RealTransport::new()),
        )
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(code: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(code), KeyModifiers::CONTROL)
    }

    #[test]
    fn test_app_honors_config_backed_startup_state() {
        let mut config = AppConfig::default();
        config.display.timestamps = false;
        config.display.timestamp_format = "%M:%S".to_string();
        config.display.color_log_levels = false;
        config.display.show_line_endings = true;
        config.display.hex_mode = true;
        config.behavior.auto_reconnect = false;
        config.behavior.reconnect_delay_ms = 2500;
        config.behavior.scrollback_lines = 321;
        config.behavior.follow_output = false;
        config.history.max_entries = 42;
        config.history.file = "/tmp/yapper-history-test".to_string();
        config.logging.auto_log = true;
        config.logging.log_directory = "/tmp/yapper-logs-test".to_string();
        config.logging.log_format = "raw".to_string();

        let app = test_app_with_config(config);

        assert!(!app.show_timestamps);
        assert_eq!(app.timestamp_format, "%M:%S");
        assert!(!app.color_log_levels);
        assert!(app.show_line_endings);
        assert!(app.hex_mode);
        assert!(!app.auto_reconnect);
        assert_eq!(app.reconnect_delay, Duration::from_millis(2500));
        assert_eq!(app.buffer.max_lines(), 321);
        assert!(!app.follow_output);
        assert_eq!(app.history.max_entries(), 42);
        assert_eq!(
            app.history.file_path(),
            Some(&PathBuf::from("/tmp/yapper-history-test"))
        );
        assert_eq!(
            app.logger.log_dir(),
            Some(&PathBuf::from("/tmp/yapper-logs-test"))
        );
        assert_eq!(app.logger.format(), LogFormat::Raw);
        assert!(app.app_config.logging.auto_log);
    }

    #[test]
    fn test_macro_scheduler_runs_commands_over_multiple_ticks() {
        let mut app = test_app_with_config(AppConfig::default());
        app.macros
            .save_macro(crate::macros::Macro {
                name: "wifi".to_string(),
                description: "Bring WiFi up".to_string(),
                commands: vec![
                    crate::macros::MacroCommand {
                        text: "AT+CWMODE=1".to_string(),
                        delay_ms: 0,
                    },
                    crate::macros::MacroCommand {
                        text: "AT+CWJAP".to_string(),
                        delay_ms: 500,
                    },
                ],
            })
            .unwrap();

        app.execute_macro("wifi");

        assert_eq!(app.pending_macro_commands.len(), 2);
        assert_eq!(app.active_macro_name.as_deref(), Some("wifi"));

        let first_ready = app.pending_macro_commands.front().unwrap().ready_at;
        assert!(app.tick(first_ready));
        assert_eq!(app.pending_macro_commands.len(), 1);
        assert_eq!(app.active_macro_name.as_deref(), Some("wifi"));

        let second_ready = app.pending_macro_commands.front().unwrap().ready_at;
        assert!(!app.tick(second_ready - Duration::from_millis(1)));
        assert!(app.tick(second_ready));
        assert!(app.pending_macro_commands.is_empty());
        assert!(app.active_macro_name.is_none());
    }

    #[test]
    fn test_port_selector_restore_mode_on_escape() {
        let mut app = test_app_with_config(AppConfig::default());
        app.mode = Mode::Input;
        app.return_mode = Mode::Input;
        app.mode = Mode::PortSelect;

        handle_key_event(&mut app, key(KeyCode::Esc));

        assert_eq!(app.mode, Mode::Input);
    }

    #[test]
    fn test_settings_cancel_restores_input_mode_and_values() {
        let mut app = test_app_with_config(AppConfig::default());
        app.mode = Mode::Input;
        app.open_settings();
        app.settings_field = 0;
        app.settings_next_value();
        app.line_ending = "\n".to_string();

        handle_key_event(&mut app, key(KeyCode::Esc));

        assert_eq!(app.mode, Mode::Input);
        assert_eq!(app.serial_config, SerialConfig::default());
        assert_eq!(app.line_ending, "\r\n");
    }

    #[test]
    fn test_help_and_filter_popups_restore_previous_mode() {
        let mut app = test_app_with_config(AppConfig::default());
        app.mode = Mode::Normal;
        app.open_help();
        handle_key_event(&mut app, key(KeyCode::Esc));
        assert_eq!(app.mode, Mode::Normal);

        app.open_filter_popup();
        handle_key_event(&mut app, key(KeyCode::Char('E')));
        handle_key_event(&mut app, key(KeyCode::Char('R')));
        handle_key_event(&mut app, key(KeyCode::Char('R')));
        handle_key_event(&mut app, key(KeyCode::Enter));
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.filter.count(), 1);
    }

    #[test]
    fn test_search_and_macro_popups_restore_previous_mode() {
        let mut app = test_app_with_config(AppConfig::default());
        app.mode = Mode::Normal;
        app.start_search();
        handle_key_event(&mut app, key(KeyCode::Enter));
        assert_eq!(app.mode, Mode::Normal);

        app.macros
            .save_macro(crate::macros::Macro {
                name: "reset".to_string(),
                description: "Reset".to_string(),
                commands: vec![crate::macros::MacroCommand {
                    text: "AT+RST".to_string(),
                    delay_ms: 0,
                }],
            })
            .unwrap();
        app.open_macro_selector();
        handle_key_event(&mut app, key(KeyCode::Enter));
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn test_settings_reconnect_only_for_transport_changes() {
        let config = SerialConfig::default();
        assert!(!App::settings_require_reconnect(
            &config, &config, "\r\n", "\n"
        ));

        let changed = SerialConfig {
            baud_rate: 9600,
            data_bits: DataBits::Seven,
            parity: Parity::Even,
            stop_bits: StopBits::Two,
            flow_control: FlowControl::Hardware,
        };
        assert!(App::settings_require_reconnect(
            &config, &changed, "\r\n", "\r\n",
        ));
    }

    #[test]
    fn test_filter_navigation_and_delete_work_while_typing() {
        let mut app = test_app_with_config(AppConfig::default());
        app.add_filter_include("ERROR");
        app.add_filter_exclude("DEBUG");
        app.mode = Mode::Normal;
        app.open_filter_popup();
        app.filter_input = "WARN".to_string();
        app.filter_select_index = 1;

        handle_key_event(&mut app, key(KeyCode::Up));
        assert_eq!(app.filter_select_index, 0);

        handle_key_event(&mut app, key(KeyCode::Delete));
        assert_eq!(app.filter.count(), 1);

        app.filter_select_index = 0;
        handle_key_event(&mut app, ctrl('d'));
        assert_eq!(app.filter.count(), 0);
    }

    #[test]
    fn test_quicksend_manual_send_builds_mru_and_fkey_send_is_stable() {
        let mut app = test_app_with_config(AppConfig::default());

        app.send_plain_text("AT", SendSource::Manual);
        app.send_plain_text("RST", SendSource::Manual);
        app.send_plain_text("AT", SendSource::Manual);

        assert_eq!(app.quicksend, vec!["AT".to_string(), "RST".to_string()]);

        let before = app.quicksend.clone();
        app.send_quicksend(1);
        assert_eq!(app.quicksend, before);

        app.send_plain_text("JOIN", SendSource::Macro);
        assert_eq!(app.quicksend, before);
        assert_eq!(app.app_config.quicksend.recent, before);
    }

    #[test]
    fn test_unicode_input_editing_uses_character_cursor() {
        let mut app = test_app_with_config(AppConfig::default());

        app.input_char('é');
        app.input_char('界');
        assert_eq!(app.input_text, "é界");
        assert_eq!(app.input_cursor, 2);

        app.input_cursor_left();
        app.input_delete();
        assert_eq!(app.input_text, "é");
        assert_eq!(app.input_cursor, 1);

        app.input_backspace();
        assert_eq!(app.input_text, "");
        assert_eq!(app.input_cursor, 0);
    }

    #[test]
    fn test_load_port_profile_prefers_saved_settings() {
        let mut config = AppConfig::default();
        config.defaults.baud_rate = 115200;
        config.defaults.line_ending = "crlf".to_string();
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

        let mut app = test_app_with_config(config);
        app.load_port_profile("/dev/ttyUSB0");

        assert_eq!(app.serial_config.baud_rate, 9600);
        assert_eq!(app.serial_config.data_bits, DataBits::Seven);
        assert_eq!(app.serial_config.parity, Parity::Even);
        assert_eq!(app.serial_config.stop_bits, StopBits::Two);
        assert_eq!(app.serial_config.flow_control, FlowControl::Hardware);
        assert_eq!(app.line_ending, "\n");
    }

    #[test]
    fn test_apply_settings_saves_connected_port_profile_only() {
        let config = AppConfig::default();
        let mut app = test_app_with_config(config);
        app.connection_state = ConnectionState::Connected("/dev/ttyUSB0".to_string());
        app.settings_original_config = Some(SerialConfig::default());
        app.settings_original_line_ending = Some("\r\n".to_string());
        app.serial_config.baud_rate = 57600;
        app.line_ending = "\n".to_string();

        app.apply_settings();

        assert_eq!(app.app_config.defaults.baud_rate, 115200);
        let profile = app
            .app_config
            .connection
            .port_profiles
            .get("/dev/ttyUSB0")
            .unwrap();
        assert_eq!(profile.baud_rate, 57600);
        assert_eq!(profile.line_ending, "lf");
    }

    #[test]
    fn test_apply_settings_saves_global_defaults_when_disconnected() {
        let config = AppConfig::default();
        let mut app = test_app_with_config(config);
        app.settings_original_config = Some(SerialConfig::default());
        app.settings_original_line_ending = Some("\r\n".to_string());
        app.serial_config.baud_rate = 38400;
        app.line_ending = "\r".to_string();

        app.apply_settings();

        assert_eq!(app.app_config.defaults.baud_rate, 38400);
        assert_eq!(app.app_config.defaults.line_ending, "cr");
        assert!(app.app_config.connection.port_profiles.is_empty());
    }

    #[test]
    fn test_apply_detected_baud_updates_port_profile_without_global_defaults() {
        let config = AppConfig::default();
        let mut app = test_app_with_config(config);

        app.apply_detected_baud("/dev/ttyUSB1", 230400);

        assert_eq!(app.serial_config.baud_rate, 230400);
        assert_eq!(app.app_config.defaults.baud_rate, 115200);
        assert_eq!(
            app.app_config
                .connection
                .port_profiles
                .get("/dev/ttyUSB1")
                .unwrap()
                .baud_rate,
            230400
        );
    }

    #[test]
    fn test_reconnect_backoff_grows_and_resets_on_cancel() {
        let mut config = AppConfig::default();
        config.behavior.reconnect_delay_ms = 250;
        let mut app = test_app_with_config(config);
        let now = Instant::now();

        app.schedule_reconnect("__missing_port__".to_string(), now);
        assert_eq!(app.reconnect_status(now).unwrap().0, 1);
        assert_eq!(
            app.reconnect_status(now).unwrap().1,
            Duration::from_millis(250)
        );

        assert!(!app.tick(now + Duration::from_millis(249)));
        assert_eq!(app.reconnect_attempts, 0);

        let mut scheduled = now + Duration::from_millis(250);
        let expected_delays = [
            Duration::from_millis(500),
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(4),
            Duration::from_secs(8),
            Duration::from_secs(10),
            Duration::from_secs(10),
        ];

        for expected_delay in expected_delays {
            assert!(!app.tick(scheduled));
            assert_eq!(app.reconnect_current_delay, expected_delay);
            scheduled = app.reconnect_next_attempt.unwrap();
        }

        app.toggle_connection();
        assert_eq!(app.connection_state, ConnectionState::Disconnected);
        assert!(app.reconnect_next_attempt.is_none());
        assert_eq!(app.reconnect_attempts, 0);
    }

    #[test]
    fn test_write_error_uses_connection_loss_path() {
        let mut app = test_app_with_config(AppConfig::default());
        app.auto_reconnect = true;
        app.connection_state = ConnectionState::Connected("/dev/ttyUSB0".to_string());

        app.handle_write_error("broken pipe".to_string());

        assert!(app.is_reconnecting());
        assert_eq!(app.reconnect_port.as_deref(), Some("/dev/ttyUSB0"));
    }

    #[test]
    fn test_rerun_last_macro_and_reload_macros() {
        let macros_path = unique_temp_path("macros.toml");
        fs::write(
            &macros_path,
            r#"
                [[macros]]
                name = "reset"
                description = "Reset"
                commands = ["AT+RST"]
            "#,
        )
        .unwrap();

        let mut config = AppConfig::default();
        config.history.file = unique_temp_path("history").display().to_string();
        config.logging.log_directory = unique_temp_path("logs").display().to_string();
        let history = CommandHistory::from_config(config.history.max_entries, &config.history.file);
        let quicksend = App::sanitize_quicksend(config.quicksend.recent.clone());
        let logger =
            SessionLogger::from_config(&config.logging.log_directory, &config.logging.log_format);
        let mut macros = MacroManager::with_path(Some(macros_path.clone()));
        macros.reload().unwrap();
        let mut app = App::build(
            SerialConfig::default(),
            "\r\n".to_string(),
            config,
            history,
            logger,
            macros,
            quicksend,
            Arc::new(RealTransport::new()),
        );

        app.execute_macro("reset");
        assert_eq!(app.last_executed_macro.as_deref(), Some("reset"));
        app.rerun_last_macro();
        assert_eq!(
            app.status_message.as_ref().map(|(msg, _)| msg.as_str()),
            Some("A macro is already running")
        );

        let first_ready = app.pending_macro_commands.front().unwrap().ready_at;
        assert!(app.tick(first_ready));

        app.rerun_last_macro();
        assert_eq!(app.active_macro_name.as_deref(), Some("reset"));

        app.reload_macros();
        assert_eq!(
            app.status_message.as_ref().map(|(msg, _)| msg.as_str()),
            Some("Cannot reload macros while a macro is running")
        );

        let rerun_ready = app.pending_macro_commands.front().unwrap().ready_at;
        assert!(app.tick(rerun_ready));

        fs::write(
            &macros_path,
            r#"
                [[macros]]
                name = "version"
                description = "Version"
                commands = ["AT+GMR"]
            "#,
        )
        .unwrap();
        app.reload_macros();

        assert!(app.macros.get("version").is_some());
        assert!(app.macros.get("reset").is_none());
    }

    #[test]
    fn test_help_scroll_clamps() {
        let mut app = test_app_with_config(AppConfig::default());
        app.set_help_scroll_max(3);
        app.scroll_help_down(10);
        assert_eq!(app.help_scroll, 3);
        app.scroll_help_up(10);
        assert_eq!(app.help_scroll, 0);
    }
}
