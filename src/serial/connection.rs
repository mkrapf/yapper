use anyhow::{Context, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use super::config::SerialConfig;

/// Messages sent from the serial reader thread to the main thread.
pub enum SerialEvent {
    /// Raw bytes received from the serial port, with the instant they were read.
    Data(Vec<u8>, Instant),
    /// The serial port encountered an error.
    Error(String),
    /// The serial port was disconnected.
    Disconnected,
}

/// Manages the serial port connection and reader thread.
pub struct SerialConnection {
    /// Channel to send write requests to the writer thread (non-blocking).
    write_tx: Option<Sender<Vec<u8>>>,
    /// Flag to signal threads to stop.
    stop_flag: Arc<AtomicBool>,
    /// Handle to the reader thread.
    reader_handle: Option<thread::JoinHandle<()>>,
    /// Handle to the writer thread.
    writer_handle: Option<thread::JoinHandle<()>>,
    /// The port name (e.g. "/dev/ttyUSB0").
    port_name: String,
    /// Total bytes sent (updated via shared counter).
    tx_count: Arc<std::sync::atomic::AtomicU64>,
    /// Total bytes received.
    pub rx_bytes: u64,
}

impl SerialConnection {
    /// Open a serial port and start the reader + writer threads.
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
        let tx_count = Arc::new(std::sync::atomic::AtomicU64::new(0));

        // Clone port for reader thread
        let reader_port = port.try_clone()
            .context("Failed to clone serial port for reader thread")?;
        let reader_stop = stop_flag.clone();

        let reader_handle = thread::Builder::new()
            .name(format!("serial-reader-{}", port_name))
            .spawn(move || {
                Self::reader_loop(reader_port, tx, reader_stop);
            })
            .context("Failed to spawn serial reader thread")?;

        // Writer thread: receives data via channel and writes to port
        let (write_tx, write_rx) = mpsc::channel::<Vec<u8>>();
        let writer_stop = stop_flag.clone();
        let writer_tx_count = tx_count.clone();
        // port is moved into the writer thread — it owns the write handle
        let writer_handle = thread::Builder::new()
            .name(format!("serial-writer-{}", port_name))
            .spawn(move || {
                Self::writer_loop(port, write_rx, writer_stop, writer_tx_count);
            })
            .context("Failed to spawn serial writer thread")?;

        Ok(Self {
            write_tx: Some(write_tx),
            stop_flag,
            reader_handle: Some(reader_handle),
            writer_handle: Some(writer_handle),
            port_name: port_name.to_string(),
            tx_count,
            rx_bytes: 0,
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
                    if tx.send(SerialEvent::Data(buf[..n].to_vec(), Instant::now())).is_err() {
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

    /// The writer thread loop: receives data from the channel and writes to the port.
    fn writer_loop(
        mut port: Box<dyn serialport::SerialPort>,
        rx: mpsc::Receiver<Vec<u8>>,
        stop: Arc<AtomicBool>,
        tx_count: Arc<std::sync::atomic::AtomicU64>,
    ) {
        use std::io::Write;

        while !stop.load(Ordering::Relaxed) {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(data) => {
                    match port.write_all(&data) {
                        Ok(_) => {
                            let _ = port.flush();
                            tx_count.fetch_add(data.len() as u64, Ordering::Relaxed);
                        }
                        Err(_) => break,
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    /// Send data to the serial port (non-blocking — queued to writer thread).
    pub fn write(&self, data: &[u8]) -> Result<usize> {
        let len = data.len();
        if let Some(tx) = &self.write_tx {
            tx.send(data.to_vec())
                .map_err(|_| anyhow::anyhow!("Writer thread disconnected"))?;
        }
        Ok(len)
    }

    /// Get total TX bytes.
    pub fn tx_bytes(&self) -> u64 {
        self.tx_count.load(Ordering::Relaxed)
    }

    /// Get the port name.
    pub fn port_name(&self) -> &str {
        &self.port_name
    }

    /// Close the connection and stop all threads.
    pub fn close(mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        self.write_tx.take(); // Drop sender to signal writer thread
        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.writer_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for SerialConnection {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }
}
