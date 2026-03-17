use std::io::Read;
use std::time::Duration;

/// Common baud rates to try, ordered by most likely.
const DETECT_RATES: &[u32] = &[115200, 9600, 57600, 38400, 19200, 230400, 460800, 921600, 4800, 2400, 1200];

/// Try to auto-detect the baud rate for a serial port.
///
/// Opens the port at each common rate, reads for ~300ms, and scores
/// the data by how much of it is printable ASCII. Returns the rate
/// with the highest score if it exceeds the threshold.
pub fn auto_detect_baud(port_name: &str) -> Option<u32> {
    let mut best_rate: Option<u32> = None;
    let mut best_score: f64 = 0.0;
    let threshold = 0.70; // At least 70% printable ASCII

    for &rate in DETECT_RATES {
        match serialport::new(port_name, rate)
            .timeout(Duration::from_millis(350))
            .open()
        {
            Ok(mut port) => {
                // Read for ~300ms
                let mut buf = [0u8; 4096];
                let mut all_bytes = Vec::new();

                let start = std::time::Instant::now();
                while start.elapsed() < Duration::from_millis(300) {
                    match port.read(&mut buf) {
                        Ok(n) if n > 0 => {
                            all_bytes.extend_from_slice(&buf[..n]);
                        }
                        _ => break,
                    }
                }

                if all_bytes.len() >= 4 {
                    let printable = all_bytes.iter()
                        .filter(|&&b| b.is_ascii_graphic() || b.is_ascii_whitespace())
                        .count();
                    let score = printable as f64 / all_bytes.len() as f64;

                    if score > best_score {
                        best_score = score;
                        best_rate = Some(rate);
                    }
                }
            }
            Err(_) => continue,
        }
    }

    if best_score >= threshold {
        best_rate
    } else {
        None
    }
}
