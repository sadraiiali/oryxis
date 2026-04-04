use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub id: Uuid,
    pub label: String,
    pub hostname: String,
    pub port: u16,
    pub username: Option<String>,
    pub auth_method: AuthMethod,
    pub key_id: Option<Uuid>,
    pub group_id: Option<Uuid>,
    pub jump_chain: Vec<Uuid>,
    pub proxy: Option<ProxyConfig>,
    pub tags: Vec<String>,
    pub notes: Option<String>,
    pub color: Option<String>,
    pub last_used: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Connection {
    pub fn new(label: impl Into<String>, hostname: impl Into<String>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4(),
            label: label.into(),
            hostname: hostname.into(),
            port: 22,
            username: None,
            auth_method: AuthMethod::Password,
            key_id: None,
            group_id: None,
            jump_chain: Vec::new(),
            proxy: None,
            tags: Vec::new(),
            notes: None,
            color: None,
            last_used: None,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthMethod {
    Password,
    Key,
    Agent,
    Interactive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub proxy_type: ProxyType,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProxyType {
    Socks5,
    Socks4,
    Http,
    Command(String),
}
