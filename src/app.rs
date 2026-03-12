use std::sync::mpsc::{self, Receiver, Sender};

use crate::buffer::ScrollbackBuffer;
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
    /// Port selector popup is open.
    PortSelect,
    /// Help overlay is shown.
    Help,
}

/// Connection state for display purposes.
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connected(String),
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
}

impl App {
    pub fn new(serial_config: SerialConfig, line_ending: String) -> Self {
        Self {
            mode: Mode::Normal,
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
        }
    }

    /// Connect to the specified serial port.
    pub fn connect(&mut self, port_name: &str) {
        // Disconnect first if already connected
        self.disconnect();

        let (tx, rx) = mpsc::channel();

        match SerialConnection::open(port_name, &self.serial_config, tx.clone()) {
            Ok(conn) => {
                self.connection_state = ConnectionState::Connected(port_name.to_string());
                self.connection = Some(conn);
                self.serial_rx = Some(rx);
                self.serial_tx = Some(tx);
            }
            Err(e) => {
                self.connection_state = ConnectionState::Error(e.to_string());
            }
        }
    }

    /// Disconnect from the current serial port.
    pub fn disconnect(&mut self) {
        if let Some(conn) = self.connection.take() {
            // Capture byte counts before closing
            self.rx_bytes += conn.rx_bytes;
            self.tx_bytes += conn.tx_bytes;
            conn.close();
        }
        self.serial_rx = None;
        self.serial_tx = None;
        self.connection_state = ConnectionState::Disconnected;
    }

    /// Toggle connection: disconnect if connected, open port selector if not.
    pub fn toggle_connection(&mut self) {
        match &self.connection_state {
            ConnectionState::Connected(_) => {
                self.disconnect();
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

        if let Some(conn) = &mut self.connection {
            match conn.write(data.as_bytes()) {
                Ok(_) => {
                    self.tx_bytes = conn.tx_bytes;
                }
                Err(_) => {
                    self.connection_state =
                        ConnectionState::Error("Write failed".to_string());
                }
            }
        }

        self.input_text.clear();
        self.input_cursor = 0;
    }

    /// Poll for serial events. Called every tick from the event loop.
    pub fn poll_serial(&mut self) {
        let rx = match &self.serial_rx {
            Some(rx) => rx,
            None => return,
        };

        // Drain all available events
        loop {
            match rx.try_recv() {
                Ok(SerialEvent::Data(data)) => {
                    self.rx_bytes += data.len() as u64;
                    self.buffer.push_bytes(&data);
                    if self.follow_output {
                        self.scroll_offset = 0;
                    }
                }
                Ok(SerialEvent::Disconnected) => {
                    self.connection_state =
                        ConnectionState::Error("Port disconnected".to_string());
                    self.connection = None;
                    break;
                }
                Ok(SerialEvent::Error(e)) => {
                    self.connection_state = ConnectionState::Error(e);
                    self.connection = None;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.connection_state =
                        ConnectionState::Error("Reader thread died".to_string());
                    self.connection = None;
                    break;
                }
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

    /// Scroll up by the specified number of lines.
    pub fn scroll_up(&mut self, lines: usize) {
        let max_scroll = self.buffer.display_len().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + lines).min(max_scroll);
        self.follow_output = false;
    }

    /// Scroll down by the specified number of lines.
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
        if self.scroll_offset == 0 {
            self.follow_output = true;
        }
    }

    /// Jump to the bottom (latest output).
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
        self.follow_output = true;
    }

    /// Jump to the top (oldest output).
    pub fn scroll_to_top(&mut self) {
        let max_scroll = self.buffer.display_len().saturating_sub(1);
        self.scroll_offset = max_scroll;
        self.follow_output = false;
    }

    /// Insert a character at the cursor position in the input.
    pub fn input_char(&mut self, c: char) {
        self.input_text.insert(self.input_cursor, c);
        self.input_cursor += 1;
    }

    /// Delete the character before the cursor (backspace).
    pub fn input_backspace(&mut self) {
        if self.input_cursor > 0 {
            self.input_cursor -= 1;
            self.input_text.remove(self.input_cursor);
        }
    }

    /// Delete the character at the cursor (delete key).
    pub fn input_delete(&mut self) {
        if self.input_cursor < self.input_text.len() {
            self.input_text.remove(self.input_cursor);
        }
    }

    /// Move input cursor left.
    pub fn input_cursor_left(&mut self) {
        self.input_cursor = self.input_cursor.saturating_sub(1);
    }

    /// Move input cursor right.
    pub fn input_cursor_right(&mut self) {
        self.input_cursor = (self.input_cursor + 1).min(self.input_text.len());
    }

    /// Move input cursor to start.
    pub fn input_cursor_home(&mut self) {
        self.input_cursor = 0;
    }

    /// Move input cursor to end.
    pub fn input_cursor_end(&mut self) {
        self.input_cursor = self.input_text.len();
    }

    /// Get the current total RX bytes (including active connection).
    pub fn total_rx_bytes(&self) -> u64 {
        self.rx_bytes
    }

    /// Get the current total TX bytes (including active connection).
    pub fn total_tx_bytes(&self) -> u64 {
        self.tx_bytes
    }

    /// Check if currently connected.
    pub fn is_connected(&self) -> bool {
        matches!(self.connection_state, ConnectionState::Connected(_))
    }
}
