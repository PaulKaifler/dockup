use anyhow::{Context, Result};
use std::process::Command;

use crate::config::Config;

pub fn run_remote_cmd_with_output(cfg: &Config, cmd: &str) -> Result<String> {
    let full_cmd = format!(
        "ssh -i {} -p {} {}@{} '{}'",
        cfg.ssh_key, cfg.ssh_port, cfg.ssh_user, cfg.ssh_host, cmd
    );

    let output = Command::new("sh")
        .arg("-c")
        .arg(&full_cmd)
        .output()
        .with_context(|| format!("Failed to run: {}", full_cmd))?;

    if !output.status.success() {
        anyhow::bail!(
            "SSH command failed: {}\nstderr: {}",
            cmd,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
