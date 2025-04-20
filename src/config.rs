use crate::email;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct RawConfig {
    pub docker_parent: Option<String>,
    pub remote_backup_path: Option<String>,
    pub ssh_user: Option<String>,
    pub ssh_host: Option<String>,
    pub ssh_key: Option<String>,
    pub ssh_port: Option<u16>,
    pub email_host: Option<String>,
    pub email_port: Option<u16>,
    pub email_user: Option<String>,
    pub email_password: Option<String>,
    pub receiver_mail: Option<String>,
    pub interval: Option<RawIntervalConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RawIntervalConfig {
    pub hour: Option<u32>,
    pub day: Option<u32>,
    pub week: Option<u32>,
    pub month: Option<u32>,
    pub year: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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
    pub interval: IntervalConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct IntervalConfig {
    pub hour: u32,
    pub day: u32,
    pub week: u32,
    pub month: u32,
    pub year: u32,
}

impl Config {
    pub fn config_path() -> PathBuf {
        dirs::home_dir()
            .expect("Could not determine home directory")
            .join(".dockup")
            .join("config.json")
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        fs::create_dir_all(path.parent().unwrap())?;
        let data = serde_json::to_string_pretty(self)?;
        fs::write(path, data)?;
        Ok(())
    }

    pub async fn load_or_create() -> Result<Self> {
        let path = Self::config_path();

        let raw: RawConfig = if path.exists() {
            let data = fs::read_to_string(&path)?;
            serde_json::from_str(&data)?
        } else {
            log::info!("No config found. Creating one.");
            RawConfig::interactive_create().await?
        };

        let finalized = raw.finalize()?;
        finalized.save()?;
        Ok(finalized)
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
            log::info!("✅ SSH connection successful");
        } else {
            log::error!(
                "❌ SSH connection failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
    }

    pub async fn test_email(&self) -> Result<()> {
        email::send_test_email(self).await
    }

    pub fn set_key_value(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "docker_parent" => self.docker_parent = value.to_string(),
            "remote_backup_path" => self.remote_backup_path = value.to_string(),
            "ssh_user" => self.ssh_user = value.to_string(),
            "ssh_host" => self.ssh_host = value.to_string(),
            "ssh_key" => self.ssh_key = value.to_string(),
            "ssh_port" => self.ssh_port = value.parse().context("Invalid value for ssh_port")?,
            "email_host" => self.email_host = value.to_string(),
            "email_port" => {
                self.email_port = value.parse().context("Invalid value for email_port")?
            }
            "email_user" => self.email_user = value.to_string(),
            "email_password" => self.email_password = value.to_string(),
            "receiver_mail" => self.receiver_mail = value.to_string(),
            "interval.hour" => {
                self.interval.hour = value.parse().context("Invalid value for interval.hour")?
            }
            "interval.day" => {
                self.interval.day = value.parse().context("Invalid value for interval.day")?
            }
            "interval.week" => {
                self.interval.week = value.parse().context("Invalid value for interval.week")?
            }
            "interval.month" => {
                self.interval.month = value.parse().context("Invalid value for interval.month")?
            }
            "interval.year" => {
                self.interval.year = value.parse().context("Invalid value for interval.year")?
            }
            _ => anyhow::bail!("Unknown config key: {}", key),
        }
        Ok(())
    }

    pub fn reset_interval_to_default(&mut self) -> Result<()> {
        self.interval = IntervalConfig {
            hour: 0,
            day: 2,
            week: 7,
            month: 4,
            year: 12,
        };
        self.save()?;
        log::info!("✅ Interval reset to default and saved to config.");
        Ok(())
    }

    pub fn suggested_cron(&self) -> Option<String> {
        if self.interval.hour > 0 {
            let interval = 60 / self.interval.hour;
            Some(format!("*/{} * * * *", interval)) // every N minutes
        } else if self.interval.day > 0 {
            let interval = 24 / self.interval.day;
            Some(format!("5 */{} * * *", interval)) // every N hours
        } else if self.interval.week > 0 {
            let interval = 7 / self.interval.week;
            let mut days = vec![];
            for i in 0..self.interval.week {
                days.push((i * interval) % 7);
            }
            Some(format!(
                "5 0 * * {}",
                days.iter()
                    .map(|d| d.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ))
        } else if self.interval.month > 0 {
            let interval = 30 / self.interval.month;
            Some(format!("5 0 1 */{} *", interval)) // every N months
        } else if self.interval.year > 0 {
            let interval = 12 / self.interval.year;
            Some(format!("5 0 1 1-12/{interval} *"))
        } else {
            None
        }
    }

    pub fn cron_human_summary(&self) -> String {
        let mut explanation = String::new();
        explanation.push_str("📦 Current Backup Retention Policy:\n");

        explanation.push_str(&format!(
            "  - Hourly backups kept: {}\n",
            self.interval.hour
        ));
        explanation.push_str(&format!("  - Daily backups kept: {}\n", self.interval.day));
        explanation.push_str(&format!(
            "  - Weekly backups kept: {}\n",
            self.interval.week
        ));
        explanation.push_str(&format!(
            "  - Monthly backups kept: {}\n",
            self.interval.month
        ));
        explanation.push_str(&format!(
            "  - Yearly backups kept: {}\n",
            self.interval.year
        ));

        explanation.push('\n');

        if let Some(cron) = self.suggested_cron() {
            explanation.push_str("🕒 Suggested cron schedule (based on finest active interval):\n");
            explanation.push_str(&format!("\n   {}\n", cron));
            explanation.push_str("\nThis schedule will ensure approximately ");
            if self.interval.hour > 0 {
                explanation.push_str(&format!("{} backups per hour.", self.interval.hour));
            } else if self.interval.day > 0 {
                explanation.push_str(&format!("{} backups per day.", self.interval.day));
            } else if self.interval.week > 0 {
                explanation.push_str(&format!("{} backups per week.", self.interval.week));
            } else if self.interval.month > 0 {
                explanation.push_str(&format!("{} backups per month.", self.interval.month));
            } else if self.interval.year > 0 {
                explanation.push_str(&format!("{} backups per year.", self.interval.year));
            }
        } else {
            explanation.push_str("⚠️  No backup interval is currently configured.\n");
        }

        explanation
    }
}

impl RawConfig {
    pub async fn interactive_create() -> Result<Self> {
        fn ask(prompt: &str) -> Result<String> {
            print!("{prompt}: ");
            io::stdout().flush()?;
            let mut buf = String::new();
            io::stdin().read_line(&mut buf)?;
            Ok(buf.trim().to_string())
        }

        fn ask_interval_value(name: &str) -> Result<u32> {
            let val = ask(&format!("  {name}:"))?;
            Ok(val
                .parse()
                .context(format!("Invalid number for interval `{name}`"))?)
        }

        let interval = match ask("Use default backup intervals? (y/n)")?.as_str() {
            "y" | "Y" => RawIntervalConfig {
                hour: Some(0),
                day: Some(2),
                week: Some(7),
                month: Some(4),
                year: Some(12),
            },
            _ => {
                println!("Enter custom backup intervals in number of days (0 = disabled):");
                RawIntervalConfig {
                    hour: Some(ask_interval_value("Hourly interval (in hours)")?),
                    day: Some(ask_interval_value("Daily interval (in days)")?),
                    week: Some(ask_interval_value("Weekly interval (in weeks)")?),
                    month: Some(ask_interval_value("Monthly interval (in months)")?),
                    year: Some(ask_interval_value("Yearly interval (in years)")?),
                }
            }
        };

        let config = RawConfig {
            docker_parent: Some(ask("Docker parent directory")?),
            remote_backup_path: Some(ask("Remote backup path")?),
            ssh_user: Some(ask("SSH user")?),
            ssh_host: Some(ask("SSH host")?),
            ssh_key: Some(ask("SSH private key path")?),
            ssh_port: Some(
                ask("SSH port (normally 22)")?
                    .parse()
                    .context("Invalid SSH port")?,
            ),
            email_host: Some(ask("Email host")?),
            email_port: Some(ask("Email port")?.parse().context("Invalid email port")?),
            email_user: Some(ask("Email user")?),
            email_password: Some(ask("Email password")?),
            receiver_mail: Some(ask("Receiver email")?),
            interval: Some(interval),
        };

        let test_prompt =
            ask("Would you like to test the SSH and Email configuration now? (y/n): ")?;
        let finalized = config.clone().finalize()?; // clone here to reuse for testing

        if test_prompt.eq_ignore_ascii_case("y") {
            finalized.test_ssh().await?;
            finalized.test_email().await?;
        }

        Ok(config)
    }

    pub fn finalize(mut self) -> Result<Config> {
        fn ask<T: std::str::FromStr>(field: &str) -> T
        where
            T::Err: std::fmt::Debug,
        {
            print!("Enter value for {}: ", field);
            io::stdout().flush().unwrap();
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            input.trim().parse::<T>().expect("Invalid input")
        }

        macro_rules! get {
            ($field:ident, $type:ty) => {
                self.$field
                    .take()
                    .unwrap_or_else(|| ask::<$type>(stringify!($field)))
            };
        }

        let interval = match self.interval.take() {
            Some(i)
                if i.hour.is_some()
                    && i.day.is_some()
                    && i.week.is_some()
                    && i.month.is_some()
                    && i.year.is_some() =>
            {
                IntervalConfig {
                    hour: i.hour.unwrap(),
                    day: i.day.unwrap(),
                    week: i.week.unwrap(),
                    month: i.month.unwrap(),
                    year: i.year.unwrap(),
                }
            }
            _ => {
                println!("Interval is incomplete or missing. Use default? (y/n)");
                let mut answer = String::new();
                io::stdin().read_line(&mut answer)?;
                if answer.trim().eq_ignore_ascii_case("y") {
                    IntervalConfig {
                        hour: 0,
                        day: 2,
                        week: 7,
                        month: 4,
                        year: 12,
                    }
                } else {
                    IntervalConfig {
                        hour: ask("interval.hour"),
                        day: ask("interval.day"),
                        week: ask("interval.week"),
                        month: ask("interval.month"),
                        year: ask("interval.year"),
                    }
                }
            }
        };

        Ok(Config {
            docker_parent: get!(docker_parent, String),
            remote_backup_path: get!(remote_backup_path, String),
            ssh_user: get!(ssh_user, String),
            ssh_host: get!(ssh_host, String),
            ssh_key: get!(ssh_key, String),
            ssh_port: get!(ssh_port, u16),
            email_host: get!(email_host, String),
            email_port: get!(email_port, u16),
            email_user: get!(email_user, String),
            email_password: get!(email_password, String),
            receiver_mail: get!(receiver_mail, String),
            interval,
        })
    }
}
