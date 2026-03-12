/// Information about an available serial port.
#[derive(Debug, Clone)]
pub struct PortInfo {
    pub name: String,
    pub description: String,
}

/// Detect all available serial ports on the system.
pub fn available_ports() -> Vec<PortInfo> {
    match serialport::available_ports() {
        Ok(ports) => ports
            .into_iter()
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
