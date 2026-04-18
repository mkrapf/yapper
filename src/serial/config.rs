use serialport::{DataBits, FlowControl, Parity, StopBits};

/// Serial port configuration parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialConfig {
    pub baud_rate: u32,
    pub data_bits: DataBits,
    pub parity: Parity,
    pub stop_bits: StopBits,
    pub flow_control: FlowControl,
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            baud_rate: 115200,
            data_bits: DataBits::Eight,
            parity: Parity::None,
            stop_bits: StopBits::One,
            flow_control: FlowControl::None,
        }
    }
}

impl SerialConfig {
    /// Format as a short summary string like "115200 8N1"
    pub fn summary(&self) -> String {
        let data = match self.data_bits {
            DataBits::Five => "5",
            DataBits::Six => "6",
            DataBits::Seven => "7",
            DataBits::Eight => "8",
        };
        let parity = match self.parity {
            Parity::None => "N",
            Parity::Odd => "O",
            Parity::Even => "E",
        };
        let stop = match self.stop_bits {
            StopBits::One => "1",
            StopBits::Two => "2",
        };
        format!("{} {}{}{}", self.baud_rate, data, parity, stop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_summary() {
        let config = SerialConfig::default();
        assert_eq!(config.summary(), "115200 8N1");
    }

    #[test]
    fn test_custom_summary() {
        let config = SerialConfig {
            baud_rate: 9600,
            data_bits: DataBits::Seven,
            parity: Parity::Even,
            stop_bits: StopBits::Two,
            flow_control: FlowControl::None,
        };
        assert_eq!(config.summary(), "9600 7E2");
    }
}
