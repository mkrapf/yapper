# yapper 🔌

A snappy, ergonomic serial terminal for embedded workflows. Built with Rust + [ratatui](https://ratatui.rs).

## Features

- **Port auto-detection** — Scans system ports, select with `p`
- **Interactive UART settings** — Press `s` to configure baud, data bits, parity, stop bits, flow control on-the-fly
- **Mouse support** — Scroll wheel, click-to-focus, drag-to-select + copy to clipboard
- **Search** — Vim-style `/` search with match highlighting and `n`/`N` navigation
- **Hex view** — Toggle raw hex dump with `h`
- **Command history** — Persistent across sessions, navigate with `↑`/`↓`
- **Session logging** — Auto-timestamped log files, toggle with `l`
- **Macro system** — Define command sequences in `macros.toml`
- **Regex filters** — Include/exclude lines by pattern
- **Auto-reconnect** — Reconnects automatically on port disconnect
- **Timestamps & line endings** — Per-line timestamps (`t`), line ending indicators (`e`)
- **Dracula theme** — Easy on the eyes during long debug sessions

## Install

```bash
cargo install --path .
```

## Usage

```bash
# Auto-detect port
yapper

# Specify port and baud rate
yapper /dev/ttyUSB0 115200

# Full configuration
yapper /dev/ttyACM0 9600 --data-bits 8 --parity none --stop-bits 1
```

### WSL

Works with `cargo.exe run` from WSL — connects to Windows COM ports directly.

## Keybindings

| Key | Action |
|-----|--------|
| `i` | Enter input mode |
| `Esc` | Back to normal mode |
| `p` | Port selector |
| `s` | UART settings |
| `c` | Connect/disconnect |
| `j`/`k` | Scroll up/down |
| `G`/`g` | Scroll to bottom/top |
| `/` | Search |
| `n`/`N` | Next/prev match |
| `h` | Hex view |
| `t` | Timestamps |
| `e` | Line endings |
| `l` | Toggle logging |
| `?` | Help |
| `q` | Quit |

### Mouse

| Action | Effect |
|--------|--------|
| Scroll wheel | Scroll output / navigate popups |
| Click input bar | Focus input |
| Click status bar | Connect (left) / Settings (right) |
| Click-drag | Select text → copies to clipboard |

## Configuration

### Macros

Create `~/.config/yapper/macros.toml`:

```toml
[[macros]]
name = "reset"
commands = ["AT+RST"]

[[macros]]
name = "init_wifi"
commands = ["AT+CWMODE=1", "AT+CWJAP=\"SSID\",\"PASS\""]
delay_ms = 500
```

## License

MIT
