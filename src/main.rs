use std::{error::Error, io, time::Duration};
use std::collections::HashMap;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap, Tabs},
    text::{Line, Span},
};

use reqwest::Client;
use reqwest::header::{CONNECTION, HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;

use rusqlite::{Connection};
mod utils;
use utils::{delete, get, patch, post, put, url_splitter};

mod db_utils;
use db_utils::{check_database};

// =========================================
// ENUMS
// =========================================

#[derive(PartialEq)]
enum InputMode {
    Normal,
    Insert,
}

#[derive(Clone, Copy, PartialEq)]
enum ActiveTab {
    Query,
    Header,
    Body,
}

/// Which field in a key-value pair is being edited
#[derive(Clone, Copy, PartialEq)]
enum KvFocus {
    Key,
    Value,
}

/// Which area has keyboard focus in Insert mode
#[derive(Clone, Copy, PartialEq)]
enum FocusArea {
    Url,
    Editor,
}

// =========================================
// DATA
// =========================================

#[derive(Default, Clone)]
struct KeyValue {
    key: String,
    value: String,
}

// =========================================
// APP
// =========================================

struct App {
    url: String,
    response: String,
    status: String,
    methods: Vec<&'static str>,
    selected_method: usize,
    response_scroll: u16,
    mode: InputMode,
    active_tab: ActiveTab,
    query_params: Vec<KeyValue>,
    headers: Vec<KeyValue>,
    body: String,
    /// Index of selected row in query_params / headers
    selected_pair: usize,
    /// Which field (key or value) is active in kv editors
    kv_focus: KvFocus,
    /// Whether URL bar or editor panel has focus in Insert mode
    focus_area: FocusArea,
}

impl Default for App {
    fn default() -> Self {
        Self {
            url: String::from("https://jsonplaceholder.typicode.com/posts/1"),
            response: String::from("Response will appear here"),
            status: String::from("Ready"),
            methods: vec!["GET", "POST", "PUT", "PATCH", "DELETE"],
            selected_method: 0,
            response_scroll: 0,
            mode: InputMode::Insert,
            active_tab: ActiveTab::Query,
            query_params: vec![KeyValue::default()],
            headers: vec![KeyValue::default()],
            body: String::from("{}"),
            selected_pair: 0,
            kv_focus: KvFocus::Key,
            focus_area: FocusArea::Url,
        }
    }
}

impl App {
    // ---- scrolling ----

    fn scroll_down(&mut self) {
        self.response_scroll = self.response_scroll.saturating_add(1);
    }

    fn scroll_up(&mut self) {
        self.response_scroll = self.response_scroll.saturating_sub(1);
    }

    // ---- tab switching ----

    fn next_tab(&mut self) {
        self.active_tab = match self.active_tab {
            ActiveTab::Query => ActiveTab::Header,
            ActiveTab::Header => ActiveTab::Body,
            ActiveTab::Body => ActiveTab::Query,
        };
        self.selected_pair = 0;
        self.kv_focus = KvFocus::Key;
    }

    // ---- method selection ----

    fn next_method(&mut self) {
        self.selected_method = (self.selected_method + 1) % self.methods.len();
    }

    fn prev_method(&mut self) {
        if self.selected_method == 0 {
            self.selected_method = self.methods.len() - 1;
        } else {
            self.selected_method -= 1;
        }
    }

    // ---- URL editing ----

    fn url_push(&mut self, c: char) {
        self.url.push(c);
    }

    fn url_backspace(&mut self) {
        self.url.pop();
    }

    // ---- KV editing (query params & headers) ----

    fn current_kv_list_mut(&mut self) -> &mut Vec<KeyValue> {
        match self.active_tab {
            ActiveTab::Query => &mut self.query_params,
            ActiveTab::Header => &mut self.headers,
            ActiveTab::Body => unreachable!("body has no kv list"),
        }
    }

    fn kv_push_char(&mut self, c: char) {
        let idx = self.selected_pair;
        let focus = self.kv_focus;
        let list = self.current_kv_list_mut();
        if let Some(kv) = list.get_mut(idx) {
            match focus {
                KvFocus::Key => kv.key.push(c),
                KvFocus::Value => kv.value.push(c),
            }
        }
    }

    fn kv_backspace(&mut self) {
        let idx = self.selected_pair;
        let focus = self.kv_focus;
        let list = self.current_kv_list_mut();
        if let Some(kv) = list.get_mut(idx) {
            match focus {
                KvFocus::Key => { kv.key.pop(); }
                KvFocus::Value => { kv.value.pop(); }
            }
        }
    }

    /// Move cursor between key and value fields
    fn kv_toggle_field(&mut self) {
        self.kv_focus = match self.kv_focus {
            KvFocus::Key => KvFocus::Value,
            KvFocus::Value => KvFocus::Key,
        };
    }

    fn kv_next_row(&mut self) {
        let len = match self.active_tab {
            ActiveTab::Query => self.query_params.len(),
            ActiveTab::Header => self.headers.len(),
            ActiveTab::Body => return,
        };
        if self.selected_pair + 1 < len {
            self.selected_pair += 1;
        } else {
            // Add a new empty row and move to it
            self.current_kv_list_mut().push(KeyValue::default());
            self.selected_pair += 1;
        }
        self.kv_focus = KvFocus::Key;
    }

    fn kv_prev_row(&mut self) {
        if self.selected_pair > 0 {
            self.selected_pair -= 1;
        }
        self.kv_focus = KvFocus::Key;
    }

    // ---- body editing ----

    fn body_push_char(&mut self, c: char) {
        self.body.push(c);
    }

    fn body_backspace(&mut self) {
        self.body.pop();
    }

    // ---- focus ----

    fn toggle_focus_area(&mut self) {
        self.focus_area = match self.focus_area {
            FocusArea::Url => FocusArea::Editor,
            FocusArea::Editor => FocusArea::Url,
        };
    }

    // ---- request building ----

    fn build_query_map(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for kv in &self.query_params {
            if !kv.key.is_empty() {
                map.insert(kv.key.clone(), kv.value.clone());
            }
        }
        map
    }

    fn build_headers(&self) -> HeaderMap {
        let mut header = HeaderMap::new();
        for kv in &self.headers {
            if kv.key.is_empty() {
                continue;
            }
            if let Ok(name) = HeaderName::from_bytes(kv.key.as_bytes()) {
                if let Ok(value) = HeaderValue::from_str(&kv.value) {
                    header.insert(name, value);
                }
            }
        }
        header
    }

    fn build_body(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|_| serde_json::json!({}))
    }

    async fn send_request(&mut self) {
        let client = Client::new();
        self.status = "Sending request...".into();

        let method = self.methods[self.selected_method];

        let (baseurl, endpoint) = match url_splitter(&self.url) {
            Some(parts) => parts,
            None => {
                self.status = "Invalid URL".into();
                return;
            }
        };

        let query_map = self.build_query_map();
        let headers = self.build_headers();
        let body = self.build_body();

        let result = match method {
            "GET" => get::<Value>(&client, &endpoint, &baseurl, Some(&query_map), Some(headers)).await,
            "POST" => post::<Value, _>(&client, &endpoint, &baseurl, &body, Some(&query_map), Some(headers)).await,
            "PUT" => put::<Value, _>(&client, &endpoint, &baseurl, &body, Some(&query_map), Some(headers)).await,
            "PATCH" => patch::<Value, _>(&client, &endpoint, &baseurl, &body, Some(&query_map), Some(headers)).await,
            "DELETE" => delete::<Value>(&client, &endpoint, &baseurl, Some(&query_map), Some(headers)).await,
            _ => unreachable!(),
        };

        match result {
            Ok(res) => {
                let pretty_body = serde_json::to_string_pretty(&res.body)
                    .unwrap_or_else(|_| res.raw_text.clone());

                self.response = format!(
                    "STATUS\n{:#?}\n\nRESPONSE TIME\n{:?}\n\nFINAL URL\n{}\n\nCONTENT LENGTH\n{:?}\n\nHTTP VERSION\n{:?}\n\nHEADERS\n{:#?}\n\nRAW TEXT\n{}\n\nBODY\n{}\n",
                    res.status,
                    res.response_time,
                    res.final_url,
                    res.content_length,
                    res.http_version,
                    res.headers,
                    res.raw_text,
                    pretty_body,
                );
                self.status = format!("SUCCESS {}", res.status);
                self.response_scroll = 0;
            }
            Err(e) => {
                self.response = format!("REQUEST FAILED\n\n{:#?}", e);
                self.status = "ERROR".into();
            }
        }
    }
}

// =========================================
// METHOD COLOURS
// =========================================

fn method_color(method: &str) -> Color {
    match method {
        "GET"    => Color::Green,
        "POST"   => Color::Blue,
        "PUT"    => Color::Yellow,
        "PATCH"  => Color::Cyan,
        "DELETE" => Color::Red,
        _        => Color::White,
    }
}

// =========================================
// UI
// =========================================

fn ui(frame: &mut ratatui::Frame, app: &App) {

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(3)])
        .split(frame.area());

    // MAIN (left + right)
    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(50)])
        .split(root[0]);

    // LEFT PANEL
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(10), Constraint::Min(10)])
        .split(main[0]);

    // ---- METHODS ----
    let method_items = app
        .methods
        .iter()
        .enumerate()
        .map(|(i, method)| {
            let color = method_color(method);
            let style = if i == app.selected_method {
                Style::default().fg(Color::Black).bg(color).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(color)
            };
            ListItem::new(*method).style(style)
        })
        .collect::<Vec<_>>();

    let methods = List::new(method_items).block(
        Block::default().title("Methods (↑↓)").borders(Borders::ALL),
    );
    frame.render_widget(methods, left[0]);

    // ---- EDITOR (tabs + content) ----
    let editor_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(left[1]);

    let titles = ["Query", "Header", "Body"]
        .iter()
        .map(|t| Line::from(*t))
        .collect::<Vec<_>>();

    let selected_tab = match app.active_tab {
        ActiveTab::Query  => 0,
        ActiveTab::Header => 1,
        ActiveTab::Body   => 2,
    };

    let editor_focused = app.mode == InputMode::Insert && app.focus_area == FocusArea::Editor;
    let tab_border_style = if editor_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let tabs = Tabs::new(titles)
        .select(selected_tab)
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Yellow))
        .divider("|")
        .block(
            Block::default()
                .title("Request (Tab=switch)")
                .borders(Borders::ALL)
                .border_style(tab_border_style),
        );
    frame.render_widget(tabs, editor_layout[0]);

    // Editor content
    let editor_content: Vec<Line> = match app.active_tab {
        ActiveTab::Query | ActiveTab::Header => {
            let list = if app.active_tab == ActiveTab::Query {
                &app.query_params
            } else {
                &app.headers
            };

            list.iter()
                .enumerate()
                .map(|(i, kv)| {
                    let is_selected = editor_focused && i == app.selected_pair;

                    let num_span = Span::styled(
                        format!("{}. ", i + 1),
                        Style::default().fg(Color::DarkGray),
                    );

                    let key_style = if is_selected && app.kv_focus == KvFocus::Key {
                        Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
                    } else if is_selected {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::White)
                    };

                    let val_style = if is_selected && app.kv_focus == KvFocus::Value {
                        Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
                    } else if is_selected {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default().fg(Color::Gray)
                    };

                    // Append a block cursor indicator when actively editing
                    let key_text = if is_selected && app.kv_focus == KvFocus::Key {
                        format!("{}_", kv.key)
                    } else {
                        kv.key.clone()
                    };
                    let val_text = if is_selected && app.kv_focus == KvFocus::Value {
                        format!("{}_", kv.value)
                    } else {
                        kv.value.clone()
                    };

                    Line::from(vec![
                        num_span,
                        Span::styled(key_text, key_style),
                        Span::styled(" = ", Style::default().fg(Color::DarkGray)),
                        Span::styled(val_text, val_style),
                    ])
                })
                .collect()
        }

        ActiveTab::Body => {
            // Show body with a trailing cursor when focused
            let display = if editor_focused {
                format!("{}_", app.body)
            } else {
                app.body.clone()
            };
            display
                .lines()
                .map(|l| Line::from(Span::styled(l.to_string(), Style::default().fg(Color::White))))
                .collect()
        }
    };

    let editor = Paragraph::new(editor_content)
        .block(
            Block::default()
                .title(match app.active_tab {
                    ActiveTab::Query  => "Editor — ↑↓=row  →←=key/val",
                    ActiveTab::Header => "Editor — ↑↓=row  →←=key/val",
                    ActiveTab::Body   => "Editor — JSON body",
                })
                .borders(Borders::ALL)
                .border_style(tab_border_style),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(editor, editor_layout[1]);

    // ---- RIGHT PANEL ----
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(10)])
        .split(main[1]);

    // ---- URL ----
    let url_focused = app.mode == InputMode::Insert && app.focus_area == FocusArea::Url;
    let url_border = if url_focused {
        Style::default().fg(Color::LightYellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    // Show cursor in URL bar when focused
    let url_display = if url_focused {
        format!("{}_", app.url)
    } else {
        app.url.clone()
    };

    let url = Paragraph::new(url_display)
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .title("URL (F2=focus)")
                .borders(Borders::ALL)
                .border_style(url_border),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(url, right[0]);

    // ---- RESPONSE ----
    let response = Paragraph::new(app.response.as_str())
        .style(Style::default().fg(Color::Green))
        .block(
            Block::default()
                .title("Response (j/k to scroll in Normal)")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .scroll((app.response_scroll, 0))
        .wrap(Wrap { trim: false });

    frame.render_widget(response, right[1]);

    // ---- FOOTER ----
    let mode_label = match app.mode {
        InputMode::Insert => "INSERT",
        InputMode::Normal => "NORMAL",
    };

    let focus_label = match app.focus_area {
        FocusArea::Url    => "URL",
        FocusArea::Editor => "EDITOR",
    };

    let hint = match app.mode {
        InputMode::Insert => match app.focus_area {
            FocusArea::Url    => "Esc=normal | F2=editor | Enter=send | Backspace=del",
            FocusArea::Editor => match app.active_tab {
                ActiveTab::Body          => "Esc=normal | Tab=switch tab | F2=url | Enter=send | Backspace=del",
                _                        => "Esc=normal | Tab=switch tab | F2=url | ↑↓=row | →←=key/val | Enter=send",
            },
        },
        InputMode::Normal => "i=insert | q=quit | j/k=scroll response | ↑↓=method",
    };

    let footer = Paragraph::new(format!("MODE:{} FOCUS:{} | {}", mode_label, focus_label, hint))
        .style(Style::default().fg(Color::Magenta))
        .block(
            Block::default()
                .title(app.status.as_str())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta)),
        );

    frame.render_widget(footer, root[1]);
}

// ========================================= 
// MAIN
// =========================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let conn = Connection::open("data.db")?;
    let db_status = check_database(&conn);
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::default();

    loop {
        terminal.draw(|f| ui(f, &app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match app.mode {
                    // ---- INSERT MODE ----
                    InputMode::Insert => match key.code {
                        KeyCode::Esc => {
                            app.mode = InputMode::Normal;
                        }

                        // Switch focus between URL and Editor
                        KeyCode::F(2) => {
                            app.toggle_focus_area();
                        }

                        // Switch editor tab (only when editor focused)
                        KeyCode::Tab => {
                            if app.focus_area == FocusArea::Editor {
                                app.next_tab();
                            }
                        }

                        KeyCode::Enter => {
                            app.send_request().await;
                        }

                        // ---- URL focused ----
                        KeyCode::Backspace if app.focus_area == FocusArea::Url => {
                            app.url_backspace();
                        }
                        KeyCode::Char(c) if app.focus_area == FocusArea::Url => {
                            app.url_push(c);
                        }

                        // ---- Editor focused ----
                        KeyCode::Backspace if app.focus_area == FocusArea::Editor => {
                            match app.active_tab {
                                ActiveTab::Body => app.body_backspace(),
                                _               => app.kv_backspace(),
                            }
                        }

                        // Navigate rows (query / header)
                        KeyCode::Down if app.focus_area == FocusArea::Editor => {
                            if app.active_tab != ActiveTab::Body {
                                app.kv_next_row();
                            }
                        }
                        KeyCode::Up if app.focus_area == FocusArea::Editor => {
                            if app.active_tab != ActiveTab::Body {
                                app.kv_prev_row();
                            }
                        }

                        // Toggle key / value field
                        KeyCode::Right | KeyCode::Left if app.focus_area == FocusArea::Editor => {
                            if app.active_tab != ActiveTab::Body {
                                app.kv_toggle_field();
                            }
                        }

                        KeyCode::Char(c) if app.focus_area == FocusArea::Editor => {
                            match app.active_tab {
                                ActiveTab::Body => app.body_push_char(c),
                                _               => app.kv_push_char(c),
                            }
                        }

                        _ => {}
                    },

                    // ---- NORMAL MODE ----
                    InputMode::Normal => match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('i') => app.mode = InputMode::Insert,
                        KeyCode::Char('j') => app.scroll_down(),
                        KeyCode::Char('k') => app.scroll_up(),
                        KeyCode::Up        => app.prev_method(),
                        KeyCode::Down      => app.next_method(),
                        _ => {}
                    },
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    Ok(())
}