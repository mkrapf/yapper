/// Information about an available serial port.
#[derive(Debug, Clone)]
pub struct PortInfo {
    pub name: String,
    pub description: String,
}

/// Detect all available serial ports on the system.
///
/// On Linux, this filters out legacy PCI/ISA serial ports (/dev/ttySNN) and
/// Bluetooth ports, which are almost never real USB-serial hardware. Only USB
/// ports and unknown-type ports (which may be USB adapters without proper type
/// info) are shown.
pub fn available_ports() -> Vec<PortInfo> {
    match serialport::available_ports() {
        Ok(ports) => ports
            .into_iter()
            .filter(|p| {
                match &p.port_type {
                    serialport::SerialPortType::UsbPort(_) => true,
                    serialport::SerialPortType::PciPort => {
                        // Allow known embedded-relevant PCI-type ports
                        // (e.g. Raspberry Pi GPIO UART: /dev/ttyAMA0)
                        is_embedded_port(&p.port_name)
                    }
                    serialport::SerialPortType::Unknown => {
                        // Allow non-legacy unknown ports (some USB adapters
                        // and RPi symlinks like /dev/serial0 report as Unknown)
                        !is_legacy_serial(&p.port_name)
                    }
                    // Bluetooth ports are not relevant for embedded work
                    serialport::SerialPortType::BluetoothPort => false,
                }
            })
            .map(|p| {
                let description = match &p.port_type {
                    serialport::SerialPortType::UsbPort(info) => {
                        let mut desc = String::new();
                        if let Some(manufacturer) = &info.manufacturer {
                            desc.push_str(manufacturer);
                        }
                        if let Some(product) = &info.product {
                            if !desc.is_empty() {
                                desc.push_str(" - ");
                            }
                            desc.push_str(product);
                        }
                        if desc.is_empty() {
                            format!("USB ({:04x}:{:04x})", info.vid, info.pid)
                        } else {
                            desc
                        }
                    }
                    serialport::SerialPortType::PciPort => "PCI".to_string(),
                    serialport::SerialPortType::BluetoothPort => "Bluetooth".to_string(),
                    serialport::SerialPortType::Unknown => "Unknown".to_string(),
                };
                PortInfo {
                    name: p.port_name,
                    description,
                }
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Check if a port name looks like a legacy 16550-style serial port.
/// These are /dev/ttySNN on Linux — almost never real hardware.
fn is_legacy_serial(port_name: &str) -> bool {
    if let Some(suffix) = port_name.strip_prefix("/dev/ttyS") {
        return suffix.chars().all(|c| c.is_ascii_digit());
    }
    false
}

/// Check if a port name is a known embedded-relevant non-USB serial port.
/// Examples: Raspberry Pi GPIO UART (/dev/ttyAMA0), RPi symlinks (/dev/serial0).
fn is_embedded_port(port_name: &str) -> bool {
    // Raspberry Pi PL011 UART (GPIO pins 14/15)
    if port_name.starts_with("/dev/ttyAMA") {
        return true;
    }
    // Raspberry Pi serial symlinks
    if port_name.starts_with("/dev/serial") {
        return true;
    }
    false
}
