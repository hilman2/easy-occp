use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Chip {
    pub id: i64,
    pub id_tag: String,
    pub label: Option<String>,
    pub employee_id: Option<i64>,
    pub kind: String,
    pub enabled: i64,
    pub expires_at: Option<String>,
    pub created_at: String,
}

impl Chip {
    pub fn is_valid_now(&self) -> bool {
        if self.enabled == 0 {
            return false;
        }
        if let Some(ts) = self.expires_at.as_deref() {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
                if dt < chrono::Utc::now() {
                    return false;
                }
            }
        }
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EnrollmentSession {
    pub id: i64,
    pub started_by: i64,
    pub wallbox_id: Option<i64>,
    pub started_at: String,
    pub expires_at: String,
    pub consumed: i64,
    pub captured_id_tag: Option<String>,
    pub captured_at: Option<String>,
}
