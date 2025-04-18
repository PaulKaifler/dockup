use crate::config::Config;
use anyhow::Result;
use lettre::{
    message::{Mailbox, Message},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
};

/// Send summary email after backup job
pub async fn send_summary_email(cfg: &Config, subject: &str, body: &str) -> Result<()> {
    let email = Message::builder()
        .from(cfg.email_user.parse::<Mailbox>()?)
        .to(cfg.receiver_mail.parse::<Mailbox>()?)
        .subject(subject)
        .body(body.to_string())?;

    let creds = Credentials::new(cfg.email_user.clone(), cfg.email_password.clone());

    let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay(&cfg.email_host)?
        .port(cfg.email_port)
        .credentials(creds)
        .build();

    match mailer.send(email).await {
        Ok(_) => println!("✅ Email sent to {}", cfg.receiver_mail),
        Err(e) => eprintln!("❌ Failed to send email: {e}"),
    }

    Ok(())
}
