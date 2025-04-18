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
                    let mut summary_messages = String::new();
                    for summary in summaries {
                        summary_messages
                            .push_str(&format!("<p><strong>{}</strong></p>", summary.name));
                        for volume_status in &summary.volume_statuses {
                            summary_messages.push_str(&format!("<li>{:?}</li>", volume_status));
                        }
                        summary_messages.push_str("</ul>");
                    }
                    email::send_summary_email(&cfg, "Dockup Backup Report", &summary_messages)
                        .await?;
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
