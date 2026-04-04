use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub id: Uuid,
    pub label: String,
    pub command: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl Snippet {
    pub fn new(label: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            label: label.into(),
            command: command.into(),
            description: None,
            tags: Vec::new(),
            created_at: chrono::Utc::now(),
        }
    }
}
