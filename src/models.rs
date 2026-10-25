use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub imap_server: String,
    pub imap_port: u16,
    pub smtp_server: String,
    pub smtp_port: u16,
    pub user: String,
    pub pass: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            imap_server: "imap.gmail.com".to_string(),
            imap_port: 993,
            smtp_server: "smtp.gmail.com".to_string(),
            smtp_port: 465,
            user: "".to_string(),
            pass: "".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailMessage {
    pub uid: u32,
    pub from: String,
    pub subject: String,
    pub body: String,
    pub date: String,
}

#[derive(Debug, thiserror::Error)]
pub enum MailError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("Authentication failed")]
    AuthFailed,
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Configuration error: {0}")]
    Config(String),
}
