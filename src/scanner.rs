use crate::config::Config;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use yaml_rust::YamlLoader;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Volume {
    pub name: String,
    pub path: PathBuf,
    pub volume_type: VolumeType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum VolumeType {
    Bind,
    Mount,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum BackupType {
    Manual,
    Scheduled,
}

impl std::fmt::Display for BackupType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackupType::Manual => write!(f, "Manual"),
            BackupType::Scheduled => write!(f, "Scheduled"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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
                                let is_bind = host_path.starts_with('/')
                                    || host_path.starts_with("./")
                                    || host_path.starts_with("../");

                                let resolved_path = if is_bind {
                                    if host_path.starts_with('/') {
                                        PathBuf::from(host_path)
                                    } else {
                                        app_root.join(host_path)
                                    }
                                } else {
                                    // If it's not a bind mount, use dummy path for completeness
                                    PathBuf::from(format!("/var/lib/docker/volumes/{}", host_path))
                                };

                                volumes.push(Volume {
                                    name: host_path.to_string(),
                                    path: resolved_path,
                                    volume_type: if is_bind {
                                        VolumeType::Bind
                                    } else {
                                        VolumeType::Mount
                                    },
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
