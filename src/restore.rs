// restore.rs

use std::io::{self, Write};

use crate::config::Config;

pub struct BackupApplication {
    pub name: String,
    pub path: String,
    pub volumes: Vec<String>,
}

pub fn handle_restore_command(
    config: &Config,
    project: Option<String>,
    version: Option<String>,
    repo: bool,
    volumes: Vec<String>,
) {
    let no_args_provided = project.is_none() && version.is_none() && !repo && volumes.is_empty();

    if no_args_provided {
        println!("No options supplied, starting interactive shell.");
        enter_interactive_shell();
    } else {
        let project = match project {
            Some(p) => p,
            None => {
                eprintln!("Project name is required when using non-interactive mode.");
                return;
            }
        };

        let version = version.unwrap_or_else(|| {
            println!("No version specified, using latest.");
            // placeholder: insert logic to find latest version if desired
            "latest".into()
        });

        if repo {
            println!(
                "Restoring repo for project '{}', version '{}'",
                project, version
            );
            // backup::restore_repo(&project, &version);
        }

        if !volumes.is_empty() {
            println!(
                "Restoring volumes {:?} for project '{}', version '{}'",
                volumes, project, version
            );
            // backup::restore_volumes(&project, &version, &volumes);
        }
    }
}

fn enter_interactive_shell() {
    // basic interactive shell logic here
    println!("Interactive shell here. Type 'help' for commands.");
}

fn list_backups() {
    // placeholder: insert logic to list backups
    println!("Listing backups...");
}
