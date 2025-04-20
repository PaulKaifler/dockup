mod backup;
mod config;
mod email;
mod logger;
mod restore;
mod scanner;
mod utils;

use clap::CommandFactory;
use clap::{Parser, Subcommand};
use clap_complete::{generate, Shell};
use std::fs;
use std::io::Write;

#[derive(Parser)]
#[command(
    name = "Dockup",
    version = "0.1.0",
    author = "Paul Kaifler",
    about = "Automatic Docker backup CLI"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(
        about = "Scan for Docker projects",
        long_about = "Scans the specified directory for Docker projects.\n\nThis command will look for Dockerfiles and docker-compose files in the specified directory."
    )]
    Scan,

    #[command(
        about = "Backup all projects",
        long_about = "Performs a backup of all projects.\nUploads the backup to the remote server.\n\nThis command will create a tarball of each project and upload it to the specified remote location."
    )]
    Backup {
        #[arg(short, help = "Mark as scheduled backup")]
        s: bool,
    },

    #[command(
        about = "Dry run without actual backup",
        long_about = "Performs a dry run of the backup process.\nNo data will be written or transferred.\nUseful for testing and validation."
    )]
    DryRun,

    #[command(
        about = "Restore a specific project",
        long_about = "Restores a specific project from backup.\n\nChoose a project to restore from the backup.\nYou can select between different backup versions and what parts of the project to restore."
    )]
    Restore {
        #[arg(long, help = "The name of the project to restore")]
        project: Option<String>,

        #[arg(
            long,
            help = "The version of the backup to restore (if omitted, latest version will be used)"
        )]
        version: Option<String>,

        #[arg(long, help = "Restore the repository")]
        repo: bool,

        #[arg(long, help = "The volumes to restore")]
        volumes: Vec<String>,
    },

    #[command(
        about = "Configure dockup",
        long_about = "Configure dockup settings.\n\nThis command allows you to view and modify the configuration settings for dockup."
    )]
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    #[command(
        about = "Configure dockup backup interval",
        long_about = "Configure dockup backup interval.\n\nThis command allows you to view and modify the backup interval settings for dockup."
    )]
    Interval {
        #[command(subcommand)]
        action: IntervalAction,
    },

    #[command(
        about = "Setup shell completion",
        long_about = "Setup shell completion for dockup.\n\nThis command will generate a completion script for your shell.\n\nSupported shells: bash, zsh."
    )]
    SetupCompletion {
        #[arg(long, help = "The shell type for which to generate completion")]
        shell: Shell,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    #[command(
        about = "View current configuration",
        long_about = "View the current configuration settings.\n\nThis command will display the current configuration settings for dockup in JSON format."
    )]
    View,

    #[command(
        about = "Set one key-value pair in the configuration",
        long_about = "Change one setting in the configuration.\n\nThis command allows you to set a specific key-value pair in the configuration settings."
    )]
    Set {
        #[arg(long, help = "The configuration key to set")]
        key: String,
        #[arg(long, help = "The value to set for the configuration key")]
        value: String,
    },

    #[command(
        about = "Test the current configuration",
        long_about = "Test the current configuration settings.\n\nThis command will test the SSH and email configuration settings to ensure they are valid.\n\nIf you don't receive an email, maybe look into your spam."
    )]
    Test,
}

#[derive(Subcommand)]
enum IntervalAction {
    #[command(
        about = "View current backup interval",
        long_about = "View the current backup interval settings.\n\nThis command will display the current backup interval settings for dockup."
    )]
    View,

    #[command(
        about = "Set backup interval",
        long_about = "Set the backup interval settings.\n\nThis command allows you to set the backup interval settings for dockup."
    )]
    Set {
        #[arg(long, help = "The configuration key to set")]
        key: String,
        #[arg(long, help = "The value to set for the configuration key")]
        value: String,
    },

    #[command(
        about = "Reset backup interval to default",
        long_about = "Reset the backup interval settings to default values.\n\nThis command will reset the backup interval settings to their default values."
    )]
    Reset,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut cfg = config::Config::load_or_create().await?;
    logger::init();

    match cli.command {
        Commands::Scan => {
            scanner::scan_projects(&cfg)?;
        }
        Commands::Backup { s } => {
            let result = backup::run_backup(&cfg, s);
            match &result {
                Ok(summaries) => {
                    let mut total_backups = 0;
                    let mut total_duration = 0.0;
                    let mut total_size = 0.0;
                    let mut summary_messages = String::new();
                    for summary in summaries {
                        let mut app_duration = 0.0;
                        let mut app_size = 0.0;
                        for vol in &summary.volume_statuses {
                            total_backups += 1;
                            if let Some(dur_str) = vol.duration.strip_suffix(" seconds") {
                                if let Ok(dur) = dur_str.parse::<f64>() {
                                    total_duration += dur;
                                    app_duration += dur;
                                }
                            }
                            let raw_size = vol.size.trim();
                            let (value_part, _unit) = raw_size
                                .chars()
                                .partition::<String, _>(|c| c.is_ascii_digit() || *c == '.');

                            if let Ok(raw) = value_part.parse::<f64>() {
                                let multiplier = if raw_size.contains("KB") {
                                    1_000.0
                                } else if raw_size.contains("MB") {
                                    1_000_000.0
                                } else if raw_size.contains("GB") {
                                    1_000_000_000.0
                                } else if raw_size.contains("B") {
                                    1.0
                                } else {
                                    1.0
                                };

                                let actual_size = raw * multiplier;
                                total_size += actual_size;
                                app_size += actual_size;
                            }
                        }
                        summary_messages.push_str(&format!(
                            "<h2>{}</h2> <p>Duration: {:.2} seconds, Size: {:.2} bytes</p>",
                            summary.name, app_duration, app_size
                        ));
                        summary_messages.push_str("<table border=\"1\" cellpadding=\"8\" cellspacing=\"0\" style=\"border-collapse: collapse; font-family: sans-serif; font-size: 14px;\"><tr style=\"background-color: #f2f2f2;\"><th>Name</th><th>Status</th><th>Type</th><th>Size</th><th>Duration</th></tr>");
                        for vol in &summary.volume_statuses {
                            summary_messages.push_str(&format!(
                                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                                vol.name, vol.status, vol.volume_type, vol.size, vol.duration
                            ));
                        }
                        summary_messages.push_str("</table>");
                    }
                    let summary_line = format!(
                        "<p>Total Backups: {} - Total Duration: {:.2} seconds - Total Size: {:.2} bytes</p>",
                        total_backups, total_duration, total_size
                    );
                    let final_message = format!("{}{}", summary_line, summary_messages);
                    email::send_summary_email(&cfg, "Dockup Backup Report", &final_message).await?;
                }
                Err(e) => {
                    let msg = format!("Backup encountered an error:\n{e}");
                    email::send_summary_email(&cfg, "Dockup Backup Report", &msg).await?;
                }
            }
            result?;
        }
        Commands::DryRun => backup::dry_run(&cfg)?,
        Commands::Restore {
            project,
            version,
            repo,
            volumes,
        } => {
            restore::handle_restore_command(&cfg, project, version, repo, volumes);
        }
        Commands::SetupCompletion { shell } => {
            let _path = match shell {
                Shell::Zsh => {
                    let path = dirs::home_dir().unwrap().join(".zfunc").join("_dockup");
                    fs::create_dir_all(path.parent().unwrap())?;
                    let mut file = fs::File::create(&path)?;
                    generate(shell, &mut Cli::command(), "dockup", &mut file);
                    log::info!("Completion script installed to: {}", path.display());
                    println!(
                        "👉 Add this to your ~/.zshrc if not already there:\n\n  fpath+=~/.zfunc\n  autoload -Uz compinit && compinit\n"
                    );
                    println!(
                        "Do you want to automatically add the setup to your shell config? (y/n):"
                    );
                    let mut answer = String::new();
                    std::io::stdin().read_line(&mut answer)?;
                    if answer.trim() == "y" {
                        let zshrc = dirs::home_dir().unwrap().join(".zshrc");
                        let snippet = "fpath+=~/.zfunc\nautoload -Uz compinit && compinit";
                        let contents = fs::read_to_string(&zshrc).unwrap_or_default();
                        if !contents.contains(snippet) {
                            let mut file = fs::OpenOptions::new().append(true).open(&zshrc)?;
                            writeln!(file, "\n{}", snippet)?;
                            log::info!("✅ Added completion setup to {}", zshrc.display());
                        }
                    }
                    path
                }
                Shell::Bash => {
                    let path = dirs::home_dir()
                        .unwrap()
                        .join(".dockup")
                        .join("dockup.bash");
                    fs::create_dir_all(path.parent().unwrap())?;
                    let mut file = fs::File::create(&path)?;
                    generate(shell, &mut Cli::command(), "dockup", &mut file);
                    log::info!("✅ Bash completion written to: {}", path.display());
                    println!(
                        "👉 Add this to your ~/.bashrc:\n\n  source {}\n",
                        path.display()
                    );
                    println!(
                        "Do you want to automatically add the setup to your shell config? (y/n):"
                    );
                    let mut answer = String::new();
                    std::io::stdin().read_line(&mut answer)?;
                    if answer.trim() == "y" {
                        let bashrc = dirs::home_dir().unwrap().join(".bashrc");
                        let snippet = format!("source {}", path.display());
                        let contents = fs::read_to_string(&bashrc).unwrap_or_default();
                        if !contents.contains(&snippet) {
                            let mut file = fs::OpenOptions::new().append(true).open(&bashrc)?;
                            writeln!(file, "\n{}", snippet)?;
                            log::info!("✅ Added completion setup to {}", bashrc.display());
                        }
                    }
                    path
                }
                _ => {
                    log::error!("❌ Completion setup for {:?} not supported yet.", shell);
                    return Ok(());
                }
            };
        }
        Commands::Interval { action } => match action {
            IntervalAction::View => {
                let interval = cfg.cron_human_summary();
                println!("{}", interval);
            }
            IntervalAction::Set { key, value } => {
                let mut cfg = cfg;
                cfg.set_key_value(&key, &value)?;
                cfg.save()?;
                log::info!("Updated backup interval key `{key}` to `{value}`");
            }
            IntervalAction::Reset => {
                cfg.reset_interval_to_default()?;
            }
        },
        Commands::Config { action } => match action {
            ConfigAction::View => println!("{:#?}", cfg),
            ConfigAction::Set { key, value } => {
                let mut cfg = cfg;
                cfg.set_key_value(&key, &value)?;
                cfg.save()?;
                log::info!("Updated config key `{key}` to `{value}`");
                println!("Do you want to test the new configuration? (y/n):");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if input.trim() == "y" {
                    cfg.test_ssh().await?;
                    cfg.test_email().await?;
                }
            }
            ConfigAction::Test => {
                cfg.test_ssh().await?;
                cfg.test_email().await?;
            }
        },
    }

    Ok(())
}
