use std::io::{Read, Write};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crossterm::event::KeyCode;
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use yapper::app::ConnectionState;
use yapper::harness::AppHarness;

#[test]
fn harness_drives_simulated_at_modem() {
    let mut harness = AppHarness::simulated();

    harness.advance_until_idle(Duration::from_millis(150), Duration::from_millis(25));
    harness.submit("AT");
    harness.advance_until_idle(Duration::from_millis(50), Duration::from_millis(10));

    let text = harness.buffer_text();
    assert!(matches!(
        harness.app.connection_state,
        ConnectionState::Connected(_)
    ));
    assert!(text.contains("BOOT: yapper simulated AT modem"));
    assert!(text.contains("READY"));
    assert!(text.contains("AT"));
    assert!(text.contains("OK"));
    assert!(harness.app.total_rx_bytes() > 0);
    assert!(harness.app.total_tx_bytes() > 0);
    assert_eq!(harness.app.quicksend, vec!["AT".to_string()]);
}

#[test]
fn harness_tests_search_filter_hex_and_reconnect_flows() {
    let mut harness = AppHarness::simulated();

    harness.advance_until_idle(Duration::from_millis(150), Duration::from_millis(25));
    harness.submit("AT+gmr");
    harness.advance_until_idle(Duration::from_millis(60), Duration::from_millis(10));

    harness.press(KeyCode::Esc);
    harness.press(KeyCode::Char('/'));
    harness.type_text("yapper-sim");
    assert_eq!(harness.app.search.current_line(), Some(3));
    harness.press(KeyCode::Enter);

    harness.press(KeyCode::Char('f'));
    harness.type_text("READY");
    harness.press(KeyCode::Enter);
    assert!(harness.app.filter.is_active);

    harness.press(KeyCode::Char('h'));
    assert!(harness.app.hex_mode);

    harness.press(KeyCode::Char('i'));
    harness.submit("AT+simdisconnect");
    harness.advance_until_idle(Duration::from_millis(30), Duration::from_millis(10));
    assert!(matches!(
        harness.app.connection_state,
        ConnectionState::Reconnecting(_)
    ));

    harness.advance_until_idle(Duration::from_millis(1100), Duration::from_millis(100));
    assert!(matches!(
        harness.app.connection_state,
        ConnectionState::Connected(_)
    ));
}

#[test]
fn ratatui_cell_snapshots_cover_responsive_simulator_screens() {
    let mut harness = AppHarness::simulated();
    harness.advance_until_idle(Duration::from_millis(150), Duration::from_millis(25));
    harness.submit("AT");
    harness.advance_until_idle(Duration::from_millis(50), Duration::from_millis(10));
    harness.advance(Duration::from_secs(4));

    for (width, height) in [(120, 32), (90, 24), (70, 20)] {
        let screen = harness.render_lines(width, height).join("\n");
        assert!(screen.contains("yapper"));
        assert!(screen.contains("Connected"));
        assert!(screen.contains("AT"));
        assert!(screen.contains("OK"));
    }

    harness.press(KeyCode::Esc);
    harness.press(KeyCode::Char('?'));
    let help_screen = harness.render_lines(90, 24).join("\n");
    assert!(help_screen.contains("Keybindings"));
    assert!(help_screen.contains("Normal Mode"));
}

#[test]
fn compiled_tui_responds_to_simulated_device_over_a_real_pty() {
    let binary = env!("CARGO_BIN_EXE_yapper");
    let temp_home = unique_temp_dir("pty-home");

    let pty_system = NativePtySystem::default();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open pty");

    let mut command = CommandBuilder::new(binary);
    command.arg("--simulate");
    command.arg("at-modem");
    command.env("HOME", &temp_home);
    command.env("XDG_CONFIG_HOME", temp_home.join(".config"));
    command.env("XDG_DATA_HOME", temp_home.join(".local/share"));

    let mut child = pair.slave.spawn_command(command).expect("spawn yapper");
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().expect("pty reader");
    let mut writer = pair.master.take_writer().expect("pty writer");
    let (tx, rx) = mpsc::channel();

    let reader_thread = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut parser = vt100::Parser::new(24, 100, 0);
    assert!(wait_for_screen(
        &rx,
        &mut parser,
        "Connected to sim://at-modem",
        Duration::from_secs(5)
    ));

    writer.write_all(b"AT\r").expect("write AT command");
    writer.flush().expect("flush AT command");
    assert!(wait_for_screen(
        &rx,
        &mut parser,
        "OK",
        Duration::from_secs(5)
    ));

    writer.write_all(b"\x1b").expect("leave input mode");
    writer.flush().expect("flush escape");
    assert!(wait_for_screen(
        &rx,
        &mut parser,
        "i: input",
        Duration::from_secs(5)
    ));
    writer.write_all(b"?").expect("open help");
    writer.flush().expect("flush help");
    assert!(wait_for_screen(
        &rx,
        &mut parser,
        "Keybindings",
        Duration::from_secs(5)
    ));

    let _ = writer.write_all(b"\x1bq");
    let _ = writer.flush();
    std::thread::sleep(Duration::from_millis(100));
    let _ = child.kill();
    let _ = child.wait();
    drop(writer);
    let _ = reader_thread.join();
}

fn wait_for_screen(
    rx: &mpsc::Receiver<Vec<u8>>,
    parser: &mut vt100::Parser,
    needle: &str,
    timeout: Duration,
) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(bytes) => {
                parser.process(&bytes);
                if parser.screen().contents().contains(needle) {
                    return true;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    false
}

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "yapper-{}-{}-{}",
        prefix,
        std::process::id(),
        nanos
    ));
    std::fs::create_dir_all(&path).expect("create temp dir");
    path
}
