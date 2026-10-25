mod models;
mod mail_service;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use std::fs;
use std::io;
use std::path::Path;

use crate::models::{Config, MailMessage};
use crate::mail_service::MailService;

const SETTINGS_FILE: &str = "settings.toml";

#[derive(PartialEq)]
enum AppMode {
    Inbox,
    Compose,
    Settings,
    ViewEmail,
}

struct App {
    mode: AppMode,
    config: Config,
    messages: Vec<MailMessage>,
    selected_message: Option<MailMessage>,
    list_state: ListState,
    status_message: String,
    input_buffer: String,
    // Settings editing state
    settings_index: usize,
    is_editing_setting: bool,
}

impl App {
    fn new() -> App {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        
        let config = Self::load_config().unwrap_or_default();

        App {
            mode: AppMode::Inbox,
            config,
            messages: vec![],
            selected_message: None,
            list_state,
            status_message: "Welcome to Mail CLI".to_string(),
            input_buffer: String::new(),
            settings_index: 0,
            is_editing_setting: false,
        }
    }

    fn load_config() -> Option<Config> {
        if Path::new(SETTINGS_FILE).exists() {
            let content = fs::read_to_string(SETTINGS_FILE).ok()?;
            toml::from_str(&content).ok()
        } else {
            None
        }
    }

    fn save_config(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let content = toml::to_string(&self.config)?;
        fs::write(SETTINGS_FILE, content)?;
        self.status_message = "Settings saved successfully!".to_string();
        Ok(())
    }

    fn next(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.messages.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.messages.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let mail_service = MailService::new(app.config.clone());

    if let Ok(msgs) = mail_service.fetch_messages().await {
        app.messages = msgs;
    }

    let res = run_app(&mut terminal, &mut app, &mail_service);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    mail_service: &MailService,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => return Ok(()),
                
                // Global/Inbox keys
                KeyCode::Down if app.mode == AppMode::Inbox => app.next(),
                KeyCode::Up if app.mode == AppMode::Inbox => app.previous(),
                KeyCode::Char('c') if app.mode == AppMode::Inbox => {
                    app.mode = AppMode::Compose;
                    app.input_buffer.clear();
                }
                KeyCode::Char('s') if app.mode == AppMode::Inbox => {
                    app.mode = AppMode::Settings;
                    app.is_editing_setting = false;
                    app.settings_index = 0;
                }
                KeyCode::Esc if app.mode != AppMode::Inbox => {
                    app.mode = AppMode::Inbox;
                    app.selected_message = None;
                    app.input_buffer.clear();
                    app.is_editing_setting = false;
                }

                // Compose Mode
                KeyCode::Char(c) if app.mode == AppMode::Compose => {
                    app.input_buffer.push(c);
                }
                KeyCode::Backspace if app.mode == AppMode::Compose => {
                    app.input_buffer.pop();
                }
                KeyCode::Enter if app.mode == AppMode::Compose => {
                    app.status_message = format!("Sent: {}", app.input_buffer);
                    app.input_buffer.clear();
                    app.mode = AppMode::Inbox;
                }

                // Settings Mode
                KeyCode::Up if app.mode == AppMode::Settings => {
                    if !app.is_editing_setting {
                        app.settings_index = app.settings_index.saturating_sub(1);
                    }
                }
                KeyCode::Down if app.mode == AppMode::Settings => {
                    if !app.is_editing_setting {
                        app.settings_index = (app.settings_index + 1).min(5);
                    }
                }
                KeyCode::Enter if app.mode == AppMode::Settings => {
                    if !app.is_editing_setting {
                        // Start editing selected field
                        app.is_editing_setting = true;
                        app.input_buffer = match app.settings_index {
                            0 => app.config.imap_server.clone(),
                            1 => app.config.imap_port.to_string(),
                            2 => app.config.smtp_server.clone(),
                            3 => app.config.smtp_port.to_string(),
                            4 => app.config.user.clone(),
                            5 => app.config.pass.clone(),
                            _ => "".to_string(),
                        };
                    } else {
                        // Commit editing field
                        match app.settings_index {
                            0 => app.config.imap_server = app.input_buffer.clone(),
                            1 => if let Ok(p) = app.input_buffer.parse() { app.config.imap_port = p },
                            2 => app.config.smtp_server = app.input_buffer.clone(),
                            3 => if let Ok(p) = app.input_buffer.parse() { app.config.smtp_port = p },
                            4 => app.config.user = app.input_buffer.clone(),
                            5 => app.config.pass = app.input_buffer.clone(),
                            _ => {}
                        }
                        app.is_editing_setting = false;
                        let _ = app.save_config(); // Attempt save
                    }
                }
                KeyCode::Char(c) if app.mode == AppMode::Settings && app.is_editing_setting => {
                    app.input_buffer.push(c);
                }
                KeyCode::Backspace if app.mode == AppMode::Settings && app.is_editing_setting => {
                    app.input_buffer.pop();
                }

                // View Email Mode
                KeyCode::Enter if app.mode == AppMode::Inbox => {
                    if let Some(i) = app.list_state.selected() {
                        if i < app.messages.len() {
                            app.selected_message = Some(app.messages[i].clone());
                            app.mode = AppMode::ViewEmail;
                        }
                    }
                }

                _ => {}
            }
        }
    }
}

fn ui(f: &mut ratatui::Frame, app: &mut App) {
    let area = f.size();
    let top_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
        .split(area);

    // Status bar with feedback
    let base_status = match app.mode {
        AppMode::Inbox => " [I]nbox | [C]ompose | [S]ettings | [Q]uit | [Enter] View ",
        AppMode::Compose => " [E]dit Mode | [ESC] Back | [Q]uit | [Enter] Send ",
        AppMode::Settings => " [↑/↓] Select Field | [Enter] Edit/Save | [ESC] Back ",
        AppMode::ViewEmail => " [ESC] Back | [Q]uit ",
    };

    let display_status = if !app.status_message.is_empty() && app.mode == AppMode::Inbox {
         format!("{} | {}", base_status, app.status_message)
    } else {
         base_status.to_string()
    };

    let status_style = Style::default().fg(Color::White).bg(Color::Rgb(30, 30, 30));
    let status = Paragraph::new(display_status)
        .style(status_style)
        .block(Block::default().borders(Borders::ALL).border_type(ratatui::widgets::BorderType::Rounded));
    f.render_widget(status, top_chunks[0]);

    match app.mode {
        AppMode::Inbox => {
            let items: Vec<ListItem> = app.messages.iter()
                .map(|m| ListItem::new(format!("{} | {} | {}", m.date, m.from, m.subject)))
                .collect();
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).border_type(ratatui::widgets::BorderType::Rounded).title(" Messages "))
                .highlight_style(Style::default().bg(Color::Rgb(50, 50, 80)).add_modifier(Modifier::BOLD));
            f.render_stateful_widget(list, top_chunks[1], &mut app.list_state);
        }
        AppMode::Compose => {
            let display = if app.input_buffer.is_empty() { "Type message here..." } else { &app.input_buffer };
            let body = Paragraph::new(display)
                .block(Block::default().borders(Borders::ALL).border_type(ratatui::widgets::BorderType::Rounded).title(" Compose New Message "));
            f.render_widget(body, top_chunks[1]);
        }
        AppMode::Settings => {
             let fields = [
                 "IMAP Server", 
                 "IMAP Port", 
                 "SMTP Server", 
                 "SMTP Port", 
                 "User", 
                 "Pass"
             ];
             
             let mut items = Vec::new();
             for (i, field) in fields.iter().enumerate() {
                 let val = match i {
                     0 => &app.config.imap_server,
                     1 => &app.config.imap_port.to_string(),
                     2 => &app.config.smtp_server,
                     3 => &app.config.smtp_port.to_string(),
                     4 => &app.config.user,
                     5 => "********", // Hide password in list
                     _ => "",
                 };
                 // If we are currently editing this field, show the buffer instead
                 let display_val = if app.is_editing_setting && app.settings_index == i {
                    &app.input_buffer
                 } else {
                    val
                 };

                 items.push(ListItem::new(format!("{:<15}: {}", field, display_val)));
             }

             let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).border_type(ratatui::widgets::BorderType::Rounded).title(" Settings "))
                .highlight_style(Style::default().bg(Color::Rgb(50, 50, 80)).add_modifier(Modifier::BOLD));
             f.render_stateful_widget(list, top_chunks[1], &mut app.list_state);
        }
        AppMode::ViewEmail => {
            if let Some(msg) = &app.selected_message {
                let content = format!("From: {}\nDate: {}\nSubject: {}\n\n{}", msg.from, msg.date, msg.subject, msg.body);
                let body = Paragraph::new(content)
                    .block(Block::default().borders(Borders::ALL).border_type(ratatui::widgets::BorderType::Rounded).title(" Email Content "));
                f.render_widget(body, top_chunks[1]);
            }
        }
    }
}
