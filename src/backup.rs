use crate::{config::Config, scanner::scan_projects};
use anyhow::Result;
use chrono::Local;
use std::{fs, path::PathBuf, process::Command};

#[derive(Debug)]
pub struct BackupThingSummary {
    pub name: String,
    pub status: String,
    pub size: String,
    pub duration: String,
}

pub struct AppSummary {
    pub name: String,
    pub volume_statuses: Vec<BackupThingSummary>,
}

pub fn run_backup(config: &Config) -> Result<Vec<AppSummary>> {
    let apps = scan_projects(config)?;
    let timestamp = Local::now().format("%Y_%m_%d_%H%M").to_string();
    let mut summaries: Vec<AppSummary> = Vec::new();

    for app in apps {
        println!("\n🗂 Backing up: {}", app.name);
        let mut volume_statuses = Vec::new();
        let remote_base = format!("{}/{}/{}", config.remote_backup_path, app.name, timestamp);
        run_remote_cmd(
            config,
            &format!("mkdir -p {}/REPO {}/VOLUMES", remote_base, remote_base),
        )?;

        let mut created_files: Vec<PathBuf> = Vec::new();
        let start_repo_time = Local::now();
        let repo_tar = create_tar(&app.path, "repo.tar.gz")?;
        created_files.push(repo_tar.clone());

        if let Err(e) = scp_upload(
            config,
            &repo_tar,
            &format!("{}/REPO/repo.tar.gz", remote_base),
        ) {
            eprintln!("❌ Failed to upload repo tarball: {e}");
        } else {
            let repo_size = get_file_size(&repo_tar)?;
            let duration = format!(
                "{:.2} seconds",
                (Local::now().timestamp_millis() - start_repo_time.timestamp_millis()) as f64
                    / 1000.0
            );
            let repo_size_str = format!("{}", repo_size);
            let repo_summary = BackupThingSummary {
                name: "REPO".to_string(),
                status: "✅".to_string(),
                size: repo_size_str,
                duration,
            };
            volume_statuses.push(repo_summary);
        }

        for vol in &app.volumes {
            let compose_project_volume_name = format!("{}_{}", app.name, vol);
            let start_volume_time = Local::now();
            let _volume_status = if let Err(e) =
                create_volume_tar(&compose_project_volume_name, &format!("{vol}.tar.gz"))
            {
                format!("❌ Failed to create tarball for volume `{}`: {e}", vol)
            } else if let Err(e) = scp_upload(
                config,
                &PathBuf::from(format!("/tmp/{}", vol)).with_extension("tar.gz"),
                &format!("{}/VOLUMES/{}", remote_base, format!("{vol}.tar.gz")),
            ) {
                format!("❌ Failed to upload volume `{}`: {e}", vol)
            } else {
                let volume_size = get_file_size(
                    &PathBuf::from(format!("/tmp/{}", vol)).with_extension("tar.gz"),
                )?;
                let duration = format!(
                    "{:.2} seconds",
                    (Local::now().timestamp_millis() - start_volume_time.timestamp_millis()) as f64
                        / 1000.0
                );
                volume_statuses.push(BackupThingSummary {
                    name: vol.to_string(),
                    status: format!("✅ {}", vol),
                    size: volume_size,
                    duration,
                });
                format!("✅ {}", vol)
            };
        }

        summaries.push(AppSummary {
            name: app.name.clone(),
            volume_statuses,
        });

        for f in created_files {
            if let Err(e) = fs::remove_file(&f) {
                eprintln!("⚠️  Failed to delete temp file {:?}: {e}", f);
            } else {
                println!("🧹 Deleted temp file {:?}", f);
            }
        }
    }
    Ok(summaries)
}

pub fn dry_run(config: &Config) -> Result<()> {
    let apps = scan_projects(config)?;
    let timestamp = Local::now().format("%Y%m%d_%H%M").to_string();

    for app in apps {
        println!("\n🚧 Dry run: {}", app.name);
        println!(
            "  Would create remote folder: {}/{}/{}",
            config.remote_backup_path, app.name, timestamp
        );
        println!("  Would archive: {:?}", app.path);
        for vol in &app.volumes {
            println!("  Would archive volume: {}", vol);
        }
    }

    Ok(())
}

fn create_tar(src: &PathBuf, output: &str) -> Result<PathBuf> {
    let output_path = PathBuf::from("/tmp").join(output);
    let status = Command::new("tar")
        .args([
            "-czf",
            output_path.to_str().unwrap(),
            "-C",
            src.to_str().unwrap(),
            ".",
        ])
        .status()?;
    if !status.success() {
        anyhow::bail!("Failed to create tarball: {:?}", output_path);
    }
    Ok(output_path)
}

fn create_volume_tar(volume: &str, tar_name: &str) -> Result<PathBuf> {
    let output_path = PathBuf::from("/tmp").join(tar_name);

    let status = Command::new("docker")
        .args([
            "run",
            "--rm",
            "-v",
            &format!("{}:/data", volume),
            "-v",
            "/tmp:/backup",
            "alpine",
            "sh",
            "-c",
            &format!("tar -czf /backup/{} -C /data .", tar_name),
        ])
        .status()?;

    if !status.success() {
        anyhow::bail!("Failed to create tarball for volume: {}", volume);
    }

    Ok(output_path)
}

fn get_file_size(path: &PathBuf) -> Result<String> {
    let output = Command::new("du")
        .args(["-sh", path.to_str().unwrap()])
        .output()?;
    if !output.status.success() {
        anyhow::bail!("Failed to get file size for: {:?}", path);
    }
    let size_str = String::from_utf8_lossy(&output.stdout);
    let size = size_str.split_whitespace().next().unwrap_or("0");
    Ok(size.to_string())
}

fn run_remote_cmd(cfg: &Config, cmd: &str) -> Result<()> {
    let full_cmd = format!(
        "ssh -i {} {}@{} '{}'",
        cfg.ssh_key, cfg.ssh_user, cfg.ssh_host, cmd
    );
    let status = Command::new("sh").arg("-c").arg(full_cmd).status()?;
    if !status.success() {
        anyhow::bail!("SSH command failed: {}", cmd);
    }
    Ok(())
}

fn scp_upload(cfg: &Config, local: &PathBuf, remote_path: &str) -> Result<()> {
    let remote = format!("{}@{}:{}", cfg.ssh_user, cfg.ssh_host, remote_path);
    let status = Command::new("scp")
        .args(["-i", &cfg.ssh_key, local.to_str().unwrap(), &remote])
        .status()?;
    if !status.success() {
        anyhow::bail!("SCP upload failed: {:?}", local);
    }
    Ok(())
}
