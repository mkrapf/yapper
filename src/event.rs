use anyhow::Result;
use crossterm::event::{self, Event};
use ratatui::prelude::*;
use std::time::Duration;

use crate::app::App;
use crate::input::handle_key_event;
use crate::ui;

/// The main event loop: multiplexes terminal events, serial RX, and ticks.
pub struct EventLoop {
    tick_rate: Duration,
}

impl EventLoop {
    pub fn new() -> Self {
        Self {
            tick_rate: Duration::from_millis(33), // ~30 FPS
        }
    }

    /// Run the event loop until the app signals quit.
    pub fn run<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
        app: &mut App,
    ) -> Result<()> {
        loop {
            // Render
            terminal.draw(|frame| ui::render(app, frame))?;

            // Poll for terminal events (with tick timeout)
            if event::poll(self.tick_rate)? {
                if let Event::Key(key) = event::read()? {
                    handle_key_event(app, key);
                }
            }

            // Drain serial events
            app.poll_serial();

            // Check quit
            if app.should_quit {
                // Clean disconnect
                app.disconnect();
                return Ok(());
            }
        }
    }
}
