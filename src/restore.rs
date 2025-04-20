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
use crate::{config::Config, scanner::BackupApplication, utils::run_remote_cmd_with_output};

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
    exit: bool,
    selected_project_index: usize,
    selected_date_index: usize,
    selected_volume_index: usize,
    selected_column: Column,
    selected_volumes: HashSet<String>,
    toggled_repo: bool,
}

#[derive(PartialEq)]
enum Column {
    Projects,
    Dates,
    Volumes,
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
            exit: false,
            selected_project_index: 0,
            selected_date_index: 0,
            selected_volume_index: 0,
            selected_column: Column::Projects,
            selected_volumes: HashSet::new(),
            toggled_repo: false,
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
            .constraints(vec![Constraint::Min(5), Constraint::Length(5)])
            .split(frame.area());

        let chunk = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
            ])
            .split(layout[0]);

        self.draw_projects(chunk[0], frame.buffer_mut());
        self.draw_backups(chunk[1], frame.buffer_mut());
        self.draw_volumes(chunk[2], frame.buffer_mut());
        self.draw_summary(layout[1], frame.buffer_mut());
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
        if key_event.code == KeyCode::Esc || key_event.code == KeyCode::Char('q') {
            self.exit();
            return;
        }
        match self.selected_column {
            Column::Projects => match key_event.code {
                KeyCode::Up => {
                    if self.selected_project_index > 0 {
                        self.selected_project_index -= 1;
                    }
                    self.selected_date_index = 0;
                }
                KeyCode::Down => {
                    if self.selected_project_index < self.projects.len() - 1 {
                        self.selected_project_index += 1;
                    }
                    self.selected_date_index = 0;
                }
                KeyCode::Right => {
                    self.selected_column = Column::Dates;
                }
                _ => {}
            },
            Column::Dates => match key_event.code {
                KeyCode::Up => {
                    if self.selected_date_index > 0 {
                        self.selected_date_index -= 1;
                    }
                    self.selected_volume_index = 0;
                }
                KeyCode::Down => {
                    if self.selected_date_index < self.backups.len() - 1 {
                        self.selected_date_index += 1;
                    }
                    self.selected_volume_index = 0;
                }
                KeyCode::Left => {
                    self.selected_column = Column::Projects;
                }
                KeyCode::Right => {
                    self.selected_column = Column::Volumes;
                }
                _ => {}
            },
            Column::Volumes => match key_event.code {
                KeyCode::Up => {
                    if self.selected_volume_index > 0 {
                        self.selected_volume_index -= 1;
                    }
                }
                KeyCode::Down => {
                    if self.selected_volume_index
                        < self.backups[self.selected_date_index].volumes.len() - 1
                    {
                        self.selected_volume_index += 1;
                    }
                }
                KeyCode::Left => {
                    self.selected_column = Column::Dates;
                    self.toggled_repo = false;
                    self.selected_volumes = HashSet::new();
                }
                KeyCode::Right => {
                    self.selected_column = Column::Projects;
                }
                KeyCode::Char(' ') => {
                    let selected_volume =
                        &self.backups[self.selected_date_index].volumes[self.selected_volume_index];
                    if selected_volume.name == "REPO" {
                        self.toggled_repo = !self.toggled_repo;
                    } else {
                        if self.selected_volumes.contains(&selected_volume.name) {
                            self.selected_volumes.remove(&selected_volume.name);
                        } else {
                            self.selected_volumes.insert(selected_volume.name.clone());
                        }
                    }
                }
                _ => {}
            },
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn draw_projects(&self, area: Rect, buf: &mut Buffer) {
        let projects = get_projects(&self.backups);

        let project_names: Vec<Line> = style_selected(
            &projects,
            self.selected_project_index,
            self.selected_column == Column::Projects,
        );

        Paragraph::new(Text::from(project_names))
            .block(
                Block::default()
                    .title("Projects")
                    .borders(ratatui::widgets::Borders::ALL),
            )
            .render(area, buf);
    }

    fn draw_backups(&self, area: Rect, buf: &mut Buffer) {
        let backups = get_backups(&self.backups, &self.projects[self.selected_project_index]);
        let binding = backups
            .iter()
            .map(|app| app.timestamp.format("%d. %B %Y %H:%M:%S").to_string())
            .collect::<Vec<String>>();
        let dates = style_selected(
            &binding,
            self.selected_date_index,
            self.selected_column == Column::Dates,
        );

        Paragraph::new(Text::from(dates))
            .block(
                Block::default()
                    .title("Dates")
                    .borders(ratatui::widgets::Borders::ALL),
            )
            .render(area, buf);
    }

    fn draw_volumes(&self, area: Rect, buf: &mut Buffer) {
        let selected_project = &self.projects[self.selected_project_index];
        let selected_backups: Vec<_> = self
            .backups
            .iter()
            .filter(|b| &b.name == selected_project)
            .collect();

        let mut volume_lines: Vec<Line> =
            if let Some(selected_backup) = selected_backups.get(self.selected_date_index) {
                selected_backup
                    .volumes
                    .iter()
                    .enumerate()
                    .map(|(i, volume)| {
                        let style = if self.selected_column == Column::Volumes
                            && i == self.selected_volume_index
                        {
                            Style::default().add_modifier(ratatui::style::Modifier::REVERSED)
                        } else {
                            Style::default()
                        };
                        let checkbox = if self.selected_volumes.contains(&volume.name) {
                            "[x] "
                        } else {
                            "[ ] "
                        };
                        Line::from(format!("{}{}", checkbox, volume.name)).style(style)
                    })
                    .collect()
            } else {
                Vec::new()
            };

        let repo_index = selected_backups
            .get(self.selected_date_index)
            .map_or(0, |b| b.volumes.len());
        let repo_style = if self.selected_column == Column::Volumes
            && self.selected_volume_index == repo_index
        {
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
            .render(area, buf);
    }

    fn draw_summary(&self, area: Rect, buf: &mut Buffer) {
        let summary_text = format!(
            "Selected Project: {}\nSelected Backup: {}\nSelected Volume: {}",
            self.projects[self.selected_project_index],
            self.backups[self.selected_date_index].timestamp,
            self.backups[self.selected_date_index]
                .volumes
                .get(self.selected_volume_index)
                .map_or("None".to_string(), |v| v.name.clone())
        );

        Paragraph::new(Text::from(summary_text))
            .block(
                Block::default()
                    .title("Summary")
                    .borders(ratatui::widgets::Borders::ALL),
            )
            .render(area, buf);
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

fn get_projects(backups: &[BackupApplication]) -> Vec<String> {
    let mut projects = HashSet::new();
    for backup in backups {
        projects.insert(backup.name.clone());
    }
    let mut projects: Vec<String> = projects.into_iter().collect();
    projects.sort();
    projects
}
fn get_backups(backups: &[BackupApplication], project: &str) -> Vec<BackupApplication> {
    let mut backups: Vec<BackupApplication> = backups
        .iter()
        .filter(|backup| backup.name == project)
        .cloned()
        .collect();
    backups.sort_by(|a, b| a.timestamp.timestamp().cmp(&b.timestamp.timestamp()));

    backups.reverse();
    backups
}
fn get_volumes(backups: &[BackupApplication], project: &str) -> Vec<String> {
    backups
        .iter()
        .filter(|backup| backup.name == project)
        .flat_map(|backup| backup.volumes.iter().map(|v| v.name.clone()))
        .collect()
}
fn style_selected(list: &Vec<String>, selected_index: usize, home_column: bool) -> Vec<Line> {
    list.iter()
        .enumerate()
        .map(|(i, item)| {
            let style = if i == selected_index && home_column {
                Style::default().add_modifier(ratatui::style::Modifier::REVERSED)
            } else if i == selected_index {
                Style::default().add_modifier(ratatui::style::Modifier::UNDERLINED)
            } else {
                Style::default()
            };
            Line::from(item.clone()).style(style)
        })
        .collect()
}
