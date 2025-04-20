use std::collections::HashSet;
use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Paragraph, Widget},
    DefaultTerminal, Frame,
};

use crate::{
    config::Config,
    scanner::{BackupApplication, Volume},
    utils::run_remote_cmd_with_output,
};

struct RestoreConfig {
    project: Option<String>,
    version: Option<String>,
    repo: bool,
    volumes: Vec<Volume>,
}

impl RestoreConfig {
    fn emppty() -> Self {
        Self {
            project: None,
            version: None,
            repo: false,
            volumes: Vec::new(),
        }
    }
}

pub fn handle_restore_command(
    config: &Config,
    project: Option<String>,
    version: Option<String>,
    repo: bool,
    volumes: Vec<String>,
) {
    let no_args_provided = project.is_none();

    if no_args_provided {
        if let Err(e) = enter_interactive_shell(config) {
            eprintln!("❌ Error in interactive shell: {e}");
        }
    } else {
        todo!(
            "Implement restore logic here, from direct CLI call {},{},{},{:?}",
            project.unwrap(),
            version.unwrap_or_default(),
            repo,
            volumes
        );
    }
}

fn enter_interactive_shell(config: &Config) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let mut app = futures::executor::block_on(RestoreApp::new(config));
    app.run(&mut terminal)
}

pub struct RestoreApp {
    backups: Vec<BackupApplication>,
    config: RestoreConfig,
    exit: bool,
    selected_index: usize,
    selected_volumes: HashSet<String>,
}

impl RestoreApp {
    pub async fn new(config: &Config) -> Self {
        let backups = scan_backup_target(config).await.unwrap_or_else(|e| {
            eprintln!("❌ Error scanning backup target: {e}");
            Vec::new()
        });
        Self {
            backups,
            config: RestoreConfig::emppty(),
            exit: false,
            selected_index: 0,
            selected_volumes: HashSet::new(),
        }
    }
}

impl RestoreApp {
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        log::debug!("{:?}", self.backups);
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(vec![Constraint::Percentage(80), Constraint::Percentage(20)])
            .split(frame.area());

        let chunk = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Percentage(33),
                Constraint::Percentage(33),
                Constraint::Percentage(34),
            ])
            .split(layout[0]);

        // Render projects
        let project_names: Vec<Line> = self
            .backups
            .iter()
            .map(|app| Line::from(app.name.clone()))
            .collect();
        Paragraph::new(Text::from(project_names))
            .block(
                Block::default()
                    .title("Projects")
                    .borders(ratatui::widgets::Borders::ALL),
            )
            .render(chunk[0], frame.buffer_mut());

        // Render dates
        let dates: Vec<Line> = self
            .backups
            .iter()
            .map(|app| Line::from(app.timestamp.to_string()))
            .collect();
        Paragraph::new(Text::from(dates))
            .block(
                Block::default()
                    .title("Dates")
                    .borders(ratatui::widgets::Borders::ALL),
            )
            .render(chunk[1], frame.buffer_mut());

        // Render volume checkboxes
        let volume_texts: Vec<Line> = self
            .config
            .volumes
            .iter()
            .map(|volume| {
                let checkbox: String = if self.selected_volumes.contains(&volume.name) {
                    "[x] ".to_string()
                } else {
                    "[ ] ".to_string()
                };
                Line::from(format!("{}{}", checkbox, volume.name))
            })
            .collect();
        Paragraph::new(Text::from(volume_texts))
            .block(
                Block::default()
                    .title("Volumes")
                    .borders(ratatui::widgets::Borders::ALL),
            )
            .render(chunk[2], frame.buffer_mut());

        // Render summary
        let summary_text = format!(
            "Selected Project: {}\nSelected Volumes: {:?}",
            self.backups
                .get(self.selected_index)
                .map_or("None".to_string(), |app| app.name.clone()),
            self.selected_volumes
        );
        Paragraph::new(Line::from(summary_text))
            .block(
                Block::default()
                    .title("Summary")
                    .borders(ratatui::widgets::Borders::ALL),
            )
            .render(layout[1], frame.buffer_mut());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        };
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Up => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            KeyCode::Down => {
                if self.selected_index < self.backups.len() - 1 {
                    self.selected_index += 1;
                }
            }
            KeyCode::Char(' ') => {
                let selected_backup = &self.backups[self.selected_index];
                if self.selected_volumes.contains(&selected_backup.name) {
                    self.selected_volumes.remove(&selected_backup.name);
                } else {
                    self.selected_volumes.insert(selected_backup.name.clone());
                }
            }
            KeyCode::Char('q') => self.exit(),
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

impl Widget for &RestoreApp {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Line::from(" Dockup Restore ".bold());
        let instructions = Line::from(vec![
            " Select ".into(),
            "<Up> <Down>".blue().bold(),
            " Quit ".into(),
            "<Q> ".blue().bold(),
        ]);
        let block = Block::bordered()
            .title(title.centered())
            .title_bottom(instructions.centered())
            .border_set(border::THICK);

        let counter_text = Text::from(vec![Line::from(vec!["Value: ".into()])]);

        Paragraph::new(counter_text)
            .centered()
            .block(block)
            .render(area, buf);
    }
}

async fn scan_backup_target(config: &Config) -> anyhow::Result<Vec<BackupApplication>> {
    log::debug!("Scanning backup target: {}", config.remote_backup_path);
    let mut backups = Vec::new();
    let listing =
        run_remote_cmd_with_output(config, &format!("ls -1 {}", config.remote_backup_path))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let application_folders = listing
        .lines()
        .filter(|line| !line.contains("."))
        .collect::<Vec<_>>();

    for app in application_folders {
        log::debug!("Found backup application: {}", app);
        let folders = run_remote_cmd_with_output(
            config,
            &format!("ls -1 {}/{}", config.remote_backup_path, app),
        )
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let just_folders = folders
            .lines()
            .filter(|line| !line.contains("."))
            .collect::<Vec<_>>();
        log::debug!("Found backup folders: {:?}", just_folders);
        for dir in just_folders {
            log::debug!("Found backup directory: {}", dir);
            log::debug!(
                "meta.json path: {}/{}/meta.json",
                config.remote_backup_path,
                dir
            );
            let meta = run_remote_cmd_with_output(
                config,
                &format!(
                    "cat {}/{}/{}/meta.json",
                    config.remote_backup_path, app, dir
                ),
            );

            if let Ok(json) = meta {
                if let Ok(mut app) = serde_json::from_str::<BackupApplication>(&json) {
                    // (Optional) fallback timestamp
                    if app.timestamp.timestamp() == 0 {
                        if let Ok(parsed) = chrono::DateTime::parse_from_str(dir, "%Y_%m_%d_%H%M%S")
                        {
                            app.timestamp = parsed.with_timezone(&chrono::Local);
                        }
                    }
                    backups.push(app);
                }
            }
        }
    }
    log::debug!("Summary of backups: {:?}", backups);
    log::info!("Found {} backups", backups.len());
    Ok(backups)
}
