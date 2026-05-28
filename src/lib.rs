#![allow(dead_code)]

pub mod app;
pub mod buffer;
pub mod clipboard;
pub mod config;
pub mod display;
pub mod event;
pub mod filter;
pub mod harness;
pub mod hex;
pub mod highlight;
pub mod history;
pub mod input;
pub mod logging;
pub mod macros;
pub mod mouse;
pub mod search;
pub mod serial;
pub mod sim;
pub mod theme;
pub mod transport;
pub mod ui;

pub use app::{App, Mode};
