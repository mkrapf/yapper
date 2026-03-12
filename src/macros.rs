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
        let file_path = dirs::config_dir().map(|d| d.join("yap").join("macros.toml"));

        let mut mgr = Self {
            macros: BTreeMap::new(),
            file_path,
        };
        mgr.load();
        mgr
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
            if let Ok(macros) = toml::from_str::<BTreeMap<String, MacroFile>>(&content) {
                for (name, mf) in macros {
                    self.macros.insert(
                        name.clone(),
                        Macro {
                            name,
                            description: mf.description,
                            commands: mf
                                .commands
                                .into_iter()
                                .map(|c| MacroCommand {
                                    text: c.text,
                                    delay_ms: c.delay_ms.unwrap_or(0),
                                })
                                .collect(),
                        },
                    );
                }
            }
        }
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

        let mut map: BTreeMap<String, MacroFile> = BTreeMap::new();
        for (name, m) in &self.macros {
            map.insert(
                name.clone(),
                MacroFile {
                    description: m.description.clone(),
                    commands: m
                        .commands
                        .iter()
                        .map(|c| MacroCommandFile {
                            text: c.text.clone(),
                            delay_ms: if c.delay_ms > 0 {
                                Some(c.delay_ms)
                            } else {
                                None
                            },
                        })
                        .collect(),
                },
            );
        }

        if let Ok(content) = toml::to_string_pretty(&map) {
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

/// File format for macros (slightly different from in-memory for serde flexibility).
#[derive(Debug, Serialize, Deserialize)]
struct MacroFile {
    description: String,
    commands: Vec<MacroCommandFile>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MacroCommandFile {
    text: String,
    delay_ms: Option<u64>,
}
