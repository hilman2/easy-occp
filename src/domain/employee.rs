use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Employee {
    pub id: i64,
    pub display_name: String,
    pub email: Option<String>,
    pub department: Option<String>,
    pub active: i64,
    pub created_at: String,
}
