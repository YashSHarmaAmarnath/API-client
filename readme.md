# 🦀 api-client.exe

A terminal-based REST client built with Rust, [ratatui](https://github.com/ratatui-org/ratatui), and [reqwest](https://github.com/seanmonstar/reqwest). Think Insomnia or Postman — but in your terminal.

![Rust](https://img.shields.io/badge/Rust-2021-orange?logo=rust)

---

## Features

- **All HTTP methods** — GET, POST, PUT, PATCH, DELETE with per-method color coding
- **Query params editor** — add and edit key/value pairs with row navigation
- **Headers editor** — set custom request headers inline
- **Body editor** — write JSON request bodies directly in the TUI
- **Response viewer** — scrollable, pretty-printed JSON with status, timing, headers, and raw text
- **Dual-focus input** — switch keyboard focus between the URL bar and editor panel
- **Vim-inspired modes** — INSERT for editing, NORMAL for navigation

---

## Installation

**Prerequisites:** Rust toolchain (1.70+) — install via [rustup](https://rustup.rs)

```bash
git clone https://github.com/YashSHarmaAmarnath/API-client
cd API-client
cargo build --release
./target/release/api-client
```

---

## Usage

### Modes

| Mode   | Enter via       | Purpose                        |
|--------|-----------------|--------------------------------|
| INSERT | `i`             | Type in fields, edit content   |
| NORMAL | `Esc`           | Navigate methods, scroll response |

### Keybindings

#### NORMAL mode

| Key      | Action                        |
|----------|-------------------------------|
| `i`      | Enter INSERT mode             |
| `q`      | Quit                          |
| `j`      | Scroll response down          |
| `k`      | Scroll response up            |
| `↑`      | Select previous HTTP method   |
| `↓`      | Select next HTTP method       |

#### INSERT mode — general

| Key      | Action                                  |
|----------|-----------------------------------------|
| `Esc`    | Return to NORMAL mode                   |
| `F2`     | Toggle focus between URL bar and editor |
| `Tab`    | Switch editor tab (Query/Header/Body)   |
| `Enter`  | Send the request                        |

#### INSERT mode — URL bar (focus: URL)

| Key         | Action              |
|-------------|---------------------|
| Any char    | Append to URL       |
| `Backspace` | Delete last char    |

#### INSERT mode — Query / Header editor (focus: EDITOR)

| Key         | Action                              |
|-------------|-------------------------------------|
| Any char    | Type into active field (key or val) |
| `Backspace` | Delete last char in active field    |
| `→` / `←`  | Toggle between Key and Value field  |
| `↓`         | Move to next row (adds row if last) |
| `↑`         | Move to previous row                |

#### INSERT mode — Body editor (focus: EDITOR, tab: Body)

| Key         | Action                   |
|-------------|--------------------------|
| Any char    | Append to body string    |
| `Backspace` | Delete last char         |

---

## Layout

```
┌─────────────┬──────────────────────────────────────┐
│  Methods    │  URL                                 │
│  (↑↓)       ├──────────────────────────────────────┤
├─────────────│  Response                            │
│  Request    │                                      │
│  Query│Hdr│Body│                                   │
│  Editor     │                                      │
└─────────────┴──────────────────────────────────────┘
│  Footer: mode · focus · hints · status             │
└────────────────────────────────────────────────────┘
```

- **Left top** — HTTP method list, highlighted with method-specific color
- **Left bottom** — Tabbed editor for query params, headers, and JSON body
- **Right top** — URL input bar
- **Right bottom** — Scrollable response panel
- **Footer** — Current mode, focus area, contextual keybind hints, and request status

---

## Method Colors

| Method | Color  |
|--------|--------|
| GET    | Green  |
| POST   | Blue   |
| PUT    | Yellow |
| PATCH  | Cyan   |
| DELETE | Red    |

---

## Response Output

After a request completes, the response panel shows:

- HTTP status code
- Response time
- Final URL (after redirects)
- Content length
- HTTP version
- Response headers
- Raw response text
- Pretty-printed JSON body

---

## Project Structure

```
src/
├── main.rs        # App state, UI rendering, event loop
└── utils.rs       # HTTP helpers (get, post, put, patch, delete, url_splitter)
```

---

## Dependencies

| Crate        | Purpose                          |
|--------------|----------------------------------|
| `ratatui`    | Terminal UI framework            |
| `crossterm`  | Cross-platform terminal control  |
| `reqwest`    | Async HTTP client                |
| `tokio`      | Async runtime                    |
| `serde_json` | JSON serialization               |