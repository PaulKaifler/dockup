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

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn enter_interactive_shell(config: &Config) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let mut app = futures::executor::block_on(RestoreApp::new(&config));

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

pub struct RestoreApp<'a> {
    config: Config,
    backups: Vec<BackupApplication>,
    projects: Vec<String>,
    exit: bool,
    selected_project_index: usize,
    selected_date_index: usize,
    selected_volume_index: usize,
    selected_column: Column,
    selected_volumes: HashSet<String>,
    toggled_repo: bool,
    show_help: bool,
    restore_message: Vec<Line<'a>>,
    show_restore_popup: bool,
}

#[derive(PartialEq)]
enum Column {
    Projects,
    Dates,
    Volumes,
}

impl<'a> RestoreApp<'a> {
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
            config: config.clone(),
            backups,
            projects,
            exit: false,
            selected_project_index: 0,
            selected_date_index: 0,
            selected_volume_index: 0,
            selected_column: Column::Projects,
            selected_volumes: HashSet::new(),
            toggled_repo: false,
            show_help: false,
            restore_message: Vec::new(),
            show_restore_popup: false,
        }
    }
}

impl<'a> RestoreApp<'a> {
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
            .constraints(vec![
                Constraint::Min(5),
                Constraint::Length(5),
                Constraint::Length(1),
            ])
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
        self.draw_dates(chunk[1], frame.buffer_mut());
        self.draw_volumes(chunk[2], frame.buffer_mut());
        self.draw_summary(layout[1], frame.buffer_mut());
        self.draw_tooltip(layout[2], frame.buffer_mut());
        if self.show_help {
            let area = centered_rect(60, 20, frame.area());
            use ratatui::widgets::Clear;
            Clear.render(area, frame.buffer_mut());
            self.draw_floating_help(area, frame.buffer_mut());
        }
        if self.show_restore_popup {
            let popup = centered_rect(60, 30, frame.area());
            // clear any background behind it
            ratatui::widgets::Clear.render(popup, frame.buffer_mut());
            Paragraph::new(Text::from(self.restore_message.clone()))
                .block(
                    Block::default()
                        .title("Restore")
                        .borders(ratatui::widgets::Borders::ALL),
                )
                .render(popup, frame.buffer_mut());
        }
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
        if key_event.code == KeyCode::Char('h') {
            self.show_help = !self.show_help;
            return;
        }
        match key_event.code {
            KeyCode::Char('a') => {
                self.selected_volumes = HashSet::new();
                let volumes = get_volumes(
                    get_backups(&self.backups, &self.projects[self.selected_project_index])
                        [self.selected_date_index]
                        .clone(),
                );
                for volume in volumes {
                    self.selected_volumes.insert(volume);
                }
            }
            KeyCode::Char('d') => {
                self.selected_volumes = HashSet::new();
            }
            KeyCode::Enter => {
                if self.show_restore_popup {
                    self.start_restore_process();
                } else {
                    self.restore_selection();
                }
            }
            _ => {}
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
                    let available_dates =
                        get_backups(&self.backups, &self.projects[self.selected_project_index])
                            .len();
                    if self.selected_date_index < available_dates - 1 {
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
                    let available_volumes = get_volumes(
                        get_backups(&self.backups, &self.projects[self.selected_project_index])
                            [self.selected_date_index]
                            .clone(),
                    )
                    .len();
                    if self.selected_volume_index < available_volumes - 1 {
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
                    let selected_volume = get_volumes(
                        get_backups(&self.backups, &self.projects[self.selected_project_index])
                            [self.selected_date_index]
                            .clone(),
                    )[self.selected_volume_index]
                        .clone();
                    if selected_volume == "REPO" {
                        self.toggled_repo = !self.toggled_repo;
                    }
                    if self.selected_volumes.contains(&selected_volume) {
                        self.selected_volumes.remove(&selected_volume);
                    } else {
                        self.selected_volumes.insert(selected_volume);
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

    fn draw_dates(&self, area: Rect, buf: &mut Buffer) {
        let dates = get_backups(&self.backups, &self.projects[self.selected_project_index]);
        let binding = dates
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
        let volumes = get_volumes(
            get_backups(&self.backups, &self.projects[self.selected_project_index])
                [self.selected_date_index]
                .clone(),
        );
        let volume_names: Vec<Line> = style_checkboxes(
            &volumes,
            self.selected_volume_index,
            &self.selected_volumes,
            self.selected_column == Column::Volumes,
        );
        Paragraph::new(Text::from(volume_names))
            .block(
                Block::default()
                    .title("Volumes")
                    .borders(ratatui::widgets::Borders::ALL),
            )
            .render(area, buf);
    }

    fn draw_summary(&self, area: Rect, buf: &mut Buffer) {
        let summary_text = format!(
            "Selected Project:   {}\nSelected Backup:    {}\nSelected Volume(s): {}",
            self.projects[self.selected_project_index],
            get_backups(&self.backups, &self.projects[self.selected_project_index])
                [self.selected_date_index]
                .timestamp
                .format("%d. %B %Y %H:%M:%S"),
            self.selected_volumes
                .iter()
                .cloned()
                .collect::<Vec<String>>()
                .join(", ")
        );

        Paragraph::new(Text::from(summary_text))
            .block(
                Block::default()
                    .title("Summary")
                    .borders(ratatui::widgets::Borders::ALL),
            )
            .render(area, buf);
    }

    fn draw_tooltip(&self, layout: Rect, buf: &mut Buffer) {
        let tooltip_text = " (q)uit | (h)elp | (space) select | (up) | (down) | (left) | (right) ";
        let paragraph =
            Paragraph::new(tooltip_text.blue().bold()).wrap(ratatui::widgets::Wrap { trim: false });
        paragraph.render(layout, buf);
    }

    fn draw_floating_help(&self, area: Rect, buf: &mut Buffer) {
        let text = Text::from(vec![
            Line::from("← →: switch column"),
            Line::from("↑ ↓: navigate"),
            Line::from("SPACE: select volume"),
            Line::from("ENTER: restore"),
            Line::from("a: select all    d: deselect all"),
            Line::from("q: quit"),
            Line::from("h: toggle help"),
        ]);
        Paragraph::new(text)
            .block(
                Block::default()
                    .title("Help")
                    .borders(ratatui::widgets::Borders::ALL)
                    .style(Style::default().bg(ratatui::style::Color::White)),
            )
            .render(area, buf);
    }

    fn restore_selection(&mut self) {
        let project = &self.projects[self.selected_project_index];
        let backup = get_backups(&self.backups, project)[self.selected_date_index].clone();

        let vols: Vec<String> = self.selected_volumes.iter().cloned().collect();
        let repo = vols.contains(&"REPO".to_string());
        let actual = vols.into_iter().filter(|v| v != "REPO").collect::<Vec<_>>();

        let mut lines = Vec::new();
        lines.push(Line::from(format!("🔁 Restoring project: {}", project)));
        lines.push(Line::from(format!(
            "📅 Date: {}",
            backup.timestamp.format("%d. %B %Y %H:%M:%S")
        )));
        lines.push(Line::from(format!("📦 Volumes: {}", actual.join(", "))));
        lines.push(Line::from(format!(
            "📁 Repo: {}",
            if repo { "yes" } else { "no" }
        )));

        self.restore_message = lines;
        self.show_restore_popup = true;
    }
}

impl<'a> Widget for &'a RestoreApp<'a> {
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
fn get_volumes(backup: BackupApplication) -> Vec<String> {
    let mut volumes = HashSet::new();
    for volume in backup.volumes {
        volumes.insert(volume.name);
    }
    let mut volumes: Vec<String> = volumes.into_iter().collect();
    volumes.sort();
    volumes.push("REPO".to_string());
    volumes
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
fn style_checkboxes<'a>(
    list: &'a Vec<String>,
    selected_index: usize,
    selected_volumes: &'a HashSet<String>,
    home_column: bool,
) -> Vec<Line<'a>> {
    list.iter()
        .enumerate()
        .map(|(i, item)| {
            let style = if i == selected_index && home_column {
                Style::default().add_modifier(ratatui::style::Modifier::REVERSED)
            } else {
                Style::default()
            };
            let checkbox = if selected_volumes.contains(item) {
                "[x] "
            } else {
                "[ ] "
            };
            Line::from(format!("{}{}", checkbox, item)).style(style)
        })
        .collect()
}

use std::{fs, process::Command};

impl<'a> RestoreApp<'a> {
    /// Kick off the actual scp/tar restore now that user has confirmed.
    fn start_restore_process(&mut self) -> io::Result<()> {
        let project = &self.projects[self.selected_project_index];
        let backups = get_backups(&self.backups, project);
        let backup = &backups[self.selected_date_index];

        // Folder name matches folder on the server
        let folder = backup.timestamp.format("%Y_%m_%d_%H%M%S").to_string();
        let remote_base = format!(
            "{}/{}/{}",
            self.config.remote_backup_path, backup.name, folder
        );

        // Build list: volumes + "REPO" if toggled
        let mut items: Vec<String> = self.selected_volumes.iter().cloned().collect();
        if self.toggled_repo && !items.contains(&"REPO".into()) {
            items.push("REPO".into());
        }

        for name in items {
            if name == "REPO" {
                let remote = format!("{}/REPO/repo.tar.gz", remote_base);
                let tmp = std::env::temp_dir().join("repo.tar.gz");

                // Download
                let status = Command::new("scp")
                    .args(&[
                        "-i",
                        &self.config.ssh_key,
                        "-P",
                        &self.config.ssh_port.to_string(),
                        &format!(
                            "{}@{}:{}",
                            self.config.ssh_user, self.config.ssh_host, remote
                        ),
                        tmp.to_str().unwrap(),
                    ])
                    .status()?;
                if !status.success() {
                    self.restore_message
                        .push(Line::from("⚠️ failed to scp repo"));
                    continue;
                }

                // Extract
                let dest = &backup.application_path;
                fs::remove_dir_all(dest).ok();
                fs::create_dir_all(dest)?;
                let status = Command::new("tar")
                    .args(&["-xzf", tmp.to_str().unwrap(), "-C", dest.to_str().unwrap()])
                    .status()?;
                if status.success() {
                    self.restore_message.push(Line::from("✅ repo restored"));
                } else {
                    self.restore_message
                        .push(Line::from("⚠️ repo extract failed"));
                }
            } else {
                // Find Volume entry
                if let Some(v) = backup.volumes.iter().find(|v| &v.name == &name) {
                    // remote tarball path uses underscores for slashes
                    let tarname = format!("{}.tar.gz", v.path.to_string_lossy().replace('/', "_"));
                    let remote = format!("{}/VOLUMES/{}", remote_base, tarname);
                    let tmp = std::env::temp_dir().join(&tarname);

                    let status = Command::new("scp")
                        .args(&[
                            "-i",
                            &self.config.ssh_key,
                            "-P",
                            &self.config.ssh_port.to_string(),
                            &format!(
                                "{}@{}:{}",
                                self.config.ssh_user, self.config.ssh_host, remote
                            ),
                            tmp.to_str().unwrap(),
                        ])
                        .status()?;
                    if !status.success() {
                        self.restore_message
                            .push(Line::from(format!("⚠️ failed scp {}", name)));
                        continue;
                    }

                    // destroy and recreate target
                    let dest = &v.path;
                    fs::remove_dir_all(dest).ok();
                    fs::create_dir_all(dest)?;
                    // extract
                    let status = Command::new("tar")
                        .args(&["-xzf", tmp.to_str().unwrap(), "-C", dest.to_str().unwrap()])
                        .status()?;
                    if status.success() {
                        self.restore_message
                            .push(Line::from(format!("✅ {}", name)));
                    } else {
                        self.restore_message
                            .push(Line::from(format!("⚠️ extract {}", name)));
                    }
                }
            }
        }

        // keep popup visible so user sees the messages
        Ok(())
    }
}
