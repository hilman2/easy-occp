use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub role: String,
    pub auth_source: String,
    pub password_hash: Option<String>,
    pub external_id: Option<String>,
    pub disabled: i64,
    pub created_at: String,
}

impl User {
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }
    pub fn is_disabled(&self) -> bool {
        self.disabled != 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Session {
    pub token: String,
    pub user_id: i64,
    pub created_at: String,
    pub expires_at: String,
}

pub fn session_expires_at() -> DateTime<Utc> {
    Utc::now() + chrono::Duration::hours(12)
}
