use std::collections::VecDeque;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};

use crate::serial::config::SerialConfig;
use crate::serial::connection::SerialEvent;
use crate::serial::detector::PortInfo;
use crate::transport::{Transport, TransportConnection};

const AT_MODEM_PORT: &str = "sim://at-modem";
const PERIODIC_STATUS_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimProfile {
    AtModem,
}

impl SimProfile {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "at-modem" => Some(Self::AtModem),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::AtModem => "at-modem",
        }
    }

    pub fn port_name(self) -> &'static str {
        match self {
            Self::AtModem => AT_MODEM_PORT,
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::AtModem => "Simulated AT modem",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SimTransport {
    profile: SimProfile,
}

impl SimTransport {
    pub fn new(profile: SimProfile) -> Self {
        Self { profile }
    }

    pub fn default_port(&self) -> &'static str {
        self.profile.port_name()
    }
}

impl Transport for SimTransport {
    fn available_ports(&self) -> Vec<PortInfo> {
        vec![PortInfo {
            name: self.profile.port_name().to_string(),
            description: self.profile.description().to_string(),
        }]
    }

    fn open(
        &self,
        port_name: &str,
        _config: &SerialConfig,
        tx: Sender<SerialEvent>,
    ) -> Result<Box<dyn TransportConnection>> {
        if port_name != self.profile.port_name() {
            return Err(anyhow!(
                "simulated profile {} does not expose port {}",
                self.profile.name(),
                port_name
            ));
        }
        Ok(Box::new(SimConnection::new(self.profile, tx)))
    }

    fn auto_detect_baud(&self, port_name: &str) -> Option<u32> {
        if port_name == self.profile.port_name() {
            Some(115200)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SimEvent {
    Data(Vec<u8>),
    Error(String),
    Disconnected,
}

#[derive(Debug, Clone)]
struct PendingEvent {
    remaining: Duration,
    event: SimEvent,
}

pub struct SimConnection {
    port_name: String,
    tx: Sender<SerialEvent>,
    device: AtModemDevice,
    pending: VecDeque<PendingEvent>,
    tx_bytes: u64,
    last_tick: Instant,
    periodic_remaining: Duration,
    closed: bool,
}

impl SimConnection {
    fn new(profile: SimProfile, tx: Sender<SerialEvent>) -> Self {
        let mut connection = Self {
            port_name: profile.port_name().to_string(),
            tx,
            device: AtModemDevice::new(),
            pending: VecDeque::new(),
            tx_bytes: 0,
            last_tick: Instant::now(),
            periodic_remaining: PERIODIC_STATUS_INTERVAL,
            closed: false,
        };
        connection.pending.extend(connection.device.boot_events());
        connection
    }

    fn enqueue(&mut self, events: impl IntoIterator<Item = PendingEvent>) {
        self.pending.extend(events);
    }

    fn emit(&self, event: SimEvent, now: Instant) -> bool {
        match event {
            SimEvent::Data(data) => self.tx.send(SerialEvent::Data(data, now)).is_ok(),
            SimEvent::Error(message) => self.tx.send(SerialEvent::Error(message)).is_ok(),
            SimEvent::Disconnected => self.tx.send(SerialEvent::Disconnected).is_ok(),
        }
    }
}

impl TransportConnection for SimConnection {
    fn write(&mut self, data: &[u8]) -> Result<usize> {
        if self.closed {
            return Err(anyhow!("simulated connection is closed"));
        }
        self.tx_bytes += data.len() as u64;
        let events = self.device.accept_bytes(data);
        self.enqueue(events);
        Ok(data.len())
    }

    fn tx_bytes(&self) -> u64 {
        self.tx_bytes
    }

    fn rx_bytes(&self) -> u64 {
        0
    }

    fn port_name(&self) -> &str {
        &self.port_name
    }

    fn tick(&mut self, now: Instant) -> bool {
        if self.closed {
            return false;
        }

        let delta = now.saturating_duration_since(self.last_tick);
        self.last_tick = now;

        if delta >= self.periodic_remaining {
            self.enqueue(self.device.periodic_status());
            self.periodic_remaining = PERIODIC_STATUS_INTERVAL;
        } else {
            self.periodic_remaining -= delta;
        }

        let mut changed = false;
        let mut waiting = VecDeque::with_capacity(self.pending.len());

        while let Some(mut pending) = self.pending.pop_front() {
            if delta >= pending.remaining {
                changed |= self.emit(pending.event, now);
            } else {
                pending.remaining -= delta;
                waiting.push_back(pending);
            }
        }

        self.pending = waiting;
        changed
    }

    fn close(mut self: Box<Self>) {
        self.closed = true;
    }
}

#[derive(Debug, Default)]
struct AtModemDevice {
    input: Vec<u8>,
    saw_cr: bool,
    boot_count: u32,
    wifi_mode: bool,
    wifi_connected: bool,
}

impl AtModemDevice {
    fn new() -> Self {
        Self::default()
    }

    fn boot_events(&mut self) -> Vec<PendingEvent> {
        self.boot_count += 1;
        vec![
            data_after(50, "BOOT: yapper simulated AT modem\r\n"),
            data_after(100, "READY\r\n"),
        ]
    }

    fn periodic_status(&self) -> Vec<PendingEvent> {
        let wifi = if self.wifi_connected {
            "CONNECTED"
        } else if self.wifi_mode {
            "IDLE"
        } else {
            "OFF"
        };
        vec![data_after(
            0,
            format!("+STATUS: WIFI={},BOOT={}\r\n", wifi, self.boot_count),
        )]
    }

    fn accept_bytes(&mut self, data: &[u8]) -> Vec<PendingEvent> {
        let mut events = Vec::new();

        for &byte in data {
            match byte {
                b'\r' => {
                    events.extend(self.commit_command());
                    self.saw_cr = true;
                }
                b'\n' if self.saw_cr => {
                    self.saw_cr = false;
                }
                b'\n' => {
                    events.extend(self.commit_command());
                }
                other => {
                    self.saw_cr = false;
                    self.input.push(other);
                }
            }
        }

        events
    }

    fn commit_command(&mut self) -> Vec<PendingEvent> {
        let raw = std::mem::take(&mut self.input);
        let command = String::from_utf8_lossy(&raw).trim().to_string();
        if command.is_empty() {
            return Vec::new();
        }
        self.handle_command(&command)
    }

    fn handle_command(&mut self, command: &str) -> Vec<PendingEvent> {
        let upper = command.to_ascii_uppercase();

        match upper.as_str() {
            "AT" => vec![data_after(20, "OK\r\n")],
            "AT+GMR" => vec![
                data_after(20, "yapper-sim 0.1.0\r\n"),
                data_after(25, "OK\r\n"),
            ],
            "AT+RST" => {
                self.wifi_connected = false;
                let mut events = vec![data_after(20, "OK\r\n")];
                events.extend(self.boot_events().into_iter().map(|mut event| {
                    event.remaining += Duration::from_millis(50);
                    event
                }));
                events
            }
            "AT+CWMODE=1" => {
                self.wifi_mode = true;
                vec![
                    data_after(30, "WIFI MODE: STA\r\n"),
                    data_after(35, "OK\r\n"),
                ]
            }
            "AT+CWQAP" => {
                self.wifi_connected = false;
                vec![
                    data_after(30, "WIFI DISCONNECT\r\n"),
                    data_after(35, "OK\r\n"),
                ]
            }
            "AT+SIMDISCONNECT" => vec![event_after(20, SimEvent::Disconnected)],
            "AT+SIMERROR" => vec![event_after(
                20,
                SimEvent::Error("simulated device error".to_string()),
            )],
            _ if upper.starts_with("AT+CWJAP=") => {
                self.wifi_mode = true;
                self.wifi_connected = true;
                vec![
                    data_after(50, "WIFI CONNECTED\r\n"),
                    data_after(75, "WIFI GOT IP\r\n"),
                    data_after(80, "OK\r\n"),
                ]
            }
            _ => vec![data_after(20, "ERROR\r\n")],
        }
    }
}

fn data_after(delay_ms: u64, data: impl Into<Vec<u8>>) -> PendingEvent {
    event_after(delay_ms, SimEvent::Data(data.into()))
}

fn event_after(delay_ms: u64, event: SimEvent) -> PendingEvent {
    PendingEvent {
        remaining: Duration::from_millis(delay_ms),
        event,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn drain_data(rx: &mpsc::Receiver<SerialEvent>) -> Vec<String> {
        let mut lines = Vec::new();
        while let Ok(event) = rx.try_recv() {
            if let SerialEvent::Data(data, _) = event {
                lines.push(String::from_utf8_lossy(&data).to_string());
            }
        }
        lines
    }

    #[test]
    fn at_modem_replies_to_basic_commands() {
        let (tx, rx) = mpsc::channel();
        let mut conn = SimConnection::new(SimProfile::AtModem, tx);
        let now = Instant::now();

        conn.write(b"AT\r\n").unwrap();
        assert!(conn.tick(now + Duration::from_millis(25)));

        let output = drain_data(&rx).join("");
        assert!(output.contains("OK"));
    }

    #[test]
    fn at_modem_handles_partial_input_and_unknown_commands() {
        let (tx, rx) = mpsc::channel();
        let mut conn = SimConnection::new(SimProfile::AtModem, tx);
        let now = Instant::now();

        conn.write(b"AT+G").unwrap();
        assert!(!conn.tick(now + Duration::from_millis(10)));
        conn.write(b"MR\n").unwrap();
        assert!(conn.tick(now + Duration::from_millis(40)));
        let output = drain_data(&rx).join("");
        assert!(output.contains("yapper-sim 0.1.0"));

        conn.write(b"AT+NOPE\n").unwrap();
        assert!(conn.tick(now + Duration::from_millis(70)));
        let output = drain_data(&rx).join("");
        assert!(output.contains("ERROR"));
    }

    #[test]
    fn at_modem_can_emit_disconnect_and_periodic_status() {
        let (tx, rx) = mpsc::channel();
        let mut conn = SimConnection::new(SimProfile::AtModem, tx);
        let now = Instant::now();

        assert!(conn.tick(now + PERIODIC_STATUS_INTERVAL + Duration::from_millis(1)));
        assert!(drain_data(&rx).join("").contains("+STATUS"));

        conn.write(b"AT+SIMDISCONNECT\n").unwrap();
        assert!(conn.tick(now + PERIODIC_STATUS_INTERVAL + Duration::from_millis(30)));
        assert!(matches!(rx.try_recv(), Ok(SerialEvent::Disconnected)));
    }
}
