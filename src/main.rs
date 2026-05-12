use std::{error::Error, io, time::Duration};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use reqwest::Client;
use serde_json::Value;

mod utils;

use utils::{delete, get, patch, post, put, url_splitter};

#[derive(PartialEq)]
enum InputMode {
    Normal,
    Insert,
}

struct App {
    url: String,
    response: String,
    status: String,

    methods: Vec<&'static str>,
    selected_method: usize,

    response_scroll: u16,
    mode: InputMode,
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
        }
    }
}

impl App {
    fn scroll_down(&mut self) {
        self.response_scroll = self.response_scroll.saturating_add(1);
    }

    fn scroll_up(&mut self) {
        self.response_scroll = self.response_scroll.saturating_sub(1);
    }
    fn input_char(&mut self, c: char) {
        self.url.push(c);
    }

    fn backspace(&mut self) {
        self.url.pop();
    }

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

        let result = match method {
            "GET" => get::<Value>(&client, &endpoint, &baseurl, None, None).await,

            "POST" => {
                let body = serde_json::json!({
                    "title": "hello",
                    "body": "something",
                    "userId": 1
                });

                post::<Value, _>(&client, &endpoint, &baseurl, &body, None, None).await
            }

            "PUT" => {
                let body = serde_json::json!({
                    "title": "updated title"
                });

                put::<Value, _>(&client, &endpoint, &baseurl, &body, None, None).await
            }

            "PATCH" => {
                let body = serde_json::json!({
                    "title": "patched title"
                });

                patch::<Value, _>(&client, &endpoint, &baseurl, &body, None, None).await
            }

            "DELETE" => delete::<Value>(&client, &endpoint, &baseurl, None, None).await,

            _ => unreachable!(),
        };

        match result {
            Ok(res) => {
                let pretty_body = serde_json::to_string_pretty(&res.body)
                    .unwrap_or_else(|_| res.raw_text.clone());

                self.response = format!(
                    r#"STATUS
{:#?}

RESPONSE TIME
{:?}

FINAL URL
{}

CONTENT LENGTH
{:?}

HTTP VERSION
{:?}

HEADERS
{:#?}

RAW TEXT
{}

BODY
{}
"#,
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
            }

            Err(e) => {
                self.response = format!("REQUEST FAILED\n\n{:#?}", e);

                self.status = "ERROR".into();
            }
        }
    }
}

fn ui(frame: &mut ratatui::Frame, app: &App) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
        .split(layout[0]);

    let method_items = app
        .methods
        .iter()
        .enumerate()
        .map(|(i, method)| {
            let style = if i == app.selected_method {
                Style::default().fg(Color::Black).bg(Color::Green)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(*method).style(style)
        })
        .collect::<Vec<_>>();

    let methods =
        List::new(method_items).block(Block::default().title("Methods").borders(Borders::ALL));

    frame.render_widget(methods, top[0]);

    let url = Paragraph::new(app.url.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().title("URL").borders(Borders::ALL))
        .wrap(Wrap { trim: false });

    frame.render_widget(url, top[1]);

    let response = Paragraph::new(app.response.as_str())
        .style(Style::default().fg(Color::Green))
        .block(
            Block::default()
                .title("Response")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .scroll((app.response_scroll, 0))
        .wrap(Wrap { trim: false });

    frame.render_widget(response, layout[1]);

    let mode_text = match  app.mode {
        InputMode::Insert => "INSERT",
        InputMode::Normal => "NORMAL",
    };

    let footer =
        Paragraph::new(format!("MODE: {} | q=quit | ↑↓=method | Enter=send | i=insert | esc=normal | j/k=response scroll",mode_text))
            .style(Style::default().fg(Color::Magenta))
            .block(
                Block::default()
                    .title(app.status.as_str())
                    .borders(Borders::ALL),
            );

    frame.render_widget(footer, layout[2]);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
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
                // prevents double key press
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match app.mode {
                    InputMode::Insert => match key.code {
                        KeyCode::Esc => {
                            app.mode = InputMode::Normal;
                        }

                        KeyCode::Backspace => {
                            app.backspace();
                        }

                        KeyCode::Enter => {
                            app.send_request().await;
                        }

                        KeyCode::Char(c) => {
                            app.input_char(c);
                        }

                        _ => {}
                    },

                    InputMode::Normal => match key.code {
                        KeyCode::Char('q') => {
                            break;
                        }

                        KeyCode::Char('i') => {
                            app.mode = InputMode::Insert;
                        }

                        KeyCode::Char('j') => {
                            app.scroll_down();
                        }

                        KeyCode::Char('k') => {
                            app.scroll_up();
                        }

                        KeyCode::Up => {
                            app.prev_method();
                        }

                        KeyCode::Down => {
                            app.next_method();
                        }

                        _ => {}
                    },
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
