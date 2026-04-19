use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A macro is a named sequence of commands to send.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Macro {
    pub name: String,
    pub description: String,
    pub commands: Vec<MacroCommand>,
}

/// A single command within a macro.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroCommand {
    /// The text to send.
    pub text: String,
    /// Delay in milliseconds before sending this command.
    #[serde(default)]
    pub delay_ms: u64,
}

/// Manages macros stored in a TOML file.
pub struct MacroManager {
    macros: BTreeMap<String, Macro>,
    file_path: Option<PathBuf>,
}

impl MacroManager {
    pub fn new() -> Self {
        let file_path = dirs::config_dir().map(|d| d.join("yapper").join("macros.toml"));
        let mut mgr = Self::with_path(file_path);
        mgr.load();
        mgr
    }

    pub fn with_path(file_path: Option<PathBuf>) -> Self {
        Self {
            macros: BTreeMap::new(),
            file_path,
        }
    }

    #[cfg(test)]
    pub fn new_in_memory() -> Self {
        Self::with_path(None)
    }

    /// Get all macros sorted by name.
    pub fn list(&self) -> Vec<&Macro> {
        self.macros.values().collect()
    }

    /// Get a macro by name.
    pub fn get(&self, name: &str) -> Option<&Macro> {
        self.macros.get(name)
    }

    /// Add or update a macro.
    pub fn save_macro(&mut self, m: Macro) {
        self.macros.insert(m.name.clone(), m);
        self.save();
    }

    /// Remove a macro.
    pub fn remove(&mut self, name: &str) {
        self.macros.remove(name);
        self.save();
    }

    /// Load macros from file.
    fn load(&mut self) {
        let path = match &self.file_path {
            Some(p) => p,
            None => return,
        };

        if !path.exists() {
            // Create default example macros
            self.create_defaults();
            return;
        }

        if let Ok(content) = fs::read_to_string(path) {
            if let Some(macros) = parse_macro_file(&content) {
                self.macros = macros;
            }
        }
    }

    /// Reload macros from disk, preserving the current set if parsing fails.
    pub fn reload(&mut self) {
        self.load();
    }

    /// Save macros to file.
    fn save(&self) {
        let path = match &self.file_path {
            Some(p) => p,
            None => return,
        };

        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let document = CanonicalMacroDocument {
            macros: self
                .macros
                .values()
                .cloned()
                .map(CanonicalMacroFile::from_macro)
                .collect(),
        };

        if let Ok(content) = toml::to_string_pretty(&document) {
            let _ = fs::write(path, content);
        }
    }

    fn create_defaults(&mut self) {
        self.macros.insert(
            "reset".to_string(),
            Macro {
                name: "reset".to_string(),
                description: "Send reset command".to_string(),
                commands: vec![MacroCommand {
                    text: "reset".to_string(),
                    delay_ms: 0,
                }],
            },
        );
        self.macros.insert(
            "version".to_string(),
            Macro {
                name: "version".to_string(),
                description: "Query firmware version".to_string(),
                commands: vec![MacroCommand {
                    text: "version".to_string(),
                    delay_ms: 0,
                }],
            },
        );
        self.save();
    }
}

fn parse_macro_file(content: &str) -> Option<BTreeMap<String, Macro>> {
    if let Ok(document) = toml::from_str::<CanonicalMacroDocument>(content) {
        if !document.macros.is_empty() {
            let mut macros = BTreeMap::new();
            for macro_file in document.macros {
                let name = macro_file.name.clone();
                macros.insert(name, macro_file.into_macro());
            }
            return Some(macros);
        }
    }

    if let Ok(document) = toml::from_str::<BTreeMap<String, LegacyMacroFile>>(content) {
        let mut macros = BTreeMap::new();
        for (name, macro_file) in document {
            let macro_name = name.clone();
            macros.insert(
                name,
                Macro {
                    name: macro_name,
                    description: macro_file.description,
                    commands: macro_file
                        .commands
                        .into_iter()
                        .map(|command| MacroCommand {
                            text: command.text,
                            delay_ms: command.delay_ms.unwrap_or(0),
                        })
                        .collect(),
                },
            );
        }
        return Some(macros);
    }

    None
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CanonicalMacroDocument {
    #[serde(default)]
    macros: Vec<CanonicalMacroFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CanonicalMacroFile {
    name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    description: String,
    commands: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    delay_ms: Option<u64>,
}

impl CanonicalMacroFile {
    fn into_macro(self) -> Macro {
        let delay_ms = self.delay_ms.unwrap_or(0);
        let commands = self
            .commands
            .into_iter()
            .enumerate()
            .map(|(index, text)| MacroCommand {
                text,
                delay_ms: if index == 0 { 0 } else { delay_ms },
            })
            .collect();

        Macro {
            name: self.name,
            description: self.description,
            commands,
        }
    }

    fn from_macro(m: Macro) -> Self {
        let delay_ms = macro_delay_ms(&m.commands);
        Self {
            name: m.name,
            description: m.description,
            commands: m.commands.into_iter().map(|c| c.text).collect(),
            delay_ms,
        }
    }
}

fn macro_delay_ms(commands: &[MacroCommand]) -> Option<u64> {
    let mut delays = commands.iter().skip(1).map(|command| command.delay_ms);
    let first_delay = delays.next()?;
    if commands
        .first()
        .map(|command| command.delay_ms != 0)
        .unwrap_or(false)
    {
        return None;
    }
    if delays.all(|delay| delay == first_delay) {
        Some(first_delay)
    } else {
        None
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct LegacyMacroFile {
    #[serde(default)]
    description: String,
    commands: Vec<LegacyMacroCommandFile>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LegacyMacroCommandFile {
    text: String,
    delay_ms: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_canonical_macro_file() {
        let content = r#"
            [[macros]]
            name = "reset"
            description = "Reset modem"
            commands = ["AT+RST", "AT"]
            delay_ms = 500
        "#;

        let macros = parse_macro_file(content).unwrap();
        let reset = macros.get("reset").unwrap();
        assert_eq!(reset.commands.len(), 2);
        assert_eq!(reset.commands[0].text, "AT+RST");
        assert_eq!(reset.commands[0].delay_ms, 0);
        assert_eq!(reset.commands[1].delay_ms, 500);
    }

    #[test]
    fn test_parse_legacy_macro_file() {
        let content = r#"
            [wifi]
            description = "Bring WiFi up"

            [[wifi.commands]]
            text = "AT+CWMODE=1"

            [[wifi.commands]]
            text = "AT+CWJAP=\"ssid\",\"pass\""
            delay_ms = 750
        "#;

        let macros = parse_macro_file(content).unwrap();
        let wifi = macros.get("wifi").unwrap();
        assert_eq!(wifi.commands.len(), 2);
        assert_eq!(wifi.commands[1].delay_ms, 750);
    }

    #[test]
    fn test_save_uses_canonical_format() {
        let macro_file = CanonicalMacroFile::from_macro(Macro {
            name: "reset".to_string(),
            description: "Reset modem".to_string(),
            commands: vec![
                MacroCommand {
                    text: "AT+RST".to_string(),
                    delay_ms: 0,
                },
                MacroCommand {
                    text: "AT".to_string(),
                    delay_ms: 250,
                },
            ],
        });

        assert_eq!(macro_file.delay_ms, Some(250));
        assert_eq!(
            macro_file.commands,
            vec!["AT+RST".to_string(), "AT".to_string()]
        );
    }
}
