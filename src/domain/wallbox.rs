use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Wallbox {
    pub id: i64,
    pub charge_point_id: String,
    pub name: String,
    pub location: Option<String>,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub firmware: Option<String>,
    pub serial_number: Option<String>,
    pub ocpp_version: Option<String>,
    pub auth_basic_user: Option<String>,
    pub auth_basic_pass: Option<String>,
    pub last_heartbeat: Option<String>,
    pub last_boot: Option<String>,
    pub connector_count: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Connector {
    pub id: i64,
    pub wallbox_id: i64,
    pub connector_id: i64,
    pub status: Option<String>,
    pub error_code: Option<String>,
    pub info: Option<String>,
    pub updated_at: String,
}
