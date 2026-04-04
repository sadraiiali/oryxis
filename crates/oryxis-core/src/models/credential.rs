use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    pub id: Uuid,
    pub label: String,
    pub username: Option<String>,
    pub notes: Option<String>,
    pub tags: Vec<String>,
}

impl Credential {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            label: label.into(),
            username: None,
            notes: None,
            tags: Vec::new(),
        }
    }
}
