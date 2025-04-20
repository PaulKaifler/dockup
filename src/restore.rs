use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Paragraph, Widget},
    DefaultTerminal, Frame,
};

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
    app.run(&mut terminal)
}

pub struct RestoreApp {
    backups: Vec<BackupApplication>,
    exit: bool,
}

impl RestoreApp {
    pub async fn new(config: &Config) -> Self {
        let backups = scan_backup_target(config).await.unwrap_or_else(|e| {
            eprintln!("❌ Error scanning backup target: {e}");
            Vec::new()
        });
        Self {
            backups,
            exit: false,
        }
    }
}

impl RestoreApp {
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        };
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
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
    let mut backups = Vec::new();
    let listing =
        run_remote_cmd_with_output(config, &format!("ls -1 {}", config.remote_backup_path))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    for dir in listing.lines() {
        let meta = run_remote_cmd_with_output(
            config,
            &format!(
                "cat {}/{}/mdb_dev_meta.json || cat {}/{}/meta.json",
                config.remote_backup_path, dir, config.remote_backup_path, dir,
            ),
        );

        if let Ok(json) = meta {
            if let Ok(mut app) = serde_json::from_str::<BackupApplication>(&json) {
                // (Optional) fallback timestamp
                if app.timestamp.timestamp() == 0 {
                    if let Ok(parsed) = chrono::DateTime::parse_from_str(dir, "%Y_%m_%d_%H%M%S") {
                        app.timestamp = parsed.with_timezone(&chrono::Local);
                    }
                }
                backups.push(app);
            }
        }
    }
    Ok(backups)
}
