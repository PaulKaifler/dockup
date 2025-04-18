mod backup;
mod config;
mod email;
mod scanner;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "Dockup",
    version,
    author,
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cfg = config::Config::load_or_create()?;

    match cli.command {
        Commands::Scan => {
            scanner::scan_projects(&cfg)?;
        }
        Commands::Backup => {
            let result = backup::run_backup(&cfg);
            match &result {
                Ok(_) => {
                    email::send_summary_email(
                        &cfg,
                        "Dockup Backup Report",
                        "All projects backed up successfully.",
                    )
                    .await?;
                }
                Err(e) => {
                    let msg = format!("Backup encountered an error:\n{e}");
                    email::send_summary_email(&cfg, "Dockup Backup Report", &msg).await?;
                }
            }
            result?
        }
        Commands::DryRun => backup::dry_run(&cfg)?,
        Commands::Config { action } => match action {
            ConfigAction::View => println!("{:#?}", cfg),
            ConfigAction::Set { key, value } => {
                let mut cfg = cfg;
                cfg.set_key_value(&key, &value)?;
                cfg.save()?;
                println!("Updated config key `{key}` to `{value}`");
            }
        },
    }

    Ok(())
}
