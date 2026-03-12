#![allow(dead_code)]

mod app;
mod buffer;
mod config;
mod event;
mod hex;
mod history;
mod input;
mod logging;
mod search;
mod serial;
mod theme;
mod ui;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;

use app::App;
use event::EventLoop;

/// yap — A modern UART serial TUI terminal for embedded workflows
#[derive(Parser, Debug)]
#[command(name = "yap", version, about)]
struct Cli {
    /// Serial port to connect to (e.g. /dev/ttyUSB0)
    #[arg(value_name = "PORT")]
    port: Option<String>,

    /// Baud rate
    #[arg(value_name = "BAUD", default_value = "115200")]
    baud: u32,

    /// Data bits (5, 6, 7, 8)
    #[arg(short, long, default_value = "8")]
    data_bits: u8,

    /// Parity (none, odd, even)
    #[arg(short, long, default_value = "none")]
    parity: String,

    /// Stop bits (1, 2)
    #[arg(short, long, default_value = "1")]
    stop_bits: u8,

    /// Flow control (none, software, hardware)
    #[arg(short, long, default_value = "none")]
    flow_control: String,

    /// Line ending to send (lf, crlf, cr)
    #[arg(long, default_value = "crlf")]
    line_ending: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let serial_config = serial::config::SerialConfig {
        baud_rate: cli.baud,
        data_bits: parse_data_bits(cli.data_bits),
        parity: parse_parity(&cli.parity),
        stop_bits: parse_stop_bits(cli.stop_bits),
        flow_control: parse_flow_control(&cli.flow_control),
    };

    let line_ending = match cli.line_ending.as_str() {
        "lf" => "\n",
        "cr" => "\r",
        _ => "\r\n",
    };

    let mut app = App::new(serial_config, line_ending.to_string());

    // If a port was specified, connect immediately
    if let Some(port) = cli.port {
        app.connect(&port);
    }

    // Run event loop
    let mut event_loop = EventLoop::new();
    let result = event_loop.run(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn parse_data_bits(bits: u8) -> serialport::DataBits {
    match bits {
        5 => serialport::DataBits::Five,
        6 => serialport::DataBits::Six,
        7 => serialport::DataBits::Seven,
        _ => serialport::DataBits::Eight,
    }
}

fn parse_parity(parity: &str) -> serialport::Parity {
    match parity.to_lowercase().as_str() {
        "odd" => serialport::Parity::Odd,
        "even" => serialport::Parity::Even,
        _ => serialport::Parity::None,
    }
}

fn parse_stop_bits(bits: u8) -> serialport::StopBits {
    match bits {
        2 => serialport::StopBits::Two,
        _ => serialport::StopBits::One,
    }
}

fn parse_flow_control(fc: &str) -> serialport::FlowControl {
    match fc.to_lowercase().as_str() {
        "software" | "sw" | "xon" => serialport::FlowControl::Software,
        "hardware" | "hw" | "rts" => serialport::FlowControl::Hardware,
        _ => serialport::FlowControl::None,
    }
}
