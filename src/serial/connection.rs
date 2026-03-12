use anyhow::{Context, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::config::SerialConfig;

/// Messages sent from the serial reader thread to the main thread.
pub enum SerialEvent {
    /// Raw bytes received from the serial port.
    Data(Vec<u8>),
    /// The serial port encountered an error.
    Error(String),
    /// The serial port was disconnected.
    Disconnected,
}

/// Manages the serial port connection and reader thread.
pub struct SerialConnection {
    /// The serial port handle (used for writing TX data).
    port: Box<dyn serialport::SerialPort>,
    /// Flag to signal the reader thread to stop.
    stop_flag: Arc<AtomicBool>,
    /// Handle to the reader thread.
    reader_handle: Option<thread::JoinHandle<()>>,
    /// The port name (e.g. "/dev/ttyUSB0").
    port_name: String,
    /// Total bytes received.
    pub rx_bytes: u64,
    /// Total bytes sent.
    pub tx_bytes: u64,
}

impl SerialConnection {
    /// Open a serial port and start the reader thread.
    pub fn open(
        port_name: &str,
        config: &SerialConfig,
        tx: Sender<SerialEvent>,
    ) -> Result<Self> {
        let port = serialport::new(port_name, config.baud_rate)
            .data_bits(config.data_bits)
            .parity(config.parity)
            .stop_bits(config.stop_bits)
            .flow_control(config.flow_control)
            .timeout(Duration::from_millis(100))
            .open()
            .with_context(|| format!("Failed to open serial port: {}", port_name))?;

        let stop_flag = Arc::new(AtomicBool::new(false));

        // Clone what the reader thread needs
        let reader_port = port.try_clone()
            .context("Failed to clone serial port for reader thread")?;
        let reader_stop = stop_flag.clone();

        let reader_handle = thread::Builder::new()
            .name(format!("serial-reader-{}", port_name))
            .spawn(move || {
                Self::reader_loop(reader_port, tx, reader_stop);
            })
            .context("Failed to spawn serial reader thread")?;

        Ok(Self {
            port,
            stop_flag,
            reader_handle: Some(reader_handle),
            port_name: port_name.to_string(),
            rx_bytes: 0,
            tx_bytes: 0,
        })
    }

    /// The reader thread loop: reads bytes and sends them to the main thread.
    fn reader_loop(
        mut port: Box<dyn serialport::SerialPort>,
        tx: Sender<SerialEvent>,
        stop: Arc<AtomicBool>,
    ) {
        let mut buf = [0u8; 1024];

        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }

            match port.read(&mut buf) {
                Ok(n) if n > 0 => {
                    if tx.send(SerialEvent::Data(buf[..n].to_vec())).is_err() {
                        break; // Main thread dropped the receiver
                    }
                }
                Ok(_) => {} // Zero bytes, timeout — just loop
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    // Normal timeout, continue
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::BrokenPipe
                    || e.kind() == std::io::ErrorKind::PermissionDenied =>
                {
                    let _ = tx.send(SerialEvent::Disconnected);
                    break;
                }
                Err(e) => {
                    let _ = tx.send(SerialEvent::Error(e.to_string()));
                    break;
                }
            }
        }
    }

    /// Write data to the serial port (TX).
    pub fn write(&mut self, data: &[u8]) -> Result<usize> {
        use std::io::Write;
        let n = self.port.write(data)
            .context("Failed to write to serial port")?;
        self.port.flush().ok();
        self.tx_bytes += n as u64;
        Ok(n)
    }

    /// Get the port name.
    pub fn port_name(&self) -> &str {
        &self.port_name
    }

    /// Close the connection and stop the reader thread.
    pub fn close(mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for SerialConnection {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        // We can't join here because we'd need to take the handle,
        // but the thread will stop on its own when it sees the flag.
    }
}
