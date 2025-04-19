use crate::{
    config::Config,
    scanner::{scan_projects, BackupApplication},
};
use anyhow::Result;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, process::Command};

#[derive(Debug)]
pub struct BackupThingSummary {
    pub name: String,
    pub status: String,
    pub size: String,
    pub duration: String,
    pub volume_type: String,
}

pub struct AppSummary {
    pub name: String,
    pub volume_statuses: Vec<BackupThingSummary>,
}

pub fn run_backup(config: &Config) -> Result<Vec<AppSummary>> {
    let apps = scan_projects(config)?;
    println!("{:?}", apps);

    return Ok(vec![]);
    let timestamp = Local::now().format("%Y_%m_%d_%H%M").to_string();
    let mut summaries: Vec<AppSummary> = Vec::new();

    backup_config(config)?;

    for app in apps {
        log::info!("🗂  Backing up: {}", app.name);
        let mut volume_statuses = Vec::new();
        let remote_base = format!("{}/{}/{}", config.remote_backup_path, app.name, timestamp);
        run_remote_cmd(
            config,
            &format!("mkdir -p {}/REPO {}/VOLUMES", remote_base, remote_base),
        )?;

        let mut created_files: Vec<PathBuf> = Vec::new();
        let start_repo_time = Local::now();
        let repo_tar = create_tar(&app.application_path, "repo.tar.gz")?;
        created_files.push(repo_tar.clone());

        if let Err(e) = scp_upload(
            config,
            &repo_tar,
            &format!("{}/REPO/repo.tar.gz", remote_base),
        ) {
            log::error!("❌ Failed to upload repo tarball: {e}");
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
                volume_type: "Repo".to_string(),
            };
            volume_statuses.push(repo_summary);
        }

        for vol in &app.volumes {
            let start_volume_time = Local::now();
            let (_success, summary) = if vol.path.starts_with(".") || vol.path.starts_with("/") {
                // 🧱 Handle bind mount
                let abs_path = app.application_path.join(vol.path.clone()); // make it absolute
                let sanitized = vol
                    .path
                    .to_string_lossy()
                    .trim_start_matches("./")
                    .replace('/', "_");
                let tar_name = format!("{sanitized}.tar.gz");
                match create_tar(&abs_path, &tar_name) {
                    Err(e) => {
                        log::error!(
                            "❌ Failed to create tarball for bind mount `{}`: {}",
                            vol.name,
                            e
                        );
                        (
                            false,
                            BackupThingSummary {
                                name: vol.name.to_string(),
                                status: "❌ Failed to tar bind mount".into(),
                                size: "-".into(),
                                duration: "-".into(),
                                volume_type: "Bind".to_string(),
                            },
                        )
                    }
                    Ok(tar) => {
                        created_files.push(tar.clone());
                        let upload_res = scp_upload(
                            config,
                            &tar,
                            &format!(
                                "{}/VOLUMES/{}",
                                remote_base,
                                tar.file_name().unwrap().to_string_lossy()
                            ),
                        );
                        let duration = format!(
                            "{:.2} seconds",
                            (Local::now().timestamp_millis() - start_volume_time.timestamp_millis())
                                as f64
                                / 1000.0
                        );
                        if let Err(e) = upload_res {
                            log::error!("❌ Upload failed for bind mount `{}`: {}", vol.name, e);
                            (
                                false,
                                BackupThingSummary {
                                    name: vol.name.to_string(),
                                    status: "❌ Upload failed".into(),
                                    size: "-".into(),
                                    duration,
                                    volume_type: "Bind".to_string(),
                                },
                            )
                        } else {
                            let size = get_file_size(&tar)?;
                            log::info!("✅ Bind mount `{}` backed up", vol.name);
                            (
                                true,
                                BackupThingSummary {
                                    name: vol.name.to_string(),
                                    status: "✅".into(),
                                    size,
                                    duration,
                                    volume_type: "Bind".to_string(),
                                },
                            )
                        }
                    }
                }
            } else {
                // 📦 Handle Docker volume
                let docker_vol = format!("{}_{}", app.name, vol.name);
                let sanitized = vol
                    .path
                    .to_string_lossy()
                    .trim_start_matches("./")
                    .replace('/', "_");
                let tar_name = format!("{sanitized}.tar.gz");
                match create_volume_tar(&docker_vol, &tar_name) {
                    Err(e) => {
                        log::error!(
                            "❌ Failed to create Docker volume tarball `{}`: {}",
                            vol.name,
                            e
                        );
                        (
                            false,
                            BackupThingSummary {
                                name: vol.name.to_string(),
                                status: "❌ Failed to tar Docker volume".into(),
                                size: "-".into(),
                                duration: "-".into(),
                                volume_type: "Docker".to_string(),
                            },
                        )
                    }
                    Ok(tar) => {
                        created_files.push(tar.clone());
                        let upload_res = scp_upload(
                            config,
                            &tar,
                            &format!(
                                "{}/VOLUMES/{}",
                                remote_base,
                                tar.file_name().unwrap().to_string_lossy()
                            ),
                        );
                        let duration = format!(
                            "{:.2} seconds",
                            (Local::now().timestamp_millis() - start_volume_time.timestamp_millis())
                                as f64
                                / 1000.0
                        );
                        if let Err(e) = upload_res {
                            log::error!("❌ Upload failed for Docker volume `{}`: {}", vol.name, e);
                            (
                                false,
                                BackupThingSummary {
                                    name: vol.name.to_string(),
                                    status: "❌ Upload failed".into(),
                                    size: "-".into(),
                                    duration,
                                    volume_type: "Docker".to_string(),
                                },
                            )
                        } else {
                            let size = get_file_size(&tar)?;
                            log::info!("✅ Docker volume `{}` backed up", vol.name);
                            (
                                true,
                                BackupThingSummary {
                                    name: vol.name.to_string(),
                                    status: "✅".into(),
                                    size,
                                    duration,
                                    volume_type: "Docker".to_string(),
                                },
                            )
                        }
                    }
                }
            };

            volume_statuses.push(summary);
        }
        summaries.push(AppSummary {
            name: app.name.clone(),
            volume_statuses,
        });

        for f in created_files {
            if let Err(e) = fs::remove_file(&f) {
                log::warn!("⚠️  Failed to delete temp file {:?}: {e}", f);
            } else {
                log::info!("🧹 Deleted temp file {:?}", f);
            }
        }
    }
    Ok(summaries)
}

pub fn dry_run(config: &Config) -> Result<()> {
    let apps = scan_projects(config)?;
    let timestamp = Local::now().format("%Y%m%d_%H%M").to_string();

    log::info!("Starting dry run...");
    println!("\n🚧 Dry run: dockup config");
    println!(
        "   Would save dockup config to {}/config.json",
        config.remote_backup_path
    );

    for app in apps {
        println!("\n🚧 Dry run: {}", app.name);
        println!(
            "   Would create remote folder: {}/{}/{}",
            config.remote_backup_path, app.name, timestamp
        );
        println!("   Would archive: {:?}", app.application_path);
        for vol in &app.volumes {
            println!("   Would archive volume: {}", vol.name);
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
        "ssh -i {} -p {} {}@{} '{}'",
        cfg.ssh_key, cfg.ssh_port, cfg.ssh_user, cfg.ssh_host, cmd
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
        .args([
            "-i",
            &cfg.ssh_key,
            "-P",
            &cfg.ssh_port.to_string(),
            local.to_str().unwrap(),
            &remote,
        ])
        .status()?;
    if !status.success() {
        anyhow::bail!("SCP upload failed: {:?}", local);
    }
    Ok(())
}

// This saves the latest dockup config to the target location
fn backup_config(config: &Config) -> Result<()> {
    let config_path = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Failed to get home directory"))?
        .join(".dockup")
        .join("config.json");

    log::info!(
        "⚙️  Backing up config to: {}/config.json",
        config.remote_backup_path
    );
    if let Err(e) = scp_upload(
        config,
        &config_path,
        &format!("{}", config.remote_backup_path),
    ) {
        log::error!("❌ Failed to upload config file: {e}");
    }
    log::info!("✅ Config file uploaded successfully");

    Ok(())
}

pub fn save_metadata(backup: &BackupApplication) -> std::io::Result<()> {
    use std::fs;
    use std::io::Write;

    let meta_path = backup.application_path.join("meta.json");
    let meta_file = fs::File::create(&meta_path)?;
    serde_json::to_writer_pretty(meta_file, backup)?;

    println!("Backup metadata saved at {}", meta_path.display());
    Ok(())
}
