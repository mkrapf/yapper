use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::{App, ConnectionState, Mode, MAX_RECONNECT_DELAY};
use crate::serial::connection::{SerialConnection, SerialEvent};
use crate::serial::detector;

impl App {
    pub(super) fn reset_reconnect_state(&mut self) {
        self.reconnect_port = None;
        self.reconnect_current_delay = self.reconnect_delay.min(MAX_RECONNECT_DELAY);
        self.reconnect_next_attempt = None;
        self.reconnect_attempts = 0;
    }

    pub(super) fn schedule_reconnect(&mut self, port_name: String, now: Instant) {
        self.reconnect_port = Some(port_name.clone());
        self.reconnect_current_delay = self.reconnect_delay.min(MAX_RECONNECT_DELAY);
        self.reconnect_next_attempt = Some(now + self.reconnect_current_delay);
        self.reconnect_attempts = 0;
        self.connection_state = ConnectionState::Reconnecting(port_name);
    }

    fn handle_connection_loss(&mut self, port_name: String, error_message: String, now: Instant) {
        self.disconnect_internal(true);
        if self.auto_reconnect && !port_name.is_empty() {
            self.schedule_reconnect(port_name, now);
        } else {
            self.connection_state = ConnectionState::Error(error_message);
        }
    }

    fn current_connection_port(&self) -> String {
        match &self.connection_state {
            ConnectionState::Connected(port) | ConnectionState::Reconnecting(port) => port.clone(),
            _ => String::new(),
        }
    }

    pub(super) fn handle_write_error(&mut self, error_message: String) {
        let port = self.current_connection_port();
        self.handle_connection_loss(
            port,
            format!("Write failed: {}", error_message),
            Instant::now(),
        );
    }

    pub fn reconnect_status(&self, now: Instant) -> Option<(usize, Duration)> {
        if !matches!(self.connection_state, ConnectionState::Reconnecting(_)) {
            return None;
        }

        let next_attempt = self.reconnect_next_attempt?;
        let remaining = next_attempt.saturating_duration_since(now);
        Some((self.reconnect_attempts + 1, remaining))
    }

    pub fn connect(&mut self, port_name: &str) {
        self.disconnect_internal(false);
        self.reset_reconnect_state();

        let (tx, rx) = mpsc::channel();

        match SerialConnection::open(port_name, &self.serial_config, tx.clone()) {
            Ok(conn) => {
                self.connection_state = ConnectionState::Connected(port_name.to_string());
                self.connection = Some(conn);
                self.serial_rx = Some(rx);
                self.serial_tx = Some(tx);
                self.set_status(format!("Connected to {}", port_name));
                self.app_config.connection.last_port = Some(port_name.to_string());
                self.save_app_config();
                self.ensure_auto_logging();
            }
            Err(e) => {
                self.connection_state = ConnectionState::Error(e.to_string());
            }
        }
    }

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
            self.reset_reconnect_state();
        }
    }

    pub fn disconnect(&mut self) {
        self.disconnect_internal(false);
        self.set_status("Disconnected".to_string());
    }

    pub fn toggle_connection(&mut self) {
        match &self.connection_state {
            ConnectionState::Connected(_) => {
                self.disconnect();
            }
            ConnectionState::Reconnecting(_) => {
                self.reset_reconnect_state();
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

    pub fn poll_serial(&mut self) -> bool {
        let mut changed = false;

        let rx = match &self.serial_rx {
            Some(rx) => rx,
            None => return changed,
        };

        loop {
            match rx.try_recv() {
                Ok(SerialEvent::Data(data, received_at)) => {
                    self.rx_bytes += data.len() as u64;
                    self.logger.log_bytes(&data);
                    self.buffer.push_bytes(&data);
                    if self.follow_output {
                        self.scroll_offset = 0;
                    }
                    if let Some(sent_at) = self.last_command_sent.take() {
                        self.last_response_time = Some(received_at.duration_since(sent_at));
                    }
                    changed = true;
                }
                Ok(SerialEvent::Disconnected) => {
                    let port = self.current_connection_port();
                    self.handle_connection_loss(
                        port,
                        "Port disconnected".to_string(),
                        Instant::now(),
                    );
                    changed = true;
                    break;
                }
                Ok(SerialEvent::Error(e)) => {
                    let port = self.current_connection_port();
                    self.handle_connection_loss(port, e, Instant::now());
                    changed = true;
                    break;
                }
                Ok(SerialEvent::WriteError(e)) => {
                    let port = self.current_connection_port();
                    self.handle_connection_loss(port, e, Instant::now());
                    changed = true;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    let port = self.current_connection_port();
                    self.handle_connection_loss(
                        port,
                        "Reader thread died".to_string(),
                        Instant::now(),
                    );
                    changed = true;
                    break;
                }
            }
        }

        changed
    }

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

    fn try_reconnect(&mut self, now: Instant) -> bool {
        let port = match &self.reconnect_port {
            Some(p) => p.clone(),
            None => return false,
        };

        if !matches!(self.connection_state, ConnectionState::Reconnecting(_)) {
            return false;
        }

        let next_attempt = match self.reconnect_next_attempt {
            Some(next_attempt) => next_attempt,
            None => return false,
        };
        if now < next_attempt {
            return false;
        }

        let (tx, rx) = mpsc::channel();
        match SerialConnection::open(&port, &self.serial_config, tx.clone()) {
            Ok(conn) => {
                self.reset_reconnect_state();
                self.connection_state = ConnectionState::Connected(port.clone());
                self.connection = Some(conn);
                self.serial_rx = Some(rx);
                self.serial_tx = Some(tx);
                self.set_status(format!("Reconnected to {}", port));
                self.app_config.connection.last_port = Some(port.clone());
                self.save_app_config();
                self.ensure_auto_logging();
                true
            }
            Err(_) => {
                self.reconnect_attempts += 1;
                self.reconnect_current_delay =
                    (self.reconnect_current_delay.saturating_mul(2)).min(MAX_RECONNECT_DELAY);
                self.reconnect_next_attempt = Some(now + self.reconnect_current_delay);
                false
            }
        }
    }

    pub fn auto_detect_baud(&mut self, port_name: &str) {
        self.set_status("Auto-detecting baud rate...".to_string());
        match crate::serial::auto_detect::auto_detect_baud(port_name) {
            Some(rate) => {
                self.apply_detected_baud(port_name, rate);
                self.set_status(format!("Detected baud rate: {}", rate));
            }
            None => {
                self.set_status("Could not detect baud rate — no readable data".to_string());
            }
        }
    }

    pub(super) fn apply_detected_baud(&mut self, port_name: &str, rate: u32) {
        self.serial_config.baud_rate = rate;
        self.save_port_profile(port_name);
    }

    pub fn open_port_selector(&mut self) {
        self.available_ports = detector::available_ports();
        self.port_select_index = 0;
        self.open_overlay(Mode::PortSelect);
    }

    pub fn connect_selected_port(&mut self) {
        if let Some(port) = self.available_ports.get(self.port_select_index) {
            let port_name = port.name.clone();
            self.load_port_profile(&port_name);
            self.restore_mode();
            self.connect(&port_name);
        }
    }
}
