use std::sync::mpsc::Sender;
use std::time::Instant;

use anyhow::Result;

use crate::serial::config::SerialConfig;
use crate::serial::connection::{SerialConnection, SerialEvent};
use crate::serial::detector::{self, PortInfo};

/// An active connection, backed by either real serial I/O or a simulator.
pub trait TransportConnection: Send {
    fn write(&mut self, data: &[u8]) -> Result<usize>;
    fn tx_bytes(&self) -> u64;
    fn rx_bytes(&self) -> u64;
    fn port_name(&self) -> &str;
    fn tick(&mut self, _now: Instant) -> bool {
        false
    }
    fn close(self: Box<Self>);
}

/// Runtime boundary for serial-like I/O.
pub trait Transport: Send + Sync {
    fn available_ports(&self) -> Vec<PortInfo>;
    fn open(
        &self,
        port_name: &str,
        config: &SerialConfig,
        tx: Sender<SerialEvent>,
    ) -> Result<Box<dyn TransportConnection>>;
    fn auto_detect_baud(&self, port_name: &str) -> Option<u32>;
}

#[derive(Debug, Default)]
pub struct RealTransport;

impl RealTransport {
    pub fn new() -> Self {
        Self
    }
}

impl Transport for RealTransport {
    fn available_ports(&self) -> Vec<PortInfo> {
        detector::available_ports()
    }

    fn open(
        &self,
        port_name: &str,
        config: &SerialConfig,
        tx: Sender<SerialEvent>,
    ) -> Result<Box<dyn TransportConnection>> {
        Ok(Box::new(SerialConnection::open(port_name, config, tx)?))
    }

    fn auto_detect_baud(&self, port_name: &str) -> Option<u32> {
        crate::serial::auto_detect::auto_detect_baud(port_name)
    }
}

impl TransportConnection for SerialConnection {
    fn write(&mut self, data: &[u8]) -> Result<usize> {
        SerialConnection::write(self, data)
    }

    fn tx_bytes(&self) -> u64 {
        SerialConnection::tx_bytes(self)
    }

    fn rx_bytes(&self) -> u64 {
        self.rx_bytes
    }

    fn port_name(&self) -> &str {
        SerialConnection::port_name(self)
    }

    fn close(self: Box<Self>) {
        SerialConnection::close(*self);
    }
}
