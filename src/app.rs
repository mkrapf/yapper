use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use crate::buffer::ScrollbackBuffer;
use crate::config::AppConfig;
use crate::filter::LineFilter;
use crate::history::CommandHistory;
use crate::logging::SessionLogger;
use crate::macros::MacroManager;
use crate::mouse::{LayoutRegions, TextSelection};
use crate::search::Search;
use crate::serial::config::SerialConfig;
use crate::serial::connection::{SerialConnection, SerialEvent};
use crate::serial::detector::{self, PortInfo};

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
    connection: Option<SerialConnection>,
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
    /// When the last reconnect attempt was made.
    last_reconnect_attempt: Option<Instant>,
    /// Reconnect delay.
    reconnect_delay: Duration,
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
    /// Quick-send commands (most frequently used from history).
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
}

impl App {
    pub fn new(serial_config: SerialConfig, line_ending: String, app_config: AppConfig) -> Self {
        let history =
            CommandHistory::from_config(app_config.history.max_entries, &app_config.history.file);
        let quicksend = history.top_commands(8);
        let logger = SessionLogger::from_config(
            &app_config.logging.log_directory,
            &app_config.logging.log_format,
        );
        let macros = MacroManager::new();
        Self::build(
            serial_config,
            line_ending,
            app_config,
            history,
            logger,
            macros,
            quicksend,
        )
    }

    fn build(
        serial_config: SerialConfig,
        line_ending: String,
        app_config: AppConfig,
        history: CommandHistory,
        logger: SessionLogger,
        macros: MacroManager,
        quicksend: Vec<String>,
    ) -> Self {
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
            last_reconnect_attempt: None,
            reconnect_delay: Duration::from_millis(app_config.behavior.reconnect_delay_ms),
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
        }
    }

    /// Connect to the specified serial port.
    pub fn connect(&mut self, port_name: &str) {
        // Disconnect first if already connected
        self.disconnect_internal(false);

        let (tx, rx) = mpsc::channel();

        match SerialConnection::open(port_name, &self.serial_config, tx.clone()) {
            Ok(conn) => {
                self.connection_state = ConnectionState::Connected(port_name.to_string());
                self.connection = Some(conn);
                self.serial_rx = Some(rx);
                self.serial_tx = Some(tx);
                self.reconnect_port = Some(port_name.to_string());
                self.last_reconnect_attempt = None;
                self.set_status(format!("Connected to {}", port_name));
                // Save last port for auto-connect on next launch
                self.app_config.connection.last_port = Some(port_name.to_string());
                self.app_config.save();
                self.ensure_auto_logging();
            }
            Err(e) => {
                self.connection_state = ConnectionState::Error(e.to_string());
            }
        }
    }

    /// Internal disconnect, optionally preserving reconnect state.
    fn disconnect_internal(&mut self, keep_reconnect: bool) {
        if let Some(conn) = self.connection.take() {
            self.rx_bytes += conn.rx_bytes;
            self.tx_bytes += conn.tx_bytes();
            conn.close();
        }
        self.serial_rx = None;
        self.serial_tx = None;
        if !keep_reconnect {
            self.connection_state = ConnectionState::Disconnected;
            self.reconnect_port = None;
        }
    }

    /// Disconnect from the current serial port.
    pub fn disconnect(&mut self) {
        self.disconnect_internal(false);
        self.set_status("Disconnected".to_string());
    }

    /// Toggle connection: disconnect if connected, open port selector if not.
    pub fn toggle_connection(&mut self) {
        match &self.connection_state {
            ConnectionState::Connected(_) => {
                self.disconnect();
            }
            ConnectionState::Reconnecting(_) => {
                // Cancel reconnection
                self.reconnect_port = None;
                self.connection_state = ConnectionState::Disconnected;
                self.set_status("Reconnection cancelled".to_string());
            }
            _ => {
                self.open_port_selector();
            }
        }
    }

    fn ensure_auto_logging(&mut self) {
        if !self.app_config.logging.auto_log || self.logger.is_active {
            return;
        }

        if let Err(err) = self.logger.start() {
            self.set_status(format!("Log error: {}", err));
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
                    if let Some(conn) = &self.connection {
                        match conn.write(&bytes) {
                            Ok(_) => {
                                self.tx_bytes = conn.tx_bytes();
                                self.last_command_sent = Some(Instant::now());
                            }
                            Err(_) => {
                                self.connection_state =
                                    ConnectionState::Error("Write failed".to_string());
                            }
                        }
                    }
                }
                Err(e) => {
                    self.set_status(format!("Hex parse error: {}", e));
                    return; // Don't clear input on error
                }
            }
        } else {
            let line_ending = self.line_ending.clone();
            let data = format!("{}{}", text, line_ending);

            // Echo command to scrollback buffer
            if self.show_sent {
                self.buffer.push_sent_line(text.clone());
            }

            if let Some(conn) = &self.connection {
                match conn.write(data.as_bytes()) {
                    Ok(_) => {
                        self.tx_bytes = conn.tx_bytes();
                        self.last_command_sent = Some(Instant::now());
                    }
                    Err(_) => {
                        self.connection_state = ConnectionState::Error("Write failed".to_string());
                    }
                }
            }
        }

        // Add to history
        self.history.push(text);
        self.history.reset_navigation();

        self.input_text.clear();
        self.input_cursor = 0;
        self.ghost_suggestion = None;
        self.update_quicksend();
    }

    /// Update the quick-send command list from history frequency.
    pub fn update_quicksend(&mut self) {
        self.quicksend = self.history.top_commands(8);
    }

    /// Send a quick-send command by index (0-based).
    pub fn send_quicksend(&mut self, index: usize) {
        if let Some(cmd) = self.quicksend.get(index).cloned() {
            self.input_text = cmd;
            self.input_cursor = self.input_text.len();
            self.send_command();
        }
    }

    /// Poll for serial events. Returns true if anything happened (needs re-render).
    pub fn poll_serial(&mut self) -> bool {
        let mut changed = false;

        let rx = match &self.serial_rx {
            Some(rx) => rx,
            None => return changed,
        };

        // Drain all available events
        loop {
            match rx.try_recv() {
                Ok(SerialEvent::Data(data, received_at)) => {
                    self.rx_bytes += data.len() as u64;
                    self.logger.log_bytes(&data);
                    self.buffer.push_bytes(&data);
                    if self.follow_output {
                        self.scroll_offset = 0;
                    }
                    // Measure response time using reader-thread timestamp
                    if let Some(sent_at) = self.last_command_sent.take() {
                        self.last_response_time = Some(received_at.duration_since(sent_at));
                    }
                    changed = true;
                }
                Ok(SerialEvent::Disconnected) => {
                    let port = match &self.connection_state {
                        ConnectionState::Connected(p) => p.clone(),
                        _ => String::new(),
                    };
                    self.disconnect_internal(true);
                    if self.auto_reconnect && !port.is_empty() {
                        self.reconnect_port = Some(port.clone());
                        self.connection_state = ConnectionState::Reconnecting(port);
                        self.set_status("Port disconnected, reconnecting...".to_string());
                    } else {
                        self.connection_state =
                            ConnectionState::Error("Port disconnected".to_string());
                    }
                    break;
                }
                Ok(SerialEvent::Error(e)) => {
                    let port = match &self.connection_state {
                        ConnectionState::Connected(p) => p.clone(),
                        _ => String::new(),
                    };
                    self.disconnect_internal(true);
                    if self.auto_reconnect && !port.is_empty() {
                        self.reconnect_port = Some(port.clone());
                        self.connection_state = ConnectionState::Reconnecting(port);
                    } else {
                        self.connection_state = ConnectionState::Error(e);
                    }
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    let port = match &self.connection_state {
                        ConnectionState::Connected(p) => p.clone(),
                        _ => String::new(),
                    };
                    self.disconnect_internal(true);
                    if self.auto_reconnect && !port.is_empty() {
                        self.reconnect_port = Some(port.clone());
                        self.connection_state = ConnectionState::Reconnecting(port);
                    } else {
                        self.connection_state =
                            ConnectionState::Error("Reader thread died".to_string());
                    }
                    break;
                }
            }
        }

        changed
    }

    /// Run time-based background work. Returns true if state changed.
    pub fn tick(&mut self, now: Instant) -> bool {
        let mut changed = false;

        if let Some((_, time)) = &self.status_message {
            if time.elapsed() > Duration::from_secs(3) {
                self.status_message = None;
                changed = true;
            }
        }

        if self.try_reconnect(now) {
            changed = true;
        }

        if self.drain_macro_queue(now) {
            changed = true;
        }

        changed
    }

    /// Attempt auto-reconnect if conditions are met.
    fn try_reconnect(&mut self, now: Instant) -> bool {
        let port = match &self.reconnect_port {
            Some(p) => p.clone(),
            None => return false,
        };

        if !matches!(self.connection_state, ConnectionState::Reconnecting(_)) {
            return false;
        }

        // Check if enough time has passed since last attempt
        if let Some(last) = &self.last_reconnect_attempt {
            if last.elapsed() < self.reconnect_delay {
                return false;
            }
        }

        self.last_reconnect_attempt = Some(now);

        let (tx, rx) = mpsc::channel();
        match SerialConnection::open(&port, &self.serial_config, tx.clone()) {
            Ok(conn) => {
                self.connection_state = ConnectionState::Connected(port.clone());
                self.connection = Some(conn);
                self.serial_rx = Some(rx);
                self.serial_tx = Some(tx);
                self.set_status(format!("Reconnected to {}", port));
                self.last_reconnect_attempt = None;
                self.ensure_auto_logging();
                true
            }
            Err(_) => {
                // Will retry on next tick
                false
            }
        }
    }

    /// Auto-detect the baud rate for a given port.
    pub fn auto_detect_baud(&mut self, port_name: &str) {
        self.set_status("Auto-detecting baud rate...".to_string());
        match crate::serial::auto_detect::auto_detect_baud(port_name) {
            Some(rate) => {
                self.serial_config.baud_rate = rate;
                self.app_config.defaults.baud_rate = rate;
                self.app_config.save();
                self.set_status(format!("Detected baud rate: {}", rate));
            }
            None => {
                self.set_status("Could not detect baud rate — no readable data".to_string());
            }
        }
    }

    /// Open the port selector popup.
    pub fn open_port_selector(&mut self) {
        self.available_ports = detector::available_ports();
        self.port_select_index = 0;
        self.open_overlay(Mode::PortSelect);
    }

    /// Connect to the currently selected port in the selector.
    pub fn connect_selected_port(&mut self) {
        if let Some(port) = self.available_ports.get(self.port_select_index) {
            let port_name = port.name.clone();
            self.restore_mode();
            self.connect(&port_name);
        }
    }

    // ── Scrolling ───────────────────────────────────────

    pub fn scroll_up(&mut self, lines: usize) {
        let view_height = self.layout.terminal_view.3 as usize; // height from layout
        let max_scroll = self.buffer.display_len().saturating_sub(view_height);
        self.scroll_offset = (self.scroll_offset + lines).min(max_scroll);
        if max_scroll > 0 {
            self.follow_output = false;
        }
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
        if self.scroll_offset == 0 {
            self.follow_output = true;
        }
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
        self.follow_output = true;
    }

    pub fn scroll_to_top(&mut self) {
        let max_scroll = self.buffer.display_len().saturating_sub(1);
        self.scroll_offset = max_scroll;
        self.follow_output = false;
    }

    /// Scroll to show a specific line index.
    pub fn scroll_to_line(&mut self, line_index: usize) {
        let total = self.buffer.display_len();
        if total == 0 {
            return;
        }
        // scroll_offset is distance from bottom
        self.scroll_offset = total.saturating_sub(line_index + 1);
        self.follow_output = false;
    }

    // ── Input editing ───────────────────────────────────

    pub fn input_char(&mut self, c: char) {
        self.input_text.insert(self.input_cursor, c);
        self.input_cursor += 1;
        self.update_ghost();
    }

    pub fn input_backspace(&mut self) {
        if self.input_cursor > 0 {
            self.input_cursor -= 1;
            self.input_text.remove(self.input_cursor);
            self.update_ghost();
        }
    }

    pub fn input_delete(&mut self) {
        if self.input_cursor < self.input_text.len() {
            self.input_text.remove(self.input_cursor);
            self.update_ghost();
        }
    }

    pub fn input_cursor_left(&mut self) {
        self.input_cursor = self.input_cursor.saturating_sub(1);
    }

    pub fn input_cursor_right(&mut self) {
        self.input_cursor = (self.input_cursor + 1).min(self.input_text.len());
    }

    pub fn input_cursor_home(&mut self) {
        self.input_cursor = 0;
    }

    pub fn input_cursor_end(&mut self) {
        self.input_cursor = self.input_text.len();
    }

    /// Update the ghost auto-complete suggestion from history.
    pub fn update_ghost(&mut self) {
        // Only suggest when cursor is at the end of input
        if self.input_cursor != self.input_text.len() || self.input_text.is_empty() {
            self.ghost_suggestion = None;
            return;
        }
        self.ghost_suggestion = self
            .history
            .suggest(&self.input_text)
            .map(|s| s.to_string());
    }

    /// Accept the current ghost suggestion, filling in the input text.
    pub fn accept_suggestion(&mut self) {
        if let Some(suggestion) = self.ghost_suggestion.take() {
            self.input_text = suggestion;
            self.input_cursor = self.input_text.len();
        }
    }

    // ── History navigation ──────────────────────────────

    pub fn history_previous(&mut self) {
        if let Some(text) = self.history.previous(&self.input_text) {
            self.input_text = text.to_string();
            self.input_cursor = self.input_text.len();
        }
    }

    pub fn history_next(&mut self) {
        if let Some(text) = self.history.next() {
            self.input_text = text.to_string();
            self.input_cursor = self.input_text.len();
        }
    }

    // ── Search ──────────────────────────────────────────

    pub fn start_search(&mut self) {
        self.search.activate();
        self.open_overlay(Mode::Search);
    }

    pub fn search_char(&mut self, c: char) {
        self.search.push_char(c);
        self.search.execute(&self.buffer);
        // Jump to current match
        if let Some(line) = self.search.current_line() {
            self.scroll_to_line(line);
        }
    }

    pub fn search_backspace(&mut self) {
        self.search.pop_char();
        self.search.execute(&self.buffer);
    }

    pub fn search_next(&mut self) {
        if let Some(line) = self.search.next_match() {
            self.scroll_to_line(line);
        }
    }

    pub fn search_prev(&mut self) {
        if let Some(line) = self.search.prev_match() {
            self.scroll_to_line(line);
        }
    }

    pub fn end_search(&mut self) {
        self.search.deactivate();
        self.restore_mode();
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

    // ── Filter ──────────────────────────────────────────

    pub fn add_filter_include(&mut self, pattern: &str) {
        match self.filter.add_include(pattern) {
            Ok(_) => self.set_status(format!("Filter +{}", pattern)),
            Err(e) => self.set_status(e),
        }
    }

    pub fn add_filter_exclude(&mut self, pattern: &str) {
        match self.filter.add_exclude(pattern) {
            Ok(_) => self.set_status(format!("Filter -{}", pattern)),
            Err(e) => self.set_status(e),
        }
    }

    pub fn clear_filters(&mut self) {
        self.filter.clear();
        self.set_status("Filters cleared".to_string());
    }

    /// Open the filter popup.
    pub fn open_filter_popup(&mut self) {
        self.filter_input.clear();
        self.filter_select_index = 0;
        self.open_overlay(Mode::Filter);
    }

    /// Submit the current filter input.
    pub fn submit_filter(&mut self) {
        if !self.filter_input.is_empty() {
            let pattern = self.filter_input.clone();
            if self.filter_mode_is_exclude {
                self.add_filter_exclude(&pattern);
            } else {
                self.add_filter_include(&pattern);
            }
            self.filter_input.clear();
        }
        self.restore_mode();
    }

    /// Remove a filter by index.
    pub fn remove_filter(&mut self, index: usize) {
        self.filter.remove(index);
        if self.filter.count() == 0 {
            self.set_status("All filters removed".to_string());
        }
        // Keep select index in bounds
        if self.filter_select_index >= self.filter.count() && self.filter_select_index > 0 {
            self.filter_select_index -= 1;
        }
    }

    // ── Macros ──────────────────────────────────────────

    /// Open the macro selector popup.
    pub fn open_macro_selector(&mut self) {
        self.macro_select_index = 0;
        self.open_overlay(Mode::MacroSelect);
    }

    pub fn open_help(&mut self) {
        self.open_overlay(Mode::Help);
    }

    /// Send raw text over serial (used by macros).
    pub fn send_text(&mut self, text: &str) {
        let line_ending = self.line_ending.clone();
        let data = format!("{}{}", text, line_ending);

        if let Some(conn) = &self.connection {
            match conn.write(data.as_bytes()) {
                Ok(_) => {
                    self.tx_bytes = conn.tx_bytes();
                    self.last_command_sent = Some(Instant::now());
                }
                Err(_) => {
                    self.connection_state = ConnectionState::Error("Write failed".to_string());
                }
            }
        }
    }

    /// Execute a macro by name.
    pub fn execute_macro(&mut self, name: &str) {
        if self.active_macro_name.is_some() {
            self.set_status("A macro is already running".to_string());
            return;
        }

        if let Some(m) = self.macros.get(name) {
            let commands = m.commands.clone();
            let mut ready_at = Instant::now();
            let pending = commands.into_iter().map(|command| {
                ready_at += Duration::from_millis(command.delay_ms);
                PendingMacroCommand {
                    text: command.text,
                    ready_at,
                }
            });

            self.pending_macro_commands = pending.collect();
            self.active_macro_name = Some(name.to_string());
            self.set_status(format!("Running macro: {}", name));
        } else {
            self.set_status(format!("Macro not found: {}", name));
        }
    }

    /// Execute the currently selected macro.
    pub fn execute_selected_macro(&mut self) {
        let macros = self.macros.list();
        if let Some(m) = macros.get(self.macro_select_index) {
            let name = m.name.clone();
            self.execute_macro(&name);
        }
    }

    fn drain_macro_queue(&mut self, now: Instant) -> bool {
        let mut changed = false;

        loop {
            let ready = match self.pending_macro_commands.front() {
                Some(command) => command.ready_at <= now,
                None => false,
            };

            if !ready {
                break;
            }

            if let Some(command) = self.pending_macro_commands.pop_front() {
                self.send_text(&command.text);
                changed = true;
            }
        }

        if changed && self.pending_macro_commands.is_empty() {
            if let Some(name) = self.active_macro_name.take() {
                self.set_status(format!("Finished macro: {}", name));
            }
        }

        changed
    }

    // ── Settings ────────────────────────────────────────

    pub fn open_settings(&mut self) {
        self.settings_field = 0;
        self.settings_original_config = Some(self.serial_config.clone());
        self.settings_original_line_ending = Some(self.line_ending.clone());
        self.open_overlay(Mode::Settings);
    }

    /// Cycle the selected settings field value forward.
    pub fn settings_next_value(&mut self) {
        use serialport::*;
        match self.settings_field {
            0 => {
                // Baud rate: cycle through common rates
                let rates = crate::ui::settings::BAUD_RATES;
                let current_idx = rates
                    .iter()
                    .position(|&r| r == self.serial_config.baud_rate);
                let next_idx = match current_idx {
                    Some(i) => (i + 1) % rates.len(),
                    None => 0,
                };
                self.serial_config.baud_rate = rates[next_idx];
            }
            1 => {
                self.serial_config.data_bits = match self.serial_config.data_bits {
                    DataBits::Five => DataBits::Six,
                    DataBits::Six => DataBits::Seven,
                    DataBits::Seven => DataBits::Eight,
                    DataBits::Eight => DataBits::Five,
                };
            }
            2 => {
                self.serial_config.parity = match self.serial_config.parity {
                    Parity::None => Parity::Odd,
                    Parity::Odd => Parity::Even,
                    Parity::Even => Parity::None,
                };
            }
            3 => {
                self.serial_config.stop_bits = match self.serial_config.stop_bits {
                    StopBits::One => StopBits::Two,
                    StopBits::Two => StopBits::One,
                };
            }
            4 => {
                self.serial_config.flow_control = match self.serial_config.flow_control {
                    FlowControl::None => FlowControl::Software,
                    FlowControl::Software => FlowControl::Hardware,
                    FlowControl::Hardware => FlowControl::None,
                };
            }
            5 => {
                // Line ending cycle: CRLF -> LF -> CR
                self.line_ending = match self.line_ending.as_str() {
                    "\r\n" => "\n".to_string(),
                    "\n" => "\r".to_string(),
                    "\r" => "\r\n".to_string(),
                    _ => "\r\n".to_string(),
                };
            }
            _ => {}
        }
    }

    /// Cycle the selected settings field value backward.
    pub fn settings_prev_value(&mut self) {
        use serialport::*;
        match self.settings_field {
            0 => {
                let rates = crate::ui::settings::BAUD_RATES;
                let current_idx = rates
                    .iter()
                    .position(|&r| r == self.serial_config.baud_rate);
                let next_idx = match current_idx {
                    Some(0) | None => rates.len() - 1,
                    Some(i) => i - 1,
                };
                self.serial_config.baud_rate = rates[next_idx];
            }
            1 => {
                self.serial_config.data_bits = match self.serial_config.data_bits {
                    DataBits::Five => DataBits::Eight,
                    DataBits::Six => DataBits::Five,
                    DataBits::Seven => DataBits::Six,
                    DataBits::Eight => DataBits::Seven,
                };
            }
            2 => {
                self.serial_config.parity = match self.serial_config.parity {
                    Parity::None => Parity::Even,
                    Parity::Odd => Parity::None,
                    Parity::Even => Parity::Odd,
                };
            }
            3 => {
                self.serial_config.stop_bits = match self.serial_config.stop_bits {
                    StopBits::One => StopBits::Two,
                    StopBits::Two => StopBits::One,
                };
            }
            4 => {
                self.serial_config.flow_control = match self.serial_config.flow_control {
                    FlowControl::None => FlowControl::Hardware,
                    FlowControl::Software => FlowControl::None,
                    FlowControl::Hardware => FlowControl::Software,
                };
            }
            5 => {
                // Line ending cycle backward: CRLF -> CR -> LF
                self.line_ending = match self.line_ending.as_str() {
                    "\r\n" => "\r".to_string(),
                    "\n" => "\r\n".to_string(),
                    "\r" => "\n".to_string(),
                    _ => "\r\n".to_string(),
                };
            }
            _ => {}
        }
    }

    /// Apply settings changes: reconnect if currently connected.
    pub fn apply_settings(&mut self) {
        let original_config = self
            .settings_original_config
            .clone()
            .unwrap_or_else(|| self.serial_config.clone());
        let original_line_ending = self
            .settings_original_line_ending
            .clone()
            .unwrap_or_else(|| self.line_ending.clone());
        let summary = self.serial_config.summary();
        self.set_status(format!("Settings: {}", summary));

        // Persist all serial settings to config file
        self.sync_config_to_disk();

        // If connected, reconnect with new settings
        if let ConnectionState::Connected(port) = &self.connection_state {
            let port = port.clone();
            if Self::settings_require_reconnect(
                &original_config,
                &self.serial_config,
                &original_line_ending,
                &self.line_ending,
            ) {
                self.disconnect();
                self.connect(&port);
            }
        }

        self.settings_original_config = None;
        self.settings_original_line_ending = None;
        self.restore_mode();
    }

    pub fn cancel_settings(&mut self) {
        if let Some(original) = self.settings_original_config.take() {
            self.serial_config = original;
        }
        if let Some(original) = self.settings_original_line_ending.take() {
            self.line_ending = original;
        }
        self.restore_mode();
    }

    fn settings_require_reconnect(
        original_config: &SerialConfig,
        current_config: &SerialConfig,
        _original_line_ending: &str,
        _current_line_ending: &str,
    ) -> bool {
        original_config != current_config
    }

    /// Sync the current serial config and line ending to app_config and save to disk.
    fn sync_config_to_disk(&mut self) {
        self.app_config.defaults.baud_rate = self.serial_config.baud_rate;
        self.app_config.defaults.data_bits = match self.serial_config.data_bits {
            serialport::DataBits::Five => 5,
            serialport::DataBits::Six => 6,
            serialport::DataBits::Seven => 7,
            serialport::DataBits::Eight => 8,
        };
        self.app_config.defaults.parity = match self.serial_config.parity {
            serialport::Parity::None => "none".to_string(),
            serialport::Parity::Odd => "odd".to_string(),
            serialport::Parity::Even => "even".to_string(),
        };
        self.app_config.defaults.stop_bits = match self.serial_config.stop_bits {
            serialport::StopBits::One => 1,
            serialport::StopBits::Two => 2,
        };
        self.app_config.defaults.flow_control = match self.serial_config.flow_control {
            serialport::FlowControl::None => "none".to_string(),
            serialport::FlowControl::Software => "software".to_string(),
            serialport::FlowControl::Hardware => "hardware".to_string(),
        };
        self.app_config.defaults.line_ending = match self.line_ending.as_str() {
            "\n" => "lf".to_string(),
            "\r" => "cr".to_string(),
            _ => "crlf".to_string(),
        };
        self.app_config.save();
    }

    // ── Word-level cursor ───────────────────────────────

    /// Move cursor one word to the left.
    pub fn input_cursor_word_left(&mut self) {
        let chars: Vec<char> = self.input_text.chars().collect();
        if self.input_cursor == 0 {
            return;
        }
        let mut pos = self.input_cursor;
        // Skip non-alphanumeric
        while pos > 0 && !chars[pos - 1].is_alphanumeric() {
            pos -= 1;
        }
        // Skip alphanumeric
        while pos > 0 && chars[pos - 1].is_alphanumeric() {
            pos -= 1;
        }
        self.input_cursor = pos;
    }

    /// Move cursor one word to the right.
    pub fn input_cursor_word_right(&mut self) {
        let chars: Vec<char> = self.input_text.chars().collect();
        let len = chars.len();
        if self.input_cursor >= len {
            return;
        }
        let mut pos = self.input_cursor;
        // Skip alphanumeric
        while pos < len && chars[pos].is_alphanumeric() {
            pos += 1;
        }
        // Skip non-alphanumeric
        while pos < len && !chars[pos].is_alphanumeric() {
            pos += 1;
        }
        self.input_cursor = pos;
    }

    /// Delete one word backward (Ctrl+W).
    pub fn input_delete_word_back(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        let old_cursor = self.input_cursor;
        self.input_cursor_word_left();
        let new_cursor = self.input_cursor;
        // Remove characters between new_cursor and old_cursor
        let chars: Vec<char> = self.input_text.chars().collect();
        self.input_text = chars[..new_cursor]
            .iter()
            .chain(chars[old_cursor..].iter())
            .collect();
        self.update_ghost();
    }

    /// Kill the entire input line (Ctrl+U).
    pub fn input_kill_line(&mut self) {
        self.input_text.clear();
        self.input_cursor = 0;
        self.ghost_suggestion = None;
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
        if cleaned.len() % 2 != 0 {
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
    use std::path::PathBuf;

    fn test_app_with_config(app_config: AppConfig) -> App {
        let history =
            CommandHistory::from_config(app_config.history.max_entries, &app_config.history.file);
        let quicksend = history.top_commands(8);
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
        app.macros.save_macro(crate::macros::Macro {
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
        });

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

        app.macros.save_macro(crate::macros::Macro {
            name: "reset".to_string(),
            description: "Reset".to_string(),
            commands: vec![crate::macros::MacroCommand {
                text: "AT+RST".to_string(),
                delay_ms: 0,
            }],
        });
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
}
