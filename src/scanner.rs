use crate::config::Config;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use yaml_rust::YamlLoader;

#[derive(Serialize, Deserialize, Debug)]
pub struct Volume {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum BackupType {
    Manual,
    Scheduled,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct BackupApplication {
    pub name: String,
    pub timestamp: chrono::DateTime<chrono::Local>,
    pub backup_type: Option<BackupType>,
    pub application_path: PathBuf,
    pub volumes: Vec<Volume>,
}

/// Entry point for scan
pub fn scan_projects(config: &Config) -> Result<Vec<BackupApplication>> {
    let apps = discover_projects(&config.docker_parent)?;
    for app in &apps {
        log::info!("📦 Project: {}", app.name);
        log::info!("   Path: {:?}", app.application_path);
        log::info!("   Volumes:");
        app.volumes.iter().for_each(|volume| {
            log::info!("      - Name: {}, Path: {:?}", volume.name, volume.path);
        });
    }
    Ok(apps)
}

/// Discover valid backup projects
fn discover_projects(base: &str) -> Result<Vec<BackupApplication>> {
    let mut projects = Vec::new();

    for entry in fs::read_dir(base)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let compose = path.join("docker-compose.yml");
            if compose.exists() {
                let volumes = parse_volumes(&compose, &path)?;
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                projects.push(BackupApplication {
                    name,
                    timestamp: chrono::Local::now(),
                    backup_type: None,
                    application_path: path.clone(),
                    volumes: volumes,
                });
            }
        }
    }

    Ok(projects)
}

/// Parse volume mounts from a docker-compose.yml file
use std::collections::HashSet;

/// Parses a Docker Compose file and extracts unique volume host paths,
/// resolving them relative to the given `app_root`.
pub fn parse_volumes(compose_file: &Path, app_root: &Path) -> Result<Vec<Volume>> {
    let content = fs::read_to_string(compose_file)
        .with_context(|| format!("Failed to read {:?}", compose_file))?;
    let yamls = YamlLoader::load_from_str(&content)?;
    let root = &yamls[0];

    let mut volumes = Vec::new();
    let mut seen = HashSet::new();

    if let Some(services) = root["services"].as_hash() {
        for (_, service) in services {
            if let Some(service_volumes) = service["volumes"].as_vec() {
                for vol in service_volumes {
                    if let Some(vol_str) = vol.as_str() {
                        if let Some((host_path, _)) = vol_str.split_once(':') {
                            if seen.insert(host_path) {
                                let abs_path = if host_path.starts_with('/') {
                                    PathBuf::from(host_path)
                                } else {
                                    app_root.join(host_path)
                                };

                                volumes.push(Volume {
                                    name: host_path.to_string(),
                                    path: abs_path,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(volumes)
}
