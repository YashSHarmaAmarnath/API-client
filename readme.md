# 🦀 api-client.exe

A terminal-based REST client built with Rust, [ratatui](https://github.com/ratatui-org/ratatui), and [reqwest](https://github.com/seanmonstar/reqwest). Think Insomnia or Postman — but in your terminal.

![Rust](https://img.shields.io/badge/Rust-2021-orange?logo=rust)

---

## Features

- **Collections** — group requests into named collections with descriptions, persisted in SQLite
- **All HTTP methods** — GET, POST, PUT, PATCH, DELETE with per-method color coding
- **Query params editor** — add and edit key/value pairs with row navigation
- **Headers editor** — set custom request headers inline
- **Body editor** — write JSON request bodies directly in the TUI
- **Response viewer** — scrollable, pretty-printed JSON with status, timing, headers, and raw text
- **Vim-inspired modes** — INSERT for editing, NORMAL for navigation
- **Persistent storage** — collections and requests saved to `data.db` via SQLite

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

| Mode   | Enter via | Purpose                           |
|--------|-----------|-----------------------------------|
| INSERT | `i`       | Type in fields, edit content      |
| NORMAL | `Esc`     | Navigate panels, scroll response  |

### Keybindings

#### NORMAL mode — Navigation

| Key       | Action                          |
|-----------|---------------------------------|
| `Tab`     | Cycle focus to next panel       |
| `Shift+Tab` | Cycle focus to previous panel |
| `j` / `↓` | Move down / scroll response    |
| `k` / `↑` | Move up / scroll response      |
| `i`       | Enter INSERT mode (editor open) |
| `q`       | Quit                            |
| `?`       | Toggle help overlay             |

#### NORMAL mode — Collections & Requests panels

| Key    | Action                                    |
|--------|-------------------------------------------|
| `n`    | New collection (when Collections focused) |
| `n`    | New request (when Requests focused)       |
| `d`    | Delete selected item (with confirmation)  |
| `e`    | Open selected request in editor           |
| `s`    | Send current request                      |
| `w`    | Save editor changes back to DB            |

#### INSERT mode — editor

| Key          | Action                                    |
|--------------|-------------------------------------------|
| `Esc`        | Return to NORMAL mode                     |
| `Tab`        | Cycle editor tab (URL → Query → Headers → Body) |
| `Ctrl+M`     | Cycle HTTP method                         |
| `Backspace`  | Delete last character                     |
| `↓` / `↑`   | Navigate rows (Query / Headers tabs)      |
| `→` / `←`   | Toggle Key ↔ Value field                  |
| `Enter`      | Insert newline (Body tab only)            |

#### New Request / New Collection overlays

| Key    | Action                      |
|--------|-----------------------------|
| `Tab`  | Next field                  |
| `←/→`  | Cycle HTTP method (method field) |
| `Enter`| Confirm and create          |
| `Esc`  | Cancel                      |

---

## Layout

```
┌────────────────┬──────────────────────────────────────┐
│  Collections   │  Requests                            │  Response
│  (j/k, n, d)   │  (j/k, n, d, e)                     │  (j/k scroll)
└────────────────┴──────────────────────────────────────┘
┌──────────────────────────────────────────────────────────┐
│  Request Editor                                          │
│  [METHOD] URL                                            │
│  URL │ Query │ Headers │ Body                            │
│  ...                                                     │
└──────────────────────────────────────────────────────────┘
│  Footer: mode · panel · keybind hints                    │
└──────────────────────────────────────────────────────────┘
```

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

## Database

Requests and collections are stored in `data.db` (SQLite) in the project root.

**Schema:**

```
collections
  collection_id  INTEGER PRIMARY KEY
  collection_name TEXT
  description     TEXT
  created_at      TIMESTAMP

api_requests
  request_id     INTEGER PRIMARY KEY
  collection_id  INTEGER → collections (CASCADE DELETE)
  request_name   TEXT
  method         TEXT
  url            TEXT
  headers        TEXT  (JSON)
  body           TEXT  (JSON)
```

> Deleting a collection cascade-deletes all its requests automatically.

---

## Project Structure

```
src/
├── main.rs      # App state, UI rendering, event loop
├── db_utils.rs  # SQLite helpers (collections + requests CRUD)
└── utils.rs     # HTTP helpers (get, post, put, patch, delete)
```

---

## Dependencies

| Crate        | Purpose                         |
|--------------|---------------------------------|
| `ratatui`    | Terminal UI framework           |
| `crossterm`  | Cross-platform terminal control |
| `reqwest`    | Async HTTP client               |
| `tokio`      | Async runtime                   |
| `rusqlite`   | SQLite database                 |
| `serde_json` | JSON serialization              |
| `serde`      | Derive Serialize / Deserialize  |

---

## TODO

- [ ] **Save query params to DB** — `api_requests` has no `query` column yet; add it alongside `headers` and `body` so query params entered in the editor persist across sessions
- [ ] **Save headers on create** — `add_request()` currently passes `None` for headers; wire the dialog's header input through to `db_insert_api_request`
- [ ] **Save query params on create** — same as above for query params at creation time
- [ ] **Save body on create** — same as above for body at creation time
- [ ] **Update request on edit** — implement `db_update_request()` and call it on `w` keypress so URL, method, headers, query, and body edits persist
- [ ] **Update collection** — add `db_update_collection()` and an edit overlay so collection name/description can be changed after creation
- [ ] **Export / import** — dump collections to JSON for sharing or backup
- [ ] **Environment variables** — support `{{base_url}}` style placeholders in URLs and headers
- [ ] **Make code modular** - split main file in multiple sub code file 