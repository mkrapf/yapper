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
    let startup_port = resolve_startup_port(&cli, &app_config);

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

    // Create app — merge CLI overrides with per-port profile or global defaults
    let effective_defaults = resolve_effective_defaults(&cli, &app_config, startup_port.as_deref());

    let serial_config = effective_defaults.to_serial_config();
    let line_ending = effective_defaults.to_line_ending();

    let mut app = App::new(serial_config, line_ending.to_string(), app_config.clone());

    // Connect: CLI port takes priority, then auto-connect to last port
    if let Some(port) = startup_port.as_deref() {
        app.connect(port);
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

fn resolve_startup_port(cli: &Cli, app_config: &config::AppConfig) -> Option<String> {
    cli.port.clone().or_else(|| {
        if !cli.no_auto && app_config.connection.auto_connect {
            app_config.connection.last_port.clone()
        } else {
            None
        }
    })
}

fn resolve_effective_defaults(
    cli: &Cli,
    app_config: &config::AppConfig,
    startup_port: Option<&str>,
) -> config::DefaultsConfig {
    let mut effective_defaults = startup_port
        .and_then(|port| app_config.connection.port_profiles.get(port))
        .cloned()
        .unwrap_or_else(|| app_config.defaults.clone());

    if let Some(baud_rate) = cli.baud {
        effective_defaults.baud_rate = baud_rate;
    }
    if let Some(data_bits) = cli.data_bits {
        effective_defaults.data_bits = data_bits;
    }
    if let Some(parity) = cli.parity.as_deref() {
        effective_defaults.parity = parity.to_string();
    }
    if let Some(stop_bits) = cli.stop_bits {
        effective_defaults.stop_bits = stop_bits;
    }
    if let Some(flow_control) = cli.flow_control.as_deref() {
        effective_defaults.flow_control = flow_control.to_string();
    }
    if let Some(line_ending) = cli.line_ending.as_deref() {
        effective_defaults.line_ending = line_ending.to_string();
    }

    effective_defaults
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cli() -> Cli {
        Cli {
            port: None,
            baud: None,
            data_bits: None,
            parity: None,
            stop_bits: None,
            flow_control: None,
            line_ending: None,
            no_auto: false,
        }
    }

    #[test]
    fn test_resolve_effective_defaults_prefers_port_profile_for_auto_connect() {
        let mut config = config::AppConfig::default();
        config.connection.last_port = Some("/dev/ttyUSB0".to_string());
        config.connection.port_profiles.insert(
            "/dev/ttyUSB0".to_string(),
            config::DefaultsConfig {
                baud_rate: 9600,
                data_bits: 7,
                parity: "even".to_string(),
                stop_bits: 2,
                flow_control: "hardware".to_string(),
                line_ending: "lf".to_string(),
            },
        );

        let cli = cli();
        let startup_port = resolve_startup_port(&cli, &config);
        let resolved = resolve_effective_defaults(&cli, &config, startup_port.as_deref());

        assert_eq!(startup_port.as_deref(), Some("/dev/ttyUSB0"));
        assert_eq!(resolved.baud_rate, 9600);
        assert_eq!(resolved.line_ending, "lf");
    }

    #[test]
    fn test_resolve_effective_defaults_applies_cli_overrides_over_port_profile() {
        let mut config = config::AppConfig::default();
        config.connection.port_profiles.insert(
            "/dev/ttyUSB1".to_string(),
            config::DefaultsConfig {
                baud_rate: 9600,
                data_bits: 7,
                parity: "even".to_string(),
                stop_bits: 2,
                flow_control: "hardware".to_string(),
                line_ending: "lf".to_string(),
            },
        );

        let mut cli = cli();
        cli.port = Some("/dev/ttyUSB1".to_string());
        cli.baud = Some(230400);
        cli.parity = Some("none".to_string());
        cli.line_ending = Some("crlf".to_string());

        let startup_port = resolve_startup_port(&cli, &config);
        let resolved = resolve_effective_defaults(&cli, &config, startup_port.as_deref());

        assert_eq!(resolved.baud_rate, 230400);
        assert_eq!(resolved.parity, "none");
        assert_eq!(resolved.data_bits, 7);
        assert_eq!(resolved.line_ending, "crlf");
    }
}
