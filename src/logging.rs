use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use chrono::Local;

/// Session logger that writes serial output to a file.
pub struct SessionLogger {
    file: Option<File>,
    file_path: Option<PathBuf>,
    format: LogFormat,
    pub is_active: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LogFormat {
    /// Raw bytes as received.
    Raw,
    /// Lines prefixed with timestamps.
    Timestamped,
}

impl SessionLogger {
    pub fn new() -> Self {
        Self {
            file: None,
            file_path: None,
            format: LogFormat::Timestamped,
            is_active: false,
        }
    }

    /// Start logging to a file. Creates a new timestamped log file.
    pub fn start(&mut self) -> Result<PathBuf, String> {
        let log_dir = dirs::data_dir()
            .map(|d| d.join("yapper").join("logs"))
            .ok_or_else(|| "Could not determine data directory".to_string())?;

        fs::create_dir_all(&log_dir)
            .map_err(|e| format!("Failed to create log directory: {}", e))?;

        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let filename = format!("yapper_{}.log", timestamp);
        let path = log_dir.join(&filename);

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(&path)
            .map_err(|e| format!("Failed to create log file: {}", e))?;

        self.file = Some(file);
        self.file_path = Some(path.clone());
        self.is_active = true;

        // Write header
        self.write_line(&format!(
            "--- yapper session started at {} ---",
            Local::now().format("%Y-%m-%d %H:%M:%S")
        ));

        Ok(path)
    }

    /// Stop logging.
    pub fn stop(&mut self) {
        if self.is_active {
            self.write_line(&format!(
                "--- yapper session ended at {} ---",
                Local::now().format("%Y-%m-%d %H:%M:%S")
            ));
        }
        self.file = None;
        self.is_active = false;
    }

    /// Toggle logging on/off.
    pub fn toggle(&mut self) -> Result<Option<PathBuf>, String> {
        if self.is_active {
            self.stop();
            Ok(None)
        } else {
            self.start().map(Some)
        }
    }

    /// Log raw bytes.
    pub fn log_bytes(&mut self, data: &[u8]) {
        if !self.is_active {
            return;
        }

        if let Some(file) = &mut self.file {
            match self.format {
                LogFormat::Raw => {
                    let _ = file.write_all(data);
                }
                LogFormat::Timestamped => {
                    // For timestamped mode, we write as UTF-8 text
                    if let Ok(text) = std::str::from_utf8(data) {
                        let _ = write!(file, "{}", text);
                    } else {
                        let _ = file.write_all(data);
                    }
                }
            }
            let _ = file.flush();
        }
    }

    /// Write a metadata line to the log.
    fn write_line(&mut self, line: &str) {
        if let Some(file) = &mut self.file {
            let _ = writeln!(file, "{}", line);
            let _ = file.flush();
        }
    }

    /// Get the current log file path.
    pub fn file_path(&self) -> Option<&PathBuf> {
        self.file_path.as_ref()
    }
}
