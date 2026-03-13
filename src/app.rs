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
}

/// Connection state for display purposes.
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connected(String),
    Reconnecting(String),
    Error(String),
}

/// Central application state.
pub struct App {
    /// Current input mode.
    pub mode: Mode,
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
    /// Whether hex view mode is enabled.
    pub hex_mode: bool,
    /// Whether to show line ending indicators.
    pub show_line_endings: bool,
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
}

impl App {
    pub fn new(serial_config: SerialConfig, line_ending: String, app_config: AppConfig) -> Self {
        Self {
            mode: Mode::Input,
            should_quit: false,
            buffer: ScrollbackBuffer::new(10000),
            input_text: String::new(),
            input_cursor: 0,
            line_ending,
            serial_config,
            connection_state: ConnectionState::Disconnected,
            connection: None,
            serial_rx: None,
            serial_tx: None,
            scroll_offset: 0,
            follow_output: true,
            rx_bytes: 0,
            tx_bytes: 0,
            available_ports: Vec::new(),
            port_select_index: 0,
            show_timestamps: true,
            hex_mode: false,
            show_line_endings: false,
            history: CommandHistory::new(500),
            search: Search::new(),
            logger: SessionLogger::new(),
            auto_reconnect: true,
            reconnect_port: None,
            last_reconnect_attempt: None,
            reconnect_delay: Duration::from_secs(1),
            status_message: None,
            filter: LineFilter::new(),
            macros: MacroManager::new(),
            macro_select_index: 0,
            settings_field: 0,
            layout: LayoutRegions::default(),
            selection: TextSelection::new(),
            ghost_suggestion: None,
            app_config,
            last_command_sent: None,
            last_response_time: None,
            quicksend: Vec::new(),
            show_sent: true,
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
                self.set_status(format!("Connected to {}", port_name));
                // Save last port for auto-connect on next launch
                self.app_config.connection.last_port = Some(port_name.to_string());
                self.app_config.save();
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

    /// Send a command string over the serial port.
    pub fn send_command(&mut self) {
        if self.input_text.is_empty() {
            return;
        }

        let text = self.input_text.clone();
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
                    self.connection_state =
                        ConnectionState::Error("Write failed".to_string());
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

        // Clear expired status messages
        if let Some((_, time)) = &self.status_message {
            if time.elapsed() > Duration::from_secs(3) {
                self.status_message = None;
                changed = true;
            }
        }

        let rx = match &self.serial_rx {
            Some(rx) => rx,
            None => {
                // Try auto-reconnect if needed
                self.try_reconnect();
                return changed;
            }
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

    /// Attempt auto-reconnect if conditions are met.
    fn try_reconnect(&mut self) {
        let port = match &self.reconnect_port {
            Some(p) => p.clone(),
            None => return,
        };

        if !matches!(self.connection_state, ConnectionState::Reconnecting(_)) {
            return;
        }

        // Check if enough time has passed since last attempt
        if let Some(last) = &self.last_reconnect_attempt {
            if last.elapsed() < self.reconnect_delay {
                return;
            }
        }

        self.last_reconnect_attempt = Some(Instant::now());

        let (tx, rx) = mpsc::channel();
        match SerialConnection::open(&port, &self.serial_config, tx.clone()) {
            Ok(conn) => {
                self.connection_state = ConnectionState::Connected(port.clone());
                self.connection = Some(conn);
                self.serial_rx = Some(rx);
                self.serial_tx = Some(tx);
                self.set_status(format!("Reconnected to {}", port));
            }
            Err(_) => {
                // Will retry on next tick
            }
        }
    }

    /// Open the port selector popup.
    pub fn open_port_selector(&mut self) {
        self.available_ports = detector::available_ports();
        self.port_select_index = 0;
        self.mode = Mode::PortSelect;
    }

    /// Connect to the currently selected port in the selector.
    pub fn connect_selected_port(&mut self) {
        if let Some(port) = self.available_ports.get(self.port_select_index) {
            let port_name = port.name.clone();
            self.mode = Mode::Normal;
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
        self.ghost_suggestion = self.history.suggest(&self.input_text).map(|s| s.to_string());
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
        self.mode = Mode::Search;
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
        self.mode = Mode::Normal;
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

    // ── Macros ──────────────────────────────────────────

    /// Send raw text over serial (used by macros).
    pub fn send_text(&mut self, text: &str) {
        let line_ending = self.line_ending.clone();
        let data = format!("{}{}", text, line_ending);

        if let Some(conn) = &self.connection {
            match conn.write(data.as_bytes()) {
                Ok(_) => {
                    self.tx_bytes = conn.tx_bytes();
                }
                Err(_) => {
                    self.connection_state =
                        ConnectionState::Error("Write failed".to_string());
                }
            }
        }
    }

    /// Execute a macro by name.
    pub fn execute_macro(&mut self, name: &str) {
        if let Some(m) = self.macros.get(name) {
            let commands: Vec<String> = m.commands.iter().map(|c| c.text.clone()).collect();
            self.set_status(format!("Running macro: {}", name));
            for cmd in commands {
                self.send_text(&cmd);
            }
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

    // ── Settings ────────────────────────────────────────

    pub fn open_settings(&mut self) {
        self.settings_field = 0;
        self.mode = Mode::Settings;
    }

    /// Cycle the selected settings field value forward.
    pub fn settings_next_value(&mut self) {
        use serialport::*;
        match self.settings_field {
            0 => {
                // Baud rate: cycle through common rates
                let rates = crate::ui::settings::BAUD_RATES;
                let current_idx = rates.iter().position(|&r| r == self.serial_config.baud_rate);
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
            _ => {}
        }
    }

    /// Cycle the selected settings field value backward.
    pub fn settings_prev_value(&mut self) {
        use serialport::*;
        match self.settings_field {
            0 => {
                let rates = crate::ui::settings::BAUD_RATES;
                let current_idx = rates.iter().position(|&r| r == self.serial_config.baud_rate);
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
            _ => {}
        }
    }

    /// Apply settings changes: reconnect if currently connected.
    pub fn apply_settings(&mut self) {
        self.mode = Mode::Normal;
        let summary = self.serial_config.summary();
        self.set_status(format!("Settings: {}", summary));

        // If connected, reconnect with new settings
        if let ConnectionState::Connected(port) = &self.connection_state {
            let port = port.clone();
            self.disconnect();
            self.connect(&port);
        }
    }
}
