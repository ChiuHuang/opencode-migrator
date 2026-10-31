use crate::models::{OpenCodeSession, TargetPlatform};
use crate::{agy, freebuff, opencode};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::collections::HashSet;
use std::io;
use std::path::PathBuf;

pub struct AppState {
    pub opencode_db: PathBuf,
    pub freebuff_dir: Option<PathBuf>,
    pub agy_dir: Option<PathBuf>,
    pub sessions: Vec<OpenCodeSession>,
    pub list_state: ListState,
    pub selected_ids: HashSet<String>,
    pub target: TargetPlatform,
    pub delete_source: bool,
    pub status_message: Option<(String, Color)>,
    pub modal_dialog: Option<String>,
    pub should_quit: bool,
}

impl AppState {
    pub fn new(
        opencode_db: PathBuf,
        freebuff_dir: Option<PathBuf>,
        agy_dir: Option<PathBuf>,
    ) -> Result<Self, String> {
        let sessions = opencode::list_sessions(&opencode_db).map_err(|e| e.to_string())?;
        let mut list_state = ListState::default();
        if !sessions.is_empty() {
            list_state.select(Some(0));
        }

        Ok(Self {
            opencode_db,
            freebuff_dir,
            agy_dir,
            sessions,
            list_state,
            selected_ids: HashSet::new(),
            target: TargetPlatform::Freebuff,
            delete_source: false,
            status_message: None,
            modal_dialog: None,
            should_quit: false,
        })
    }

    pub fn refresh_sessions(&mut self) {
        if let Ok(sessions) = opencode::list_sessions(&self.opencode_db) {
            self.sessions = sessions;
            if self.sessions.is_empty() {
                self.list_state.select(None);
            } else if self.list_state.selected().is_none() {
                self.list_state.select(Some(0));
            } else if let Some(sel) = self.list_state.selected() {
                if sel >= self.sessions.len() {
                    self.list_state.select(Some(self.sessions.len() - 1));
                }
            }
            self.status_message = Some(("Refreshed session list.".to_string(), Color::Green));
        }
    }

    pub fn next(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i + 1 >= self.sessions.len() {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn previous(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.sessions.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn toggle_selection(&mut self) {
        if let Some(i) = self.list_state.selected() {
            if let Some(s) = self.sessions.get(i) {
                if self.selected_ids.contains(&s.session_id) {
                    self.selected_ids.remove(&s.session_id);
                } else {
                    self.selected_ids.insert(s.session_id.clone());
                }
            }
        }
    }

    pub fn toggle_all(&mut self) {
        if self.selected_ids.len() == self.sessions.len() {
            self.selected_ids.clear();
        } else {
            self.selected_ids = self.sessions.iter().map(|s| s.session_id.clone()).collect();
        }
    }

    pub fn cycle_target(&mut self) {
        self.target = match self.target {
            TargetPlatform::Freebuff => TargetPlatform::Agy,
            TargetPlatform::Agy => TargetPlatform::Both,
            TargetPlatform::Both => TargetPlatform::Freebuff,
        };
    }

    pub fn run_migration(&mut self) {
        let to_migrate: Vec<String> = if !self.selected_ids.is_empty() {
            self.selected_ids.iter().cloned().collect()
        } else if let Some(i) = self.list_state.selected() {
            if let Some(s) = self.sessions.get(i) {
                vec![s.session_id.clone()]
            } else {
                return;
            }
        } else {
            return;
        };

        if to_migrate.is_empty() {
            self.status_message = Some((
                "No sessions selected for migration.".to_string(),
                Color::Yellow,
            ));
            return;
        }

        let mut success = 0;
        let mut failed = 0;
        let mut logs = Vec::new();

        logs.push(format!(
            "Starting migration of {} session(s)...",
            to_migrate.len()
        ));
        logs.push(format!("Target: {}", self.target));
        logs.push(format!(
            "Mode: {}",
            if self.delete_source {
                "MOVE (delete source)"
            } else {
                "COPY"
            }
        ));
        logs.push("".to_string());

        for sid in &to_migrate {
            match opencode::load_full_session(&self.opencode_db, sid) {
                Ok(Some(session)) => {
                    let mut ok = true;

                    if matches!(self.target, TargetPlatform::Freebuff | TargetPlatform::Both) {
                        if let Some(fb_root) = &self.freebuff_dir {
                            match freebuff::import_session(&session, fb_root) {
                                Ok(tid) => {
                                    logs.push(format!(
                                        "[+] {}: Imported to Freebuff thread {}",
                                        sid, tid
                                    ));
                                }
                                Err(e) => {
                                    ok = false;
                                    logs.push(format!("[-] {}: Freebuff error: {}", sid, e));
                                }
                            }
                        } else {
                            ok = false;
                            logs.push(format!("[-] {}: Freebuff directory not found", sid));
                        }
                    }

                    if matches!(self.target, TargetPlatform::Agy | TargetPlatform::Both) {
                        if let Some(agy_root) = &self.agy_dir {
                            match agy::import_session(&session, agy_root) {
                                Ok(cid) => {
                                    logs.push(format!("[+] {}: Imported to AGY conv {}", sid, cid));
                                }
                                Err(e) => {
                                    ok = false;
                                    logs.push(format!("[-] {}: AGY error: {}", sid, e));
                                }
                            }
                        } else {
                            ok = false;
                            logs.push(format!("[-] {}: AGY directory not found", sid));
                        }
                    }

                    if ok {
                        if self.delete_source {
                            let _ = opencode::delete_session(&self.opencode_db, sid);
                            logs.push(format!("    Deleted {} from OpenCode source.", sid));
                        }
                        success += 1;
                    } else {
                        failed += 1;
                    }
                }
                Ok(None) => {
                    failed += 1;
                    logs.push(format!("[-] {}: Not found in OpenCode DB", sid));
                }
                Err(e) => {
                    failed += 1;
                    logs.push(format!("[-] {}: Load error: {}", sid, e));
                }
            }
        }

        logs.push("".to_string());
        logs.push(format!("Done: {} succeeded, {} failed.", success, failed));
        logs.push("Press any key to close this dialog.".to_string());

        self.modal_dialog = Some(logs.join("\n"));
        self.selected_ids.clear();
        self.refresh_sessions();
    }
}

pub fn run_tui(mut state: AppState) -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|f| ui(f, &mut state))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                if state.modal_dialog.is_some() {
                    state.modal_dialog = None;
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        state.should_quit = true;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        state.next();
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        state.previous();
                    }
                    KeyCode::Char(' ') => {
                        state.toggle_selection();
                    }
                    KeyCode::Char('a') => {
                        state.toggle_all();
                    }
                    KeyCode::Tab | KeyCode::Right | KeyCode::Left => {
                        state.cycle_target();
                    }
                    KeyCode::Char('1') => {
                        state.target = TargetPlatform::Freebuff;
                    }
                    KeyCode::Char('2') => {
                        state.target = TargetPlatform::Agy;
                    }
                    KeyCode::Char('3') => {
                        state.target = TargetPlatform::Both;
                    }
                    KeyCode::Char('d') => {
                        state.delete_source = !state.delete_source;
                    }
                    KeyCode::Char('r') => {
                        state.refresh_sessions();
                    }
                    KeyCode::Enter | KeyCode::Char('m') => {
                        state.run_migration();
                    }
                    _ => {}
                }
            }
        }

        if state.should_quit {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn ui(f: &mut Frame, state: &mut AppState) {
    let size = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Main View (List + Details)
            Constraint::Length(4), // Target / Mode & Controls
        ])
        .split(size);

    // --- Header ---
    let header_text = vec![Line::from(vec![
        Span::styled(
            " OpenCode Session Migrator ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("(Ratatui TUI) | DB: "),
        Span::styled(
            state.opencode_db.to_string_lossy().to_string(),
            Style::default().fg(Color::Yellow),
        ),
    ])];
    let header = Paragraph::new(header_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(header, chunks[0]);

    // --- Main View (Left: List, Right: Details) ---
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(chunks[1]);

    // Sessions List
    let items: Vec<ListItem> = state
        .sessions
        .iter()
        .map(|s| {
            let is_checked = state.selected_ids.contains(&s.session_id);
            let check_mark = if is_checked { "[x] " } else { "[ ] " };
            let count_str = format!("({} msgs)", s.message_count);
            let title = if s.title.is_empty() {
                "Untitled"
            } else {
                &s.title
            };

            let text = Line::from(vec![
                Span::styled(
                    check_mark,
                    if is_checked {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
                Span::styled(
                    format!("{:<28} ", s.session_id),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(
                    format!("{:<10} ", count_str),
                    Style::default().fg(Color::Blue),
                ),
                Span::raw(title.chars().take(35).collect::<String>()),
            ]);
            ListItem::new(text)
        })
        .collect();

    let list_title = format!(
        " OpenCode Sessions ({}) [Selected: {}] ",
        state.sessions.len(),
        state.selected_ids.len()
    );
    let list_widget = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(list_title))
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(30, 45, 60))
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list_widget, main_chunks[0], &mut state.list_state);

    // Details View
    let detail_text = if let Some(i) = state.list_state.selected() {
        if let Some(s) = state.sessions.get(i) {
            let dt_str = if s.time_updated > 0 {
                chrono::DateTime::<chrono::Utc>::from_timestamp(s.time_updated / 1000, 0)
                    .map(|d| d.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                    .unwrap_or_else(|| "Unknown".to_string())
            } else {
                "Unknown".to_string()
            };

            vec![
                Line::from(vec![
                    Span::styled("Session ID: ", Style::default().fg(Color::Cyan)),
                    Span::styled(&s.session_id, Style::default().fg(Color::Yellow)),
                ]),
                Line::from(vec![
                    Span::styled("Title:      ", Style::default().fg(Color::Cyan)),
                    Span::raw(&s.title),
                ]),
                Line::from(vec![
                    Span::styled("Directory:  ", Style::default().fg(Color::Cyan)),
                    Span::raw(&s.directory),
                ]),
                Line::from(vec![
                    Span::styled("Model:      ", Style::default().fg(Color::Cyan)),
                    Span::raw(&s.model),
                ]),
                Line::from(vec![
                    Span::styled("Messages:   ", Style::default().fg(Color::Cyan)),
                    Span::raw(s.message_count.to_string()),
                ]),
                Line::from(vec![
                    Span::styled("Updated:    ", Style::default().fg(Color::Cyan)),
                    Span::raw(dt_str),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Tip: ", Style::default().fg(Color::Yellow)),
                    Span::raw("Press Space to select, Enter or 'm' to migrate."),
                ]),
            ]
        } else {
            vec![Line::from("No session selected")]
        }
    } else {
        vec![Line::from("No sessions available")]
    };

    let detail_widget = Paragraph::new(detail_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Session Details "),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(detail_widget, main_chunks[1]);

    // --- Bottom Bar ---
    let target_freebuff_style = if state.target == TargetPlatform::Freebuff {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    let target_agy_style = if state.target == TargetPlatform::Agy {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };
    let target_both_style = if state.target == TargetPlatform::Both {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Magenta)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };

    let delete_style = if state.delete_source {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Green)
    };

    let bottom_lines = vec![
        Line::from(vec![
            Span::raw(" Target: "),
            Span::styled(" [1] Freebuff ", target_freebuff_style),
            Span::raw(" "),
            Span::styled(" [2] AGY (Antigravity) ", target_agy_style),
            Span::raw(" "),
            Span::styled(" [3] Both ", target_both_style),
            Span::raw("  | Mode: "),
            Span::styled(
                if state.delete_source {
                    "[MOVE - Delete Source]"
                } else {
                    "[COPY - Safe]"
                },
                delete_style,
            ),
            Span::styled(" (toggle with 'd')", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(" ↑/↓ / j/k", Style::default().fg(Color::Yellow)),
            Span::raw(" Move | "),
            Span::styled("Space", Style::default().fg(Color::Yellow)),
            Span::raw(" Select | "),
            Span::styled("a", Style::default().fg(Color::Yellow)),
            Span::raw(" All | "),
            Span::styled("Tab", Style::default().fg(Color::Yellow)),
            Span::raw(" Target | "),
            Span::styled(
                "Enter/m",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Migrate | "),
            Span::styled("r", Style::default().fg(Color::Yellow)),
            Span::raw(" Reload | "),
            Span::styled("q/Esc", Style::default().fg(Color::Red)),
            Span::raw(" Quit"),
        ]),
    ];

    let bottom_widget = Paragraph::new(bottom_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Controls & Settings "),
    );
    f.render_widget(bottom_widget, chunks[2]);

    // --- Modal Dialog Popup ---
    if let Some(msg) = &state.modal_dialog {
        let area = centered_rect(65, 60, size);
        f.render_widget(Clear, area);

        let dialog = Paragraph::new(msg.as_str())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Migration Summary ")
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(dialog, area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
