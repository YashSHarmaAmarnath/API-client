use std::{collections::HashMap, error::Error, io, time::Duration};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap},
};

use rusqlite::Connection;
use serde_json::Value;

mod db_utils;
use db_utils::{
    ApiRequest, Collection, check_database, db_create_collection, db_delete_collection,
    db_delete_request, db_insert_api_request, get_all_collections, get_requests_by_collection_id,
};
mod utils;
use reqwest::Client;
use reqwest::header::{ HeaderMap, HeaderName, HeaderValue};

use utils::{delete, get, patch, post, put};
// ══════════════════════════════════════════════════════════════════════════════
//  PALETTE
// ══════════════════════════════════════════════════════════════════════════════
const C_BORDER: Color = Color::Rgb(60, 60, 80);
const C_BORDER_ACTIVE: Color = Color::Rgb(120, 120, 200);
const C_ACCENT: Color = Color::Rgb(130, 100, 230);
const C_DIM: Color = Color::Rgb(90, 90, 110);
const C_FG: Color = Color::Rgb(210, 210, 220);
const C_SEL_BG: Color = Color::Rgb(45, 40, 70);
const C_SEL_FG: Color = Color::Rgb(200, 190, 255);

fn method_color(m: &str) -> Color {
    match m {
        "GET" => Color::Rgb(80, 200, 120),
        "POST" => Color::Rgb(90, 150, 255),
        "PUT" => Color::Rgb(255, 190, 50),
        "PATCH" => Color::Rgb(80, 210, 210),
        "DELETE" => Color::Rgb(255, 90, 90),
        _ => C_FG,
    }
}

fn method_short(m: &str) -> &'static str {
    match m {
        "GET" => "GET  ",
        "POST" => "POST ",
        "PUT" => "PUT  ",
        "PATCH" => "PATCH",
        "DELETE" => "DEL  ",
        _ => "?????",
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  ENUMS
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, PartialEq, Debug)]
enum Panel {
    Collections,
    Requests,
    Response,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum EditorTab {
    Url,
    Headers,
    Query,
    Body,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum KvField {
    Key,
    Value,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum InputMode {
    /// Navigating panels / lists
    Normal,
    /// Editing a text field
    Insert,
}

/// Which overlay/dialog is open
#[derive(Clone, PartialEq, Debug)]
enum Overlay {
    None,
    NewCollection {
        name: String,
        desc: String,
        field: u8,
    },
    NewRequest {
        name: String,
        url: String,
        method_idx: usize,
        field: u8,
    },
    ConfirmDelete {
        target: DeleteTarget,
    },
    Help,
}

#[derive(Clone, PartialEq, Debug)]
enum DeleteTarget {
    Collection(i64),
    Request(i64),
}

// ══════════════════════════════════════════════════════════════════════════════
//  DATA
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Default, Clone)]
struct KvRow {
    key: String,
    value: String,
}

// ══════════════════════════════════════════════════════════════════════════════
//  APP STATE
// ══════════════════════════════════════════════════════════════════════════════

struct App {
    // ── database ─────────────────────────────────────────────────────────────
    conn: Connection,

    // ── panels ──────────────────────────────────────────────────────────────
    active_panel: Panel,
    input_mode: InputMode,
    overlay: Overlay,

    // ── collections ─────────────────────────────────────────────────────────
    collections: Vec<Collection>,
    coll_state: ListState,

    // ── requests ────────────────────────────────────────────────────────────
    requests: Vec<ApiRequest>,
    req_state: ListState,

    // ── request editor (bottom pane) ────────────────────────────────────────
    editor_open: bool,
    editor_tab: EditorTab,
    /// index into METHODS
    method_idx: usize,

    url: String,

    query_params: Vec<KvRow>,
    query_row: usize,
    query_field: KvField,

    headers: Vec<KvRow>,
    header_row: usize,
    header_field: KvField,

    body: String,

    // ── response ─────────────────────────────────────────────────────────────
    response_text: String,
    response_scroll: u16,
    status_line: String,
}

const METHODS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE"];

impl App {
    fn new(conn: Connection) -> Self {
        // Load all collections from DB
        let collections = get_all_collections(&conn).unwrap_or_default();

        // Load every request for every collection
        let mut requests: Vec<ApiRequest> = Vec::new();
        for c in &collections {
            if let Ok(mut reqs) = get_requests_by_collection_id(&conn, c.collection_id) {
                requests.append(&mut reqs);
            }
        }

        let mut coll_state = ListState::default();
        if !collections.is_empty() {
            coll_state.select(Some(0));
        }
        let mut req_state = ListState::default();
        if !requests.is_empty() {
            req_state.select(Some(0));
        }

        Self {
            conn,
            active_panel: Panel::Collections,
            input_mode: InputMode::Normal,
            overlay: Overlay::None,
            collections,
            coll_state,
            requests,
            req_state,
            editor_open: false,
            editor_tab: EditorTab::Url,
            method_idx: 0,
            url: String::new(),

            query_params: vec![KvRow::default()],
            query_row: 0,
            query_field: KvField::Key,

            headers: vec![KvRow::default()],
            header_row: 0,
            header_field: KvField::Key,
            body: "{}".into(),
            response_text: "Press Enter on a request, or 's' to send — response appears here."
                .into(),
            response_scroll: 0,
            status_line: "Ready".into(),
        }
    }

    // ── collection helpers ───────────────────────────────────────────────────

    fn selected_collection(&self) -> Option<&Collection> {
        self.coll_state
            .selected()
            .and_then(|i| self.collections.get(i))
    }

    fn selected_collection_id(&self) -> Option<i64> {
        self.selected_collection().map(|c| c.collection_id)
    }

    fn requests_for_selected(&self) -> Vec<&ApiRequest> {
        match self.selected_collection_id() {
            Some(id) => self
                .requests
                .iter()
                .filter(|r| r.collection_id == id)
                .collect(),
            None => vec![],
        }
    }

    fn coll_next(&mut self) {
        let len = self.collections.len();
        if len == 0 {
            return;
        }
        let i = self
            .coll_state
            .selected()
            .map(|i| (i + 1) % len)
            .unwrap_or(0);
        self.coll_state.select(Some(i));
        self.req_state.select(Some(0));
    }

    fn coll_prev(&mut self) {
        let len = self.collections.len();
        if len == 0 {
            return;
        }
        let i = self
            .coll_state
            .selected()
            .map(|i| if i == 0 { len - 1 } else { i - 1 })
            .unwrap_or(0);
        self.coll_state.select(Some(i));
        self.req_state.select(Some(0));
    }

    fn add_collection(&mut self, name: String, desc: String) {
        if let Err(e) = db_create_collection(&self.conn, &name, &desc) {
            self.status_line = format!("DB error: {}", e);
            return;
        }
        // Reload from DB so we get the real autoincrement ID
        self.reload_collections();
        self.coll_state
            .select(Some(self.collections.len().saturating_sub(1)));
        self.status_line = format!("Collection '{}' created", name);
    }

    fn reload_collections(&mut self) {
        self.collections = get_all_collections(&self.conn).unwrap_or_default();
    }

    fn reload_requests(&mut self) {
        self.requests.clear();
        for c in &self.collections {
            if let Ok(mut reqs) = get_requests_by_collection_id(&self.conn, c.collection_id) {
                self.requests.append(&mut reqs);
            }
        }
    }

    fn delete_selected_collection(&mut self) {
        if let Some(i) = self.coll_state.selected() {
            let id = self.collections[i].collection_id;
            if let Err(e) = db_delete_collection(&self.conn, id) {
                self.status_line = format!("DB error: {}", e);
                return;
            }
            // Reload both from DB (cascade delete handles requests)
            self.reload_collections();
            self.reload_requests();
            let next = i.min(self.collections.len().saturating_sub(1));
            self.coll_state.select(if self.collections.is_empty() {
                None
            } else {
                Some(next)
            });
            self.req_state.select(Some(0));
            self.status_line = "Collection deleted".into();
        }
    }

    // ── request helpers ──────────────────────────────────────────────────────

    fn req_next(&mut self) {
        let visible: Vec<_> = self.requests_for_selected();
        if visible.is_empty() {
            return;
        }
        let i = self
            .req_state
            .selected()
            .map(|i| (i + 1) % visible.len())
            .unwrap_or(0);
        self.req_state.select(Some(i));
    }

    fn req_prev(&mut self) {
        let visible: Vec<_> = self.requests_for_selected();
        if visible.is_empty() {
            return;
        }
        let i = self
            .req_state
            .selected()
            .map(|i| if i == 0 { visible.len() - 1 } else { i - 1 })
            .unwrap_or(0);
        self.req_state.select(Some(i));
    }

    fn add_request(&mut self, name: String, url: String, method_idx: usize) {
        if let Some(cid) = self.selected_collection_id() {
            let method = METHODS[method_idx];
            if let Err(e) = db_insert_api_request(&self.conn, cid, &name, method, &url, None, None)
            {
                self.status_line = format!("DB error: {}", e);
                return;
            }
            self.reload_requests();
            let visible_count = self.requests_for_selected().len();
            self.req_state.select(Some(visible_count.saturating_sub(1)));
            self.status_line = format!("Request '{}' created", name);
        }
    }

    fn delete_selected_request(&mut self) {
        if let Some(sel_i) = self.req_state.selected() {
            let visible_ids: Vec<i64> = self
                .requests_for_selected()
                .iter()
                .map(|r| r.request_id)
                .collect();
            if let Some(&rid) = visible_ids.get(sel_i) {
                if let Err(e) = db_delete_request(&self.conn, rid) {
                    self.status_line = format!("DB error: {}", e);
                    return;
                }
                self.reload_requests();
                let new_len = self.requests_for_selected().len();
                self.req_state.select(if new_len == 0 {
                    None
                } else {
                    Some(sel_i.min(new_len - 1))
                });
                self.status_line = "Request deleted".into();
            }
        }
    }

    fn load_request_into_editor(&mut self) {
        // Clone the data we need out of self before any mutation
        let data: Option<(
            String,
            String,
            Option<HashMap<String, String>>,
            Option<serde_json::Value>,
        )> = {
            let visible: Vec<&ApiRequest> = self.requests_for_selected();
            self.req_state
                .selected()
                .and_then(|i| visible.get(i))
                .map(|req| {
                    (
                        req.method.clone(),
                        req.url.clone(),
                        req.headers.clone(),
                        req.body.clone(),
                    )
                })
        };

        if let Some((method, url, headers, body)) = data {
            self.method_idx = METHODS.iter().position(|&m| m == method).unwrap_or(0);
            self.url = url;
            self.headers = headers
                .map(|h| {
                    h.iter()
                        .map(|(k, v)| KvRow {
                            key: k.clone(),
                            value: v.clone(),
                        })
                        .collect()
                })
                .unwrap_or_else(|| vec![KvRow::default()]);
            self.body = body
                .map(|b| serde_json::to_string_pretty(&b).unwrap_or_default())
                .unwrap_or_else(|| "{}".into());
            self.query_params = vec![KvRow::default()];
            self.editor_open = true;
            self.editor_tab = EditorTab::Url;
        }
    }
    fn build_query_map(&self) -> HashMap<String, String> {
        self.query_params
            .iter()
            .filter(|q| !q.key.is_empty())
            .map(|q| (q.key.clone(), q.value.clone()))
            .collect()
    }
    fn build_header_map(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();

        for h in &self.headers {
            if h.key.is_empty() {
                continue;
            }

            if let (Ok(name), Ok(value)) = (
                HeaderName::from_bytes(h.key.as_bytes()),
                HeaderValue::from_str(&h.value),
            ) {
                headers.insert(name, value);
            }
        }

        headers
    }
    async fn send_request(&mut self) {
        // TODO: wire to actual HTTP via utils module
        // println!("{}",METHODS[self.method_idx]);
        let client = Client::new();
        let query = self.build_query_map();
        let header = self.build_header_map();
        let result = match self.method_idx {
            0 => get::<Value>(&client, &self.url, Some(&query), Some(&header)).await,
            1 => {
                post::<Value, _>(
                    &client,
                    &self.url,
                    &self.body,
                    Some(&query),
                    Some(header.clone()),
                )
                .await
            }
            2 => {
                put::<Value, _>(
                    &client,
                    &self.url,
                    &self.body,
                    Some(&query),
                    Some(header.clone()),
                )
                .await
            }
            3 => {
                patch::<Value, _>(
                    &client,
                    &self.url,
                    &self.body,
                    Some(&query),
                    Some(header.clone()),
                )
                .await
            }
            4 => delete::<Value>(&client, &self.url, Some(&query), Some(header.clone())).await,
            _ => unreachable!("Unreachable method index!"),
        };
        match result {
            Ok(res) => {
                let pretty_body = serde_json::to_string_pretty(&res.body)
                    .unwrap_or_else(|_| res.raw_text.clone());

                self.response_text = format!(
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
                // self.status = format!("SUCCESS {}", res.status);
                self.response_scroll = 0;
                self.status_line = format!("Sent {} {}", METHODS[self.method_idx], self.url);
            }
            Err(e) => {
                self.response_text = format!("REQUEST FAILED\n\n{:#?}", e);
                self.status_line = format!("ERROR: {} failed", METHODS[self.method_idx]);
                // self.status = "ERROR".into();
            }
        }
        self.response_scroll = 0;
    }

    // ── editor helpers ───────────────────────────────────────────────────────

    fn editor_backspace(&mut self) {
        match self.editor_tab {
            EditorTab::Url => {
                self.url.pop();
            }
            EditorTab::Body => {
                self.body.pop();
            }
            EditorTab::Query => {
                if let Some(row) = self.query_params.get_mut(self.query_row) {
                    match self.query_field {
                        KvField::Key => row.key.pop(),
                        KvField::Value => row.value.pop(),
                    };
                }
            }
            EditorTab::Headers => {
                if let Some(row) = self.headers.get_mut(self.header_row) {
                    match self.header_field {
                        KvField::Key => {
                            row.key.pop();
                        }
                        KvField::Value => {
                            row.value.pop();
                        }
                    }
                }
            }
        }
    }

    fn editor_push(&mut self, c: char) {
        match self.editor_tab {
            EditorTab::Url => self.url.push(c),
            EditorTab::Body => self.body.push(c),
            EditorTab::Query => {
                if let Some(row) = self.query_params.get_mut(self.query_row) {
                    match self.query_field {
                        KvField::Key => row.key.push(c),
                        KvField::Value => row.value.push(c),
                    }
                }
            }
            EditorTab::Headers => {
                if let Some(row) = self.headers.get_mut(self.header_row) {
                    match self.header_field {
                        KvField::Key => row.key.push(c),
                        KvField::Value => row.value.push(c),
                    }
                }
            }
        }
    }

    fn header_next_row(&mut self) {
        if self.header_row + 1 < self.headers.len() {
            self.header_row += 1;
        } else {
            self.headers.push(KvRow::default());
            self.header_row += 1;
        }
        self.header_field = KvField::Key;
    }

    fn header_prev_row(&mut self) {
        if self.header_row > 0 {
            self.header_row -= 1;
        }
        self.header_field = KvField::Key;
    }

    fn query_next_row(&mut self) {
        if self.query_row + 1 < self.query_params.len() {
            self.query_row += 1;
        } else {
            self.query_params.push(KvRow::default());
            self.query_row += 1;
        }
        self.query_field = KvField::Key;
    }

    fn query_prev_row(&mut self) {
        if self.query_row > 0 {
            self.query_row -= 1;
        }
        self.query_field = KvField::Key;
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  DRAW
// ══════════════════════════════════════════════════════════════════════════════

fn draw(f: &mut Frame, app: &App) {
    let full = f.area();

    // ── outer vertical split: top area + footer ──────────────────────────────
    let vert = Layout::vertical([
        Constraint::Min(10),
        Constraint::Length(if app.editor_open { 14 } else { 0 }),
        Constraint::Length(1),
    ])
    .split(full);

    let top_area = vert[0];
    let editor_area = vert[1];
    let footer_area = vert[2];

    // ── top: three columns ───────────────────────────────────────────────────
    let cols = Layout::horizontal([
        Constraint::Length(28),
        Constraint::Length(34),
        Constraint::Min(30),
    ])
    .split(top_area);

    draw_collections(f, app, cols[0]);
    draw_requests(f, app, cols[1]);
    draw_response(f, app, cols[2]);

    // ── editor ───────────────────────────────────────────────────────────────
    if app.editor_open {
        draw_editor(f, app, editor_area);
    }

    // ── footer ───────────────────────────────────────────────────────────────
    draw_footer(f, app, footer_area);

    // ── overlays (on top of everything) ──────────────────────────────────────
    match &app.overlay {
        Overlay::None => {}
        Overlay::NewCollection { name, desc, field } => {
            draw_new_collection_dialog(f, full, name, desc, *field);
        }
        Overlay::NewRequest {
            name,
            url,
            method_idx,
            field,
        } => {
            draw_new_request_dialog(f, full, name, url, *method_idx, *field);
        }
        Overlay::ConfirmDelete { target } => {
            draw_confirm_delete(f, full, target);
        }
        Overlay::Help => {
            draw_help(f, full);
        }
    }
}

// ── collections panel ────────────────────────────────────────────────────────

fn draw_collections(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel == Panel::Collections && app.overlay == Overlay::None;
    let border_style = Style::default().fg(if active { C_BORDER_ACTIVE } else { C_BORDER });

    let items: Vec<ListItem> = app
        .collections
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let selected = app.coll_state.selected() == Some(i);
            let icon = if selected { "▶ " } else { "  " };
            let name_style = if selected {
                Style::default()
                    .fg(C_SEL_FG)
                    .bg(C_SEL_BG)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(C_FG)
            };
            let desc = c.description.as_deref().unwrap_or("—");
            let desc_style = Style::default().fg(C_DIM);

            ListItem::new(vec![
                Line::from(vec![
                    Span::raw(icon),
                    Span::styled(c.collection_name.as_str(), name_style),
                ]),
                Line::from(vec![Span::raw("   "), Span::styled(desc, desc_style)]),
            ])
        })
        .collect();

    let title = format!(" Collections ({}) ", app.collections.len());
    let list = List::new(items).block(
        Block::default()
            .title(title)
            .title_style(Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(border_style),
    );
    f.render_stateful_widget(list, area, &mut app.coll_state.clone());

    // hint at bottom-right of the block
    let hint = Span::styled(" n=new  d=del ", Style::default().fg(C_DIM));
    let hint_area = Rect {
        x: area.x + area.width.saturating_sub(14),
        y: area.y + area.height.saturating_sub(1),
        width: 14,
        height: 1,
    };
    f.render_widget(Paragraph::new(hint), hint_area);
}

// ── requests panel ───────────────────────────────────────────────────────────

fn draw_requests(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel == Panel::Requests && app.overlay == Overlay::None;
    let border_style = Style::default().fg(if active { C_BORDER_ACTIVE } else { C_BORDER });

    let visible: Vec<&ApiRequest> = app.requests_for_selected();

    let coll_name = app
        .selected_collection()
        .map(|c| c.collection_name.as_str())
        .unwrap_or("—");

    let items: Vec<ListItem> = visible
        .iter()
        .enumerate()
        .map(|(i, req)| {
            let selected = app.req_state.selected() == Some(i);
            let icon = if selected { "▶ " } else { "  " };
            let name_style = if selected {
                Style::default()
                    .fg(C_SEL_FG)
                    .bg(C_SEL_BG)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(C_FG)
            };
            let mc = method_color(&req.method);
            let ms = method_short(&req.method);

            ListItem::new(vec![
                Line::from(vec![
                    Span::raw(icon),
                    Span::styled(ms, Style::default().fg(mc).add_modifier(Modifier::BOLD)),
                    Span::raw(" "),
                    Span::styled(req.request_name.as_str(), name_style),
                ]),
                Line::from(vec![
                    Span::raw("        "),
                    Span::styled(truncate(&req.url, 24), Style::default().fg(C_DIM)),
                ]),
            ])
        })
        .collect();

    let title = format!(" {} ({}) ", coll_name, visible.len());
    let list = List::new(items).block(
        Block::default()
            .title(title)
            .title_style(Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(border_style),
    );
    f.render_stateful_widget(list, area, &mut app.req_state.clone());

    let hint = Span::styled(" n=new  d=del  e=edit ", Style::default().fg(C_DIM));
    let w = 22u16;
    let hint_area = Rect {
        x: area.x + area.width.saturating_sub(w),
        y: area.y + area.height.saturating_sub(1),
        width: w,
        height: 1,
    };
    f.render_widget(Paragraph::new(hint), hint_area);
}

// ── response panel ───────────────────────────────────────────────────────────

fn draw_response(f: &mut Frame, app: &App, area: Rect) {
    let active = app.active_panel == Panel::Response && app.overlay == Overlay::None;
    let border_style = Style::default().fg(if active { C_BORDER_ACTIVE } else { C_BORDER });

    let status_color = if app.status_line.contains("ERROR") {
        Color::Rgb(255, 90, 90)
    } else if app.status_line.starts_with("Sent") {
        Color::Rgb(80, 200, 120)
    } else {
        C_DIM
    };

    let p = Paragraph::new(app.response_text.as_str())
        .style(Style::default().fg(C_FG))
        .block(
            Block::default()
                .title(format!(" Response — {} ", app.status_line))
                .title_style(
                    Style::default()
                        .fg(status_color)
                        .add_modifier(Modifier::BOLD),
                )
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.response_scroll, 0));

    f.render_widget(p, area);
}

// ── editor pane ──────────────────────────────────────────────────────────────

fn draw_editor(f: &mut Frame, app: &App, area: Rect) {
    let border_style = Style::default().fg(if app.input_mode == InputMode::Insert {
        C_BORDER_ACTIVE
    } else {
        C_BORDER
    });

    // outer block
    let outer = Block::default()
        .title(
            " Request Editor  [i=insert  Esc=normal  s=send  Tab=next-field  Ctrl-M=cycle method] ",
        )
        .title_style(Style::default().fg(C_ACCENT))
        .borders(Borders::ALL)
        .border_style(border_style);
    f.render_widget(outer.clone(), area);

    let inner = outer.inner(area);

    // layout: method+url bar on top, tabs row, content
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(5),
    ])
    .split(inner);

    // ── method + url bar ─────────────────────────────────────────────────────
    let method = METHODS[app.method_idx];
    let mc = method_color(method);
    let cursor = if app.editor_tab == EditorTab::Url && app.input_mode == InputMode::Insert {
        "_"
    } else {
        ""
    };
    let url_display = format!("{}{}", app.url, cursor);

    let method_url = Line::from(vec![
        Span::styled(
            format!(" {} ", method),
            Style::default()
                .fg(Color::Black)
                .bg(mc)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(url_display, Style::default().fg(Color::Rgb(255, 220, 100))),
    ]);
    f.render_widget(Paragraph::new(method_url), rows[0]);

    // ── tab bar ──────────────────────────────────────────────────────────────
    let tab_titles = vec![
        Line::from("URL"),
        Line::from("Query"),
        Line::from("Headers"),
        Line::from("Body"),
    ];
    let selected_tab = match app.editor_tab {
        EditorTab::Url => 0,
        EditorTab::Query => 1,
        EditorTab::Headers => 2,
        EditorTab::Body => 3,
    };
    let tabs = Tabs::new(tab_titles)
        .select(selected_tab)
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(C_ACCENT)
                .add_modifier(Modifier::BOLD),
        )
        .divider("│")
        .style(Style::default().fg(C_DIM));
    f.render_widget(tabs, rows[1]);

    // ── tab content ──────────────────────────────────────────────────────────
    match app.editor_tab {
        EditorTab::Url => {
            let text = vec![
                Line::from(vec![
                    Span::styled("  URL  ", Style::default().fg(C_DIM)),
                    Span::styled(
                        format!(
                            "{}{}",
                            app.url,
                            if app.input_mode == InputMode::Insert {
                                "_"
                            } else {
                                ""
                            }
                        ),
                        Style::default().fg(Color::Rgb(255, 220, 100)),
                    ),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "  Tip: Tab → Headers, Ctrl-M → cycle method, Enter on request to load it",
                    Style::default().fg(C_DIM),
                )),
            ];
            f.render_widget(Paragraph::new(text), rows[2]);
        }

        EditorTab::Query => {
            let lines: Vec<Line> = app
                .query_params
                .iter()
                .enumerate()
                .map(|(i, row)| {
                    let sel = i == app.query_row && app.input_mode == InputMode::Insert;

                    let key_style = if sel && app.query_field == KvField::Key {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Rgb(255, 220, 100))
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(C_FG)
                    };

                    let val_style = if sel && app.query_field == KvField::Value {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Rgb(100, 200, 255))
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Rgb(130, 190, 230))
                    };

                    let k_cur = if sel && app.query_field == KvField::Key {
                        "_"
                    } else {
                        ""
                    };
                    let v_cur = if sel && app.query_field == KvField::Value {
                        "_"
                    } else {
                        ""
                    };

                    Line::from(vec![
                        Span::styled(format!("  {:2}. ", i + 1), Style::default().fg(C_DIM)),
                        Span::styled(format!("{}{}", row.key, k_cur), key_style),
                        Span::styled("  =  ", Style::default().fg(C_DIM)),
                        Span::styled(format!("{}{}", row.value, v_cur), val_style),
                    ])
                })
                .collect();

            f.render_widget(Paragraph::new(lines), rows[2]);
        }

        EditorTab::Headers => {
            let lines: Vec<Line> = app
                .headers
                .iter()
                .enumerate()
                .map(|(i, row)| {
                    let sel = i == app.header_row && app.input_mode == InputMode::Insert;
                    let key_style = if sel && app.header_field == KvField::Key {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Rgb(255, 220, 100))
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(C_FG)
                    };
                    let val_style = if sel && app.header_field == KvField::Value {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Rgb(100, 200, 255))
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Rgb(130, 190, 230))
                    };
                    let k_cur = if sel && app.header_field == KvField::Key {
                        "_"
                    } else {
                        ""
                    };
                    let v_cur = if sel && app.header_field == KvField::Value {
                        "_"
                    } else {
                        ""
                    };
                    Line::from(vec![
                        Span::styled(format!("  {:2}. ", i + 1), Style::default().fg(C_DIM)),
                        Span::styled(format!("{}{}", row.key, k_cur), key_style),
                        Span::styled("  :  ", Style::default().fg(C_DIM)),
                        Span::styled(format!("{}{}", row.value, v_cur), val_style),
                    ])
                })
                .collect();
            f.render_widget(Paragraph::new(lines), rows[2]);
        }

        EditorTab::Body => {
            let display = format!(
                "{}{}",
                app.body,
                if app.input_mode == InputMode::Insert {
                    "_"
                } else {
                    ""
                }
            );
            let lines: Vec<Line> = display
                .lines()
                .map(|l| Line::from(Span::styled(l.to_string(), Style::default().fg(C_FG))))
                .collect();
            f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), rows[2]);
        }
    }
}

// ── footer ───────────────────────────────────────────────────────────────────

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let (mode_str, mode_color) = match app.input_mode {
        InputMode::Normal => (" NORMAL ", C_ACCENT),
        InputMode::Insert => (" INSERT ", Color::Rgb(80, 200, 120)),
    };

    let panel_str = match app.active_panel {
        Panel::Collections => "COLLECTIONS",
        Panel::Requests => "REQUESTS",
        Panel::Response => "RESPONSE",
    };

    let hint = match app.overlay {
        Overlay::None => match app.active_panel {
            Panel::Collections => "Tab=next panel  n=new  d=delete  j/k=nav  ?=help  q=quit",
            Panel::Requests => {
                "Tab=next panel  n=new  d=delete  e=edit  Enter=open editor  s=send  j/k=nav"
            }
            Panel::Response => "Tab=next panel  j/k=scroll  q=quit",
        },
        _ => "Enter=confirm  Esc=cancel",
    };

    let line = Line::from(vec![
        Span::styled(
            mode_str,
            Style::default()
                .fg(Color::Black)
                .bg(mode_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(format!(" {} ", panel_str), Style::default().fg(mode_color)),
        Span::styled(" │ ", Style::default().fg(C_DIM)),
        Span::styled(hint, Style::default().fg(C_DIM)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

// ── overlay: new collection ───────────────────────────────────────────────────

fn draw_new_collection_dialog(f: &mut Frame, area: Rect, name: &str, desc: &str, field: u8) {
    let dialog = centered_rect(50, 11, area);
    f.render_widget(Clear, dialog);

    let block = Block::default()
        .title(" ✦ New Collection ")
        .title_style(Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(C_BORDER_ACTIVE));
    f.render_widget(block.clone(), dialog);
    let inner = block.inner(dialog);

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  Name  : ", Style::default().fg(C_DIM)),
            Span::styled(
                format!("{}{}", name, if field == 0 { "_" } else { "" }),
                Style::default().fg(if field == 0 {
                    Color::Rgb(255, 220, 100)
                } else {
                    C_FG
                }),
            ),
        ])),
        rows[1],
    );

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  Desc  : ", Style::default().fg(C_DIM)),
            Span::styled(
                format!("{}{}", desc, if field == 1 { "_" } else { "" }),
                Style::default().fg(if field == 1 {
                    Color::Rgb(255, 220, 100)
                } else {
                    C_FG
                }),
            ),
        ])),
        rows[3],
    );

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "  Tab=next field   Enter=create   Esc=cancel",
            Style::default().fg(C_DIM),
        ))),
        rows[5],
    );
}

// ── overlay: new request ──────────────────────────────────────────────────────
fn draw_new_request_dialog(
    f: &mut Frame,
    area: Rect,
    name: &str,
    url: &str,
    method_idx: usize,
    field: u8,
) {
    let dialog = centered_rect(60, 13, area);
    f.render_widget(Clear, dialog);

    let block = Block::default()
        .title(" ✦ New Request ")
        .title_style(Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(C_BORDER_ACTIVE));
    f.render_widget(block.clone(), dialog);
    let inner = block.inner(dialog);

    let rows = Layout::vertical([
        Constraint::Length(1), // padding
        Constraint::Length(1), // name
        Constraint::Length(1), // padding
        Constraint::Length(1), // method
        Constraint::Length(1), // padding
        Constraint::Length(1), // url
        Constraint::Length(1), // padding
        Constraint::Length(1), // hint
        Constraint::Length(1), // padding
    ])
    .split(inner);

    let active_color = Color::Rgb(255, 220, 100);
    let inactive_color = C_FG;

    // name field
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  Name   : ", Style::default().fg(C_DIM)),
            Span::styled(
                format!("{}{}", name, if field == 0 { "_" } else { "" }),
                Style::default().fg(if field == 0 {
                    active_color
                } else {
                    inactive_color
                }),
            ),
        ])),
        rows[1],
    );

    // method field (cycle with Tab)
    let method = METHODS[method_idx];
    let mc = method_color(method);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  Method : ", Style::default().fg(C_DIM)),
            Span::styled(
                format!(" {} ", method),
                Style::default()
                    .fg(if field == 1 { Color::Black } else { mc })
                    .bg(if field == 1 { mc } else { Color::Reset })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                if field == 1 { "  ←/→ to cycle" } else { "" },
                Style::default().fg(C_DIM),
            ),
        ])),
        rows[3],
    );

    // url field
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("  URL    : ", Style::default().fg(C_DIM)),
            Span::styled(
                format!("{}{}", url, if field == 2 { "_" } else { "" }),
                Style::default().fg(if field == 2 {
                    active_color
                } else {
                    inactive_color
                }),
            ),
        ])),
        rows[5],
    );

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "  Tab=next field   ←/→=cycle method   Enter=create   Esc=cancel",
            Style::default().fg(C_DIM),
        ))),
        rows[7],
    );
}

// ── overlay: confirm delete ───────────────────────────────────────────────────

fn draw_confirm_delete(f: &mut Frame, area: Rect, target: &DeleteTarget) {
    let dialog = centered_rect(44, 9, area);
    f.render_widget(Clear, dialog);

    let label = match target {
        DeleteTarget::Collection(_) => "collection (and all its requests)",
        DeleteTarget::Request(_) => "this request",
    };

    let block = Block::default()
        .title(" ⚠  Confirm Delete ")
        .title_style(
            Style::default()
                .fg(Color::Rgb(255, 90, 90))
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(255, 90, 90)));
    f.render_widget(block.clone(), dialog);
    let inner = block.inner(dialog);

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("  Delete {}?", label),
            Style::default().fg(C_FG),
        ))),
        rows[1],
    );

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "  Enter=confirm   Esc=cancel",
            Style::default().fg(C_DIM),
        ))),
        rows[3],
    );
}

// ── overlay: help ─────────────────────────────────────────────────────────────

fn draw_help(f: &mut Frame, area: Rect) {
    let dialog = centered_rect(60, 24, area);
    f.render_widget(Clear, dialog);

    let block = Block::default()
        .title(" ✦ Keybindings ")
        .title_style(Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(C_BORDER_ACTIVE));
    f.render_widget(block.clone(), dialog);
    let inner = block.inner(dialog);

    let lines = vec![
        Line::from(""),
        kv_line("  Tab / Shift-Tab", "cycle panels"),
        kv_line("  j / k  or  ↑↓", "navigate list"),
        Line::from(""),
        kv_line("  n", "new collection / request (context-aware)"),
        kv_line("  d", "delete selected (with confirmation)"),
        kv_line("  e  or  Enter", "open request in editor"),
        kv_line("  s", "send the current request"),
        Line::from(""),
        kv_line("  Editor — i", "enter insert mode"),
        kv_line("  Editor — Esc", "leave insert mode"),
        kv_line("  Editor — Tab", "cycle URL → Headers → Body"),
        kv_line("  Editor — Ctrl-M", "cycle HTTP method"),
        kv_line("  Editor — ↑↓", "navigate header rows"),
        kv_line("  Editor — →", "toggle key / value field"),
        Line::from(""),
        kv_line("  ?", "toggle this help"),
        kv_line("  q", "quit"),
        Line::from(""),
        Line::from(Span::styled(
            "  Press any key to close",
            Style::default().fg(C_DIM),
        )),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

fn kv_line<'a>(key: &'a str, val: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{:<24}", key), Style::default().fg(C_ACCENT)),
        Span::styled(val, Style::default().fg(C_FG)),
    ])
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let w = area.width * percent_x / 100;
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect {
        x,
        y,
        width: w.min(area.width),
        height: height.min(area.height),
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}

// ══════════════════════════════════════════════════════════════════════════════
//  EVENT HANDLING
// ══════════════════════════════════════════════════════════════════════════════

async fn handle_event(app: &mut App, key: crossterm::event::KeyEvent) -> bool {
    use KeyCode::*;

    // ── overlays swallow all input ───────────────────────────────────────────
    match &mut app.overlay.clone() {
        Overlay::Help => {
            app.overlay = Overlay::None;
            return false;
        }

        Overlay::NewCollection { name, desc, field } => {
            let mut name = name.clone();
            let mut desc = desc.clone();
            let mut field = *field;
            match key.code {
                Esc => {
                    app.overlay = Overlay::None;
                }
                Tab => {
                    field = 1 - field;
                    app.overlay = Overlay::NewCollection { name, desc, field };
                }
                Enter => {
                    if !name.is_empty() {
                        app.add_collection(name, desc);
                    }
                    app.overlay = Overlay::None;
                }
                Backspace => {
                    if field == 0 {
                        name.pop();
                    } else {
                        desc.pop();
                    }
                    app.overlay = Overlay::NewCollection { name, desc, field };
                }
                Char(c) => {
                    if field == 0 {
                        name.push(c);
                    } else {
                        desc.push(c);
                    }
                    app.overlay = Overlay::NewCollection { name, desc, field };
                }
                _ => {}
            }
            return false;
        }

        Overlay::NewRequest {
            name,
            url,
            method_idx,
            field,
        } => {
            let (mut name, mut url, mut method_idx, mut field) =
                (name.clone(), url.clone(), *method_idx, *field);
            match key.code {
                Esc => {
                    app.overlay = Overlay::None;
                    return false;
                }
                Tab => {
                    field = (field + 1) % 3;
                }
                Enter => {
                    if !name.is_empty() {
                        app.add_request(name.clone(), url.clone(), method_idx);
                    }
                    app.overlay = Overlay::None;
                }
                Left if field == 1 => {
                    method_idx = if method_idx == 0 {
                        METHODS.len() - 1
                    } else {
                        method_idx - 1
                    };
                }
                Right if field == 1 => {
                    method_idx = (method_idx + 1) % METHODS.len();
                }
                Backspace => match field {
                    0 => {
                        name.pop();
                    }
                    2 => {
                        url.pop();
                    }
                    _ => {}
                },
                Char(c) => match field {
                    0 => name.push(c),
                    2 => url.push(c),
                    _ => {}
                },
                _ => {}
            }
            app.overlay = Overlay::NewRequest {
                name,
                url,
                method_idx,
                field,
            };
            return false;
        }

        Overlay::ConfirmDelete { target } => {
            let target = target.clone();
            match key.code {
                Enter => {
                    match target {
                        DeleteTarget::Collection(_) => app.delete_selected_collection(),
                        DeleteTarget::Request(_) => app.delete_selected_request(),
                    }
                    app.overlay = Overlay::None;
                }
                Esc | Char('n') => {
                    app.overlay = Overlay::None;
                }
                _ => {}
            }
            return false;
        }

        Overlay::None => {}
    }

    // ── insert mode in editor ────────────────────────────────────────────────
    if app.input_mode == InputMode::Insert && app.editor_open {
        match key.code {
            Esc => {
                app.input_mode = InputMode::Normal;
            }
            Tab => {
                app.editor_tab = match app.editor_tab {
                    EditorTab::Url => EditorTab::Query,
                    EditorTab::Query => EditorTab::Headers,
                    EditorTab::Headers => EditorTab::Body,
                    EditorTab::Body => EditorTab::Url,
                };
            }
            Char('m')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                app.method_idx = (app.method_idx + 1) % METHODS.len();
            }
            Backspace => app.editor_backspace(),
            Down => match app.editor_tab {
                EditorTab::Query => app.query_next_row(),
                EditorTab::Headers => app.header_next_row(),
                _ => {}
            },
            Up => match app.editor_tab {
                EditorTab::Query => app.query_prev_row(),
                EditorTab::Headers => app.header_prev_row(),
                _ => {}
            },
            Right => match app.editor_tab {
                EditorTab::Query => {
                    app.query_field = match app.query_field {
                        KvField::Key => KvField::Value,
                        KvField::Value => KvField::Key,
                    };
                }
                EditorTab::Headers => {
                    app.header_field = match app.header_field {
                        KvField::Key => KvField::Value,
                        KvField::Value => KvField::Key,
                    };
                }
                _ => {}
            },
            Char(c) => app.editor_push(c),
            Enter => match app.editor_tab {
                EditorTab::Body => app.body.push('\n'),
                _ => {}
            },
            _ => {}
        }
        return false;
    }

    // ── normal / navigation mode ─────────────────────────────────────────────
    match key.code {
        Char('q') => return true,
        Char('?') => {
            app.overlay = Overlay::Help;
        }

        // panel cycling
        Tab => {
            app.active_panel = match app.active_panel {
                Panel::Collections => Panel::Requests,
                Panel::Requests => Panel::Response,
                Panel::Response => Panel::Collections,
            };
        }
        BackTab => {
            app.active_panel = match app.active_panel {
                Panel::Collections => Panel::Response,
                Panel::Requests => Panel::Collections,
                Panel::Response => Panel::Requests,
            };
        }

        // navigation
        Char('j') | Down => match app.active_panel {
            Panel::Collections => app.coll_next(),
            Panel::Requests => app.req_next(),
            Panel::Response => app.response_scroll = app.response_scroll.saturating_add(1),
        },
        Char('k') | Up => match app.active_panel {
            Panel::Collections => app.coll_prev(),
            Panel::Requests => app.req_prev(),
            Panel::Response => app.response_scroll = app.response_scroll.saturating_sub(1),
        },

        // new
        Char('n') => match app.active_panel {
            Panel::Collections => {
                app.overlay = Overlay::NewCollection {
                    name: String::new(),
                    desc: String::new(),
                    field: 0,
                };
            }
            Panel::Requests => {
                if app.selected_collection_id().is_some() {
                    app.overlay = Overlay::NewRequest {
                        name: String::new(),
                        url: String::new(),
                        method_idx: 0,
                        field: 0,
                    };
                }
            }
            _ => {}
        },

        // delete
        Char('d') => match app.active_panel {
            Panel::Collections => {
                if let Some(id) = app.selected_collection_id() {
                    app.overlay = Overlay::ConfirmDelete {
                        target: DeleteTarget::Collection(id),
                    };
                }
            }
            Panel::Requests => {
                let visible = app.requests_for_selected();
                if let Some(i) = app.req_state.selected() {
                    if let Some(r) = visible.get(i) {
                        app.overlay = Overlay::ConfirmDelete {
                            target: DeleteTarget::Request(r.request_id),
                        };
                    }
                }
            }
            _ => {}
        },

        // open editor
        Char('e') | Enter if app.active_panel == Panel::Requests => {
            app.load_request_into_editor();
        }

        // send
        Char('s') => {
            if !app.editor_open {
                app.load_request_into_editor();
            }
            // tokio::runtime::Handle::current().block_on(app.send_request());
            app.send_request().await;

            app.active_panel = Panel::Response;
        }

        // enter insert in editor
        Char('i') if app.editor_open => {
            app.input_mode = InputMode::Insert;
        }

        _ => {}
    }

    false
}

// ══════════════════════════════════════════════════════════════════════════════
//  MAIN
// ══════════════════════════════════════════════════════════════════════════════

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let conn = Connection::open("data.db")?;
    check_database(&conn)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new(conn);

    loop {
        terminal.draw(|f| draw(f, &app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if handle_event(&mut app, key).await {
                    break;
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}
