mod backup;
mod config;
mod email;
mod scanner;

use clap::{Parser, Subcommand};

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
    Scan,
    Backup,
    DryRun,
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    View,
    Set {
        #[arg(long)]
        key: String,
        #[arg(long)]
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
