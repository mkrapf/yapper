#![allow(dead_code)]

mod app;
mod buffer;
mod config;
mod event;
mod filter;
mod hex;
mod highlight;
mod history;
mod input;
mod logging;
mod macros;
mod mouse;
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

use app::{App, Mode};
use event::EventLoop;

/// yapper — A modern UART serial TUI terminal for embedded workflows
#[derive(Parser, Debug)]
#[command(name = "yapper", version, about)]
struct Cli {
    /// Serial port to connect to (e.g. /dev/ttyUSB0)
    #[arg(value_name = "PORT")]
    port: Option<String>,

    /// Baud rate (defaults to saved config value, or 115200)
    #[arg(value_name = "BAUD")]
    baud: Option<u32>,

    /// Data bits (5, 6, 7, 8)
    #[arg(short, long)]
    data_bits: Option<u8>,

    /// Parity (none, odd, even)
    #[arg(short, long)]
    parity: Option<String>,

    /// Stop bits (1, 2)
    #[arg(short, long)]
    stop_bits: Option<u8>,

    /// Flow control (none, software, hardware)
    #[arg(short, long)]
    flow_control: Option<String>,

    /// Line ending to send (lf, crlf, cr)
    #[arg(long)]
    line_ending: Option<String>,

    /// Skip auto-connecting to the last used port
    #[arg(long)]
    no_auto: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let app_config = config::AppConfig::load();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::with_options(
        backend,
        ratatui::TerminalOptions {
            viewport: ratatui::Viewport::Fullscreen,
        },
    )?;

    // Create app — merge CLI overrides with saved config defaults
    let serial_config = serial::config::SerialConfig {
        baud_rate: cli.baud.unwrap_or(app_config.defaults.baud_rate),
        data_bits: parse_data_bits(cli.data_bits.unwrap_or(app_config.defaults.data_bits)),
        parity: parse_parity(cli.parity.as_deref().unwrap_or(&app_config.defaults.parity)),
        stop_bits: parse_stop_bits(cli.stop_bits.unwrap_or(app_config.defaults.stop_bits)),
        flow_control: parse_flow_control(
            cli.flow_control
                .as_deref()
                .unwrap_or(&app_config.defaults.flow_control),
        ),
    };

    let line_ending = match cli
        .line_ending
        .as_deref()
        .unwrap_or(&app_config.defaults.line_ending)
    {
        "lf" => "\n",
        "cr" => "\r",
        _ => "\r\n",
    };

    let mut app = App::new(serial_config, line_ending.to_string(), app_config.clone());

    // Connect: CLI port takes priority, then auto-connect to last port
    if let Some(port) = cli.port.as_deref() {
        app.connect(port);
    } else if !cli.no_auto && app_config.connection.auto_connect {
        if let Some(last_port) = &app_config.connection.last_port {
            app.connect(last_port);
        }
    }

    if cli.port.is_none() && !app.is_connected() {
        app.mode = Mode::Normal;
        app.open_port_selector();
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
