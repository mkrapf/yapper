use super::{App, ConnectionState};
use crate::serial::config::SerialConfig;

impl App {
    pub fn open_settings(&mut self) {
        self.settings_field = 0;
        self.settings_original_config = Some(self.serial_config.clone());
        self.settings_original_line_ending = Some(self.line_ending.clone());
        self.open_overlay(super::Mode::Settings);
    }

    pub fn settings_next_value(&mut self) {
        use serialport::*;
        match self.settings_field {
            0 => {
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

        self.sync_config_to_disk();

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

    pub(crate) fn settings_require_reconnect(
        original_config: &SerialConfig,
        current_config: &SerialConfig,
        _original_line_ending: &str,
        _current_line_ending: &str,
    ) -> bool {
        original_config != current_config
    }

    fn sync_config_to_disk(&mut self) {
        match &self.connection_state {
            ConnectionState::Connected(port) | ConnectionState::Reconnecting(port) => {
                let port = port.clone();
                self.save_port_profile(&port);
            }
            _ => self.save_global_defaults(),
        }
    }
}
