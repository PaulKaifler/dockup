mod backup;
mod config;
mod email;
mod scanner;

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
    Backup,

    #[command(
        about = "Dry run without actual backup",
        long_about = "Performs a dry run of the backup process.\nNo data will be written or transferred.\nUseful for testing and validation."
    )]
    DryRun,

    #[command(
        about = "Configure dockup",
        long_about = "Configure dockup settings.\n\nThis command allows you to view and modify the configuration settings for dockup."
    )]
    Config {
        #[command(subcommand)]
        action: ConfigAction,
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
    Test,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cfg = config::Config::load_or_create().await?;

    match cli.command {
        Commands::Scan => {
            scanner::scan_projects(&cfg)?;
        }
        Commands::Backup => {
            let result = backup::run_backup(&cfg);
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
                            if let Some(size_part) = vol.size.split_whitespace().next() {
                                if let Ok(sz) = size_part.parse::<f64>() {
                                    total_size += sz;
                                    app_size += sz;
                                }
                            }
                        }
                        summary_messages.push_str(&format!(
                            "<h2>{}</h2> <p>Duration: {:.2} seconds, Size: {:.2} bytes</p>",
                            summary.name, app_duration, app_size
                        ));
                        summary_messages.push_str("<table border=\"1\"><tr><th>Name</th><th>Status</th><th>Size</th><th>Duration</th></tr>");
                        for vol in &summary.volume_statuses {
                            summary_messages.push_str(&format!(
                                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                                vol.name, vol.status, vol.size, vol.duration
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
        Commands::SetupCompletion { shell } => {
            let path = match shell {
                Shell::Zsh => {
                    let path = dirs::home_dir().unwrap().join(".zfunc").join("_dockup");
                    fs::create_dir_all(path.parent().unwrap())?;
                    let mut file = fs::File::create(&path)?;
                    generate(shell, &mut Cli::command(), "dockup", &mut file);
                    println!("✅ Completion script installed to: {}", path.display());
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
                            println!("✅ Added completion setup to {}", zshrc.display());
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
                    println!("✅ Bash completion written to: {}", path.display());
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
                            println!("✅ Added completion setup to {}", bashrc.display());
                        }
                    }
                    path
                }
                _ => {
                    println!("❌ Completion setup for {:?} not supported yet.", shell);
                    return Ok(());
                }
            };
        }
        Commands::Config { action } => match action {
            ConfigAction::View => println!("{:#?}", cfg),
            ConfigAction::Set { key, value } => {
                let mut cfg = cfg;
                cfg.set_key_value(&key, &value)?;
                cfg.save()?;
                println!("Updated config key `{key}` to `{value}`");
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
