use std::collections::HashSet;
use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Paragraph, Widget},
    DefaultTerminal, Frame,
};

use crate::logger::disable_stdout_logging;
use crate::logger::enable_stdout_logging;
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

    // First render may get corrupted due to logging output
    terminal.draw(|frame| app.draw(frame))?;

    // Disable log output once TUI begins
    disable_stdout_logging();

    // Clear any leftover log noise with a full redraw
    terminal.clear()?;
    terminal.draw(|frame| app.draw(frame))?;

    app.run(&mut terminal)?;
    ratatui::restore();
    enable_stdout_logging();
    Ok(())
}

pub struct RestoreApp {
    backups: Vec<BackupApplication>,
    projects: Vec<String>,
    config: RestoreConfig,
    exit: bool,
    selected_project_index: usize,
    selected_backup_index: usize,
    selected_volume_index: usize,
    selected_column: usize,
    selected_volumes: HashSet<String>,
}

impl RestoreApp {
    pub async fn new(config: &Config) -> Self {
        let backups = scan_backup_target(config).await.unwrap_or_else(|e| {
            eprintln!("❌ Error scanning backup target: {e}");
            Vec::new()
        });

        let mut seen = HashSet::new();
        let projects: Vec<String> = backups
            .iter()
            .map(|b| b.name.clone())
            .filter(|name| seen.insert(name.clone()))
            .collect();

        Self {
            backups,
            projects,
            config: RestoreConfig::emppty(),
            exit: false,
            selected_project_index: 0,
            selected_backup_index: 0,
            selected_volume_index: 0,
            selected_column: 0,
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
            .projects
            .iter()
            .enumerate()
            .map(|(i, app)| {
                let style = if self.selected_column == 0 && i == self.selected_project_index {
                    Style::default().add_modifier(ratatui::style::Modifier::REVERSED)
                } else {
                    Style::default()
                };
                Line::from(app.clone()).style(style)
            })
            .collect();
        Paragraph::new(Text::from(project_names))
            .block(
                Block::default()
                    .title("Projects")
                    .borders(ratatui::widgets::Borders::ALL),
            )
            .render(chunk[0], frame.buffer_mut());

        // Render dates
        let selected_project = &self.projects[self.selected_project_index];
        let dates: Vec<Line> = self
            .backups
            .iter()
            .filter(|b| &b.name == selected_project)
            .enumerate()
            .map(|(i, app)| {
                let style = if self.selected_column == 1 && i == self.selected_backup_index {
                    Style::default().add_modifier(ratatui::style::Modifier::REVERSED)
                } else {
                    Style::default()
                };
                Line::from(app.timestamp.format("%d. %B %Y %H:%M:%S").to_string()).style(style)
            })
            .collect();
        Paragraph::new(Text::from(dates))
            .block(
                Block::default()
                    .title("Dates")
                    .borders(ratatui::widgets::Borders::ALL),
            )
            .render(chunk[1], frame.buffer_mut());

        // Render volume checkboxes
        let selected_backups: Vec<_> = self
            .backups
            .iter()
            .filter(|b| &b.name == selected_project)
            .collect();
        let mut volume_lines: Vec<Line> = if let Some(selected_backup) =
            selected_backups.get(self.selected_backup_index)
        {
            selected_backup
                .volumes
                .iter()
                .enumerate()
                .map(|(i, volume)| {
                    let style = if self.selected_column == 2 && i == self.selected_volume_index {
                        Style::default().add_modifier(ratatui::style::Modifier::REVERSED)
                    } else {
                        Style::default()
                    };
                    let checkbox: String = if self.selected_volumes.contains(&volume.name) {
                        "[x] ".to_string()
                    } else {
                        "[ ] ".to_string()
                    };
                    Line::from(format!("{}{}", checkbox, volume.name)).style(style)
                })
                .collect()
        } else {
            Vec::new()
        };
        // Add REPO as selectable
        let repo_index =
            if let Some(selected_backup) = selected_backups.get(self.selected_backup_index) {
                selected_backup.volumes.len()
            } else {
                0
            };
        let repo_style = if self.selected_column == 2 && self.selected_volume_index == repo_index {
            Style::default().add_modifier(ratatui::style::Modifier::REVERSED)
        } else {
            Style::default()
        };
        let repo_checkbox = if self.selected_volumes.contains("repo") {
            "[x] "
        } else {
            "[ ] "
        };
        volume_lines.push(Line::from(format!("{}REPO", repo_checkbox)).style(repo_style));

        Paragraph::new(Text::from(volume_lines))
            .block(
                Block::default()
                    .title("Volumes")
                    .borders(ratatui::widgets::Borders::ALL),
            )
            .render(chunk[2], frame.buffer_mut());

        // Render summary
        let selected_project = self
            .projects
            .get(self.selected_project_index)
            .cloned()
            .unwrap_or_default();
        let selected_backup = self
            .backups
            .iter()
            .filter(|b| b.name == selected_project)
            .nth(self.selected_backup_index);

        let mut summary_lines = vec![format!("Project: {}", selected_project)];

        if let Some(backup) = selected_backup {
            summary_lines.push(format!(
                "Date: {}",
                backup.timestamp.format("%d. %B %Y %H:%M:%S")
            ));
            summary_lines.push(format!(
                "Repo: {}",
                if self.selected_volumes.contains("repo") {
                    "yes"
                } else {
                    "no"
                }
            ));
            summary_lines.push("Volumes:".to_string());
            for volume in &backup.volumes {
                if self.selected_volumes.contains(&volume.name) {
                    summary_lines.push(format!("  - {}", volume.name));
                }
            }
        }

        let summary_text = Text::from(
            summary_lines
                .into_iter()
                .map(Line::from)
                .collect::<Vec<_>>(),
        );
        Paragraph::new(summary_text)
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
                if self.selected_column == 0 && self.selected_project_index > 0 {
                    self.selected_project_index -= 1;
                } else if self.selected_column == 1 {
                    let selected_project = &self.projects[self.selected_project_index];
                    let matching_backups: Vec<_> = self
                        .backups
                        .iter()
                        .filter(|b| &b.name == selected_project)
                        .collect();
                    if self.selected_backup_index > 0 && !matching_backups.is_empty() {
                        self.selected_backup_index -= 1;
                    }
                } else if self.selected_column == 2 {
                    let selected_backups: Vec<_> = self
                        .backups
                        .iter()
                        .filter(|b| &b.name == &self.projects[self.selected_project_index])
                        .collect();
                    if self.selected_volume_index > 0 && !selected_backups.is_empty() {
                        self.selected_volume_index -= 1;
                    }
                }
            }
            KeyCode::Down => {
                if self.selected_column == 0
                    && self.selected_project_index < self.projects.len() - 1
                {
                    self.selected_project_index += 1;
                } else if self.selected_column == 1 {
                    let selected_project = &self.projects[self.selected_project_index];
                    let matching_backups: Vec<_> = self
                        .backups
                        .iter()
                        .filter(|b| &b.name == selected_project)
                        .collect();
                    if self.selected_backup_index < matching_backups.len() - 1 {
                        self.selected_backup_index += 1;
                    }
                } else if self.selected_column == 2 {
                    let selected_backups: Vec<_> = self
                        .backups
                        .iter()
                        .filter(|b| &b.name == &self.projects[self.selected_project_index])
                        .collect();
                    if self.selected_volume_index < selected_backups.len() - 1 {
                        self.selected_volume_index += 1;
                    }
                }
            }
            KeyCode::Left => {
                if self.selected_column > 0 {
                    self.selected_column -= 1;
                }
            }
            KeyCode::Right => {
                if self.selected_column < 2 {
                    self.selected_column += 1;
                }
            }
            KeyCode::Char(' ') => {
                if self.selected_column == 2 {
                    let selected_project = &self.projects[self.selected_project_index];
                    let selected_backups: Vec<_> = self
                        .backups
                        .iter()
                        .filter(|b| &b.name == selected_project)
                        .collect();
                    if let Some(backup) = selected_backups.get(self.selected_backup_index) {
                        if let Some(volume) = backup.volumes.get(self.selected_volume_index) {
                            if self.selected_volumes.contains(&volume.name) {
                                self.selected_volumes.remove(&volume.name);
                            } else {
                                self.selected_volumes.insert(volume.name.clone());
                            }
                        }
                    }
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
        let listing = run_remote_cmd_with_output(
            config,
            &format!("ls -1 {}/{}", config.remote_backup_path, app),
        )
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let backup_folders = listing
            .lines()
            .filter(|line| !line.contains("."))
            .collect::<Vec<_>>();
        log::debug!("Found backup folders: {:?}", backup_folders);
        for backup_folder in backup_folders {
            let meta = run_remote_cmd_with_output(
                config,
                &format!(
                    "cat {}/{}/{}/meta.json",
                    config.remote_backup_path, app, backup_folder
                ),
            );

            let meta = match meta {
                Ok(meta) => {
                    log::debug!("Found meta.json: {}", meta);
                    let meta: BackupApplication = serde_json::from_str(&meta)
                        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
                    log::debug!("Parsed meta.json: {:?}", meta);
                    meta
                }
                Err(e) => {
                    log::error!("Failed to read meta.json: {}", e);
                    continue;
                }
            };
            backups.push(meta);
        }
    }
    Ok(backups)
}
