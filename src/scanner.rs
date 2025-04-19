use crate::config::Config;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use yaml_rust::YamlLoader;

#[derive(Debug)]
pub struct BackupApplication {
    pub name: String,
    pub path: PathBuf,
    pub volumes: Vec<String>,
}

/// Entry point for scan
pub fn scan_projects(config: &Config) -> Result<Vec<BackupApplication>> {
    let apps = discover_projects(&config.docker_parent)?;
    for app in &apps {
        println!("\n📦 Project: {}", app.name);
        println!("   Path: {:?}", app.path);
        println!("   Volumes: {:?}", app.volumes);
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
                let volumes = parse_volumes(&compose)?;
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                projects.push(BackupApplication {
                    name,
                    path,
                    volumes,
                });
            }
        }
    }

    Ok(projects)
}

/// Parse volume mounts from a docker-compose.yml file
fn parse_volumes(path: &Path) -> Result<Vec<String>> {
    let content = fs::read_to_string(path).with_context(|| format!("Failed to read {:?}", path))?;
    let yamls = YamlLoader::load_from_str(&content)?;
    let root = &yamls[0];

    let mut volumes = Vec::new();

    if let Some(services) = root["services"].as_hash() {
        for (_, service) in services {
            if let Some(service_volumes) = service["volumes"].as_vec() {
                for vol in service_volumes {
                    if let Some(vol_str) = vol.as_str() {
                        if let Some(target) = vol_str.split(':').next() {
                            if !target.starts_with("./") && !target.contains('/') {
                                volumes.push(target.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(volumes)
}
