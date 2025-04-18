use crate::email;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub docker_parent: String,
    pub remote_backup_path: String,
    pub ssh_user: String,
    pub ssh_host: String,
    pub ssh_key: String,
    pub ssh_port: u16,
    pub email_host: String,
    pub email_port: u16,
    pub email_user: String,
    pub email_password: String,
    pub receiver_mail: String,
}

impl Config {
    fn config_path() -> PathBuf {
        dirs::home_dir()
            .expect("Could not determine home directory")
            .join(".dockup")
            .join("config.json")
    }

    pub async fn load_or_create() -> Result<Config> {
        let path = Self::config_path();

        if path.exists() {
            let data = fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&data)?)
        } else {
            println!("No configuration found. Let's create one:");
            let config = Config::interactive_create().await?;
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        fs::create_dir_all(path.parent().unwrap())?;
        let data = serde_json::to_string_pretty(self)?;
        fs::write(path, data)?;
        Ok(())
    }

    pub async fn interactive_create() -> Result<Config> {
        fn ask(prompt: &str) -> Result<String> {
            print!("{prompt}: ");
            io::stdout().flush()?;
            let mut buf = String::new();
            io::stdin().read_line(&mut buf)?;
            Ok(buf.trim().to_string())
        }

        let config = Config {
            docker_parent: ask("Docker parent directory")?,
            remote_backup_path: ask("Remote backup path")?,
            ssh_user: ask("SSH user")?,
            ssh_host: ask("SSH host")?,
            ssh_key: ask("SSH private key path")?,
            ssh_port: ask("SSH port (normally 22)")?
                .parse()
                .context("Invalid SSH port")?,
            email_host: ask("Email host")?,
            email_port: ask("Email port")?.parse().context("Invalid email port")?,
            email_user: ask("Email user")?,
            email_password: ask("Email password")?,
            receiver_mail: ask("Receiver email")?,
        };

        let test_prompt =
            ask("Would you like to test the SSH and Email configuration now? (y/n): ")?;
        if test_prompt.eq_ignore_ascii_case("y") {
            config.test_ssh().await?;
            config.test_email().await?;
        }

        Ok(config)
    }

    pub fn set_key_value(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "docker_parent" => self.docker_parent = value.to_string(),
            "remote_backup_path" => self.remote_backup_path = value.to_string(),
            "ssh_user" => self.ssh_user = value.to_string(),
            "ssh_host" => self.ssh_host = value.to_string(),
            "ssh_key" => self.ssh_key = value.to_string(),
            "ssh_port" => self.ssh_port = value.parse().context("SSH_PORT must be a number")?,
            "email_host" => self.email_host = value.to_string(),
            "email_port" => {
                self.email_port = value.parse().context("EMAIL_PORT must be a number")?
            }
            "email_user" => self.email_user = value.to_string(),
            "email_password" => self.email_password = value.to_string(),
            "receiver_mail" => self.receiver_mail = value.to_string(),
            _ => anyhow::bail!("Unknown config key: {}", key),
        }
        Ok(())
    }

    pub async fn test_ssh(&self) -> Result<()> {
        let output = std::process::Command::new("ssh")
            .arg("-i")
            .arg(&self.ssh_key)
            .arg("-p")
            .arg(self.ssh_port.to_string())
            .arg(format!("{}@{}", self.ssh_user, self.ssh_host))
            .arg("echo 'SSH connection successful'")
            .output()?;

        if output.status.success() {
            println!("✅ SSH connection successful");
        } else {
            eprintln!(
                "❌ SSH connection failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
    }
    pub async fn test_email(&self) -> Result<()> {
        email::send_test_email(self).await
    }
}
