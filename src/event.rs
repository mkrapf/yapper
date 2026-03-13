use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::prelude::*;
use std::time::Duration;

use crate::app::App;
use crate::input::handle_key_event;
use crate::mouse::handle_mouse_event;
use crate::ui;

/// The main event loop: multiplexes terminal events, serial RX, and ticks.
pub struct EventLoop {
    tick_rate: Duration,
}

impl EventLoop {
    pub fn new() -> Self {
        Self {
            tick_rate: Duration::from_millis(16), // ~60 FPS
        }
    }

    /// Run the event loop until the app signals quit.
    pub fn run<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
        app: &mut App,
    ) -> Result<()> {
        let mut needs_render = true;

        loop {
            // Drain serial events
            let had_serial = app.poll_serial();
            if had_serial {
                needs_render = true;
            }

            // Drain ALL pending terminal events before rendering.
            while event::poll(Duration::ZERO)? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        handle_key_event(app, key);
                        needs_render = true;
                    }
                    Event::Mouse(mouse) => {
                        handle_mouse_event(app, mouse);
                        needs_render = true;
                    }
                    Event::Resize(_, _) => {
                        needs_render = true;
                    }
                    _ => {}
                }
            }

            // Check quit before render
            if app.should_quit {
                app.disconnect();
                return Ok(());
            }

            // Only render when state has actually changed
            if needs_render {
                terminal.draw(|frame| ui::render(app, frame))?;
                needs_render = false;
            }

            // Sleep until next event or tick
            match event::poll(self.tick_rate)? {
                true => {
                    // Event arrived — will be drained next iteration
                    match event::read()? {
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
                            handle_key_event(app, key);
                            needs_render = true;
                        }
                        Event::Mouse(mouse) => {
                            handle_mouse_event(app, mouse);
                            needs_render = true;
                        }
                        Event::Resize(_, _) => {
                            needs_render = true;
                        }
                        _ => {}
                    }
                }
                false => {
                    // Tick expired — check serial and re-render if there's
                    // a status message timer or reconnect animation
                    if app.status_message.is_some() || app.is_reconnecting() {
                        needs_render = true;
                    }
                }
            }
        }
    }
}
