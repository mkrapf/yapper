use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

use crate::app::App;
use crate::config::AppConfig;
use crate::input::handle_key_event;
use crate::serial::config::SerialConfig;
use crate::sim::{SimProfile, SimTransport};
use crate::ui;

static HARNESS_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub struct AppHarness {
    pub app: App,
    now: Instant,
}

impl AppHarness {
    pub fn simulated() -> Self {
        Self::with_sim_profile(SimProfile::AtModem)
    }

    pub fn with_sim_profile(profile: SimProfile) -> Self {
        let mut config = AppConfig::default();
        config.connection.auto_connect = false;
        config.history.file = unique_temp_path("history").display().to_string();
        config.logging.log_directory = unique_temp_path("logs").display().to_string();

        let transport = Arc::new(SimTransport::new(profile));
        let mut app = App::new_with_transport(
            SerialConfig::default(),
            "\r\n".to_string(),
            config,
            transport,
        );
        app.connect(profile.port_name());

        Self {
            app,
            now: Instant::now(),
        }
    }

    pub fn press(&mut self, code: KeyCode) {
        handle_key_event(&mut self.app, KeyEvent::new(code, KeyModifiers::NONE));
    }

    pub fn press_ctrl(&mut self, ch: char) {
        handle_key_event(
            &mut self.app,
            KeyEvent::new(KeyCode::Char(ch), KeyModifiers::CONTROL),
        );
    }

    pub fn type_text(&mut self, text: &str) {
        for ch in text.chars() {
            self.press(KeyCode::Char(ch));
        }
    }

    pub fn submit(&mut self, text: &str) {
        self.type_text(text);
        self.press(KeyCode::Enter);
    }

    pub fn advance(&mut self, duration: Duration) -> bool {
        self.now += duration;
        let mut changed = self.app.tick(self.now);
        changed |= self.app.poll_serial();
        changed
    }

    pub fn advance_until_idle(&mut self, total: Duration, step: Duration) {
        let mut elapsed = Duration::ZERO;
        while elapsed < total {
            let next = step.min(total - elapsed);
            self.advance(next);
            elapsed += next;
        }
    }

    pub fn buffer_text(&self) -> String {
        self.buffer_lines().join("\n")
    }

    pub fn buffer_lines(&self) -> Vec<String> {
        self.app
            .buffer
            .iter()
            .map(|entry| entry.text.clone())
            .collect()
    }

    pub fn render_lines(&mut self, width: u16, height: u16) -> Vec<String> {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("test backend terminal");
        terminal
            .draw(|frame| ui::render(&mut self.app, frame))
            .expect("render app");
        buffer_lines(terminal.backend().buffer(), width, height)
    }
}

fn unique_temp_path(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "yapper-harness-{}-{}-{}",
        prefix,
        std::process::id(),
        HARNESS_COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

fn buffer_lines(buffer: &ratatui::buffer::Buffer, width: u16, height: u16) -> Vec<String> {
    (0..height)
        .map(|y| {
            let mut line = String::new();
            for x in 0..width {
                if let Some(cell) = buffer.cell((x, y)) {
                    line.push_str(cell.symbol());
                }
            }
            line.trim_end().to_string()
        })
        .collect()
}
