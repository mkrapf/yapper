use std::time::{Duration, Instant};

use super::{App, Mode, PendingMacroCommand, SendSource};

impl App {
    pub fn open_macro_selector(&mut self) {
        self.macro_select_index = 0;
        self.open_overlay(Mode::MacroSelect);
    }

    pub fn open_help(&mut self) {
        self.help_scroll = 0;
        self.help_scroll_max = 0;
        self.open_overlay(Mode::Help);
    }

    pub fn send_text(&mut self, text: &str) {
        self.send_plain_text(text, SendSource::Macro);
    }

    pub fn execute_macro(&mut self, name: &str) {
        if self.active_macro_name.is_some() {
            self.set_status("A macro is already running".to_string());
            return;
        }

        if let Some(m) = self.macros.get(name) {
            let commands = m.commands.clone();
            if commands.is_empty() {
                self.set_status(format!("Macro has no commands: {}", name));
                return;
            }
            let mut ready_at = Instant::now();
            let pending = commands.into_iter().map(|command| {
                ready_at += Duration::from_millis(command.delay_ms);
                PendingMacroCommand {
                    text: command.text,
                    ready_at,
                }
            });

            self.pending_macro_commands = pending.collect();
            self.active_macro_name = Some(name.to_string());
            self.last_executed_macro = Some(name.to_string());
            self.set_status(format!("Running macro: {}", name));
        } else {
            self.set_status(format!("Macro not found: {}", name));
        }
    }

    pub fn execute_selected_macro(&mut self) {
        let macros = self.macros.list();
        if let Some(m) = macros.get(self.macro_select_index) {
            let name = m.name.clone();
            self.execute_macro(&name);
        }
    }

    pub(super) fn drain_macro_queue(&mut self, now: Instant) -> bool {
        let mut changed = false;

        loop {
            let ready = match self.pending_macro_commands.front() {
                Some(command) => command.ready_at <= now,
                None => false,
            };

            if !ready {
                break;
            }

            if let Some(command) = self.pending_macro_commands.pop_front() {
                self.send_plain_text(&command.text, SendSource::Macro);
                changed = true;
            }
        }

        if changed && self.pending_macro_commands.is_empty() {
            if let Some(name) = self.active_macro_name.take() {
                self.set_status(format!("Finished macro: {}", name));
            }
        }

        changed
    }
}
