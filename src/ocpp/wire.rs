//! OCPP-Wire-Protokoll (JSON über WebSocket) – gemeinsamer Rahmen für 1.6J und 2.0.1.
//!
//! Nachrichten sind JSON-Arrays:
//!   [2, "<uniqueId>", "<Action>", <payload>]  = CALL
//!   [3, "<uniqueId>", <payload>]               = CALLRESULT
//!   [4, "<uniqueId>", "<errorCode>", "<errorDescription>", <errorDetails>] = CALLERROR

use serde_json::Value;

#[derive(Debug, Clone)]
pub enum OcppMessage {
    Call {
        unique_id: String,
        action: String,
        payload: Value,
    },
    CallResult {
        unique_id: String,
        payload: Value,
    },
    CallError {
        unique_id: String,
        error_code: String,
        error_description: String,
        error_details: Value,
    },
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("OCPP-CallError {code}: {description}")]
pub struct OcppCallError {
    pub code: String,
    pub description: String,
}

impl OcppMessage {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let arr: Vec<Value> = serde_json::from_str(raw).map_err(|e| format!("JSON: {e}"))?;
        if arr.is_empty() {
            return Err("Leeres OCPP-Array".into());
        }
        let msg_type = arr[0].as_i64().ok_or("msgType muss Zahl sein")?;
        match msg_type {
            2 => {
                if arr.len() < 4 {
                    return Err("CALL benötigt 4 Elemente".into());
                }
                Ok(Self::Call {
                    unique_id: arr[1].as_str().unwrap_or("").to_string(),
                    action: arr[2].as_str().unwrap_or("").to_string(),
                    payload: arr[3].clone(),
                })
            }
            3 => Ok(Self::CallResult {
                unique_id: arr[1].as_str().unwrap_or("").to_string(),
                payload: arr.get(2).cloned().unwrap_or(Value::Null),
            }),
            4 => Ok(Self::CallError {
                unique_id: arr[1].as_str().unwrap_or("").to_string(),
                error_code: arr[2].as_str().unwrap_or("").to_string(),
                error_description: arr[3].as_str().unwrap_or("").to_string(),
                error_details: arr.get(4).cloned().unwrap_or(Value::Null),
            }),
        _ => Err(format!("Unbekannter OCPP-msgType {msg_type}")),
        }
    }

    pub fn serialize(&self) -> String {
        match self {
            Self::Call {
                unique_id,
                action,
                payload,
            } => serde_json::to_string(&serde_json::json!([2, unique_id, action, payload])).unwrap(),
            Self::CallResult { unique_id, payload } => {
                serde_json::to_string(&serde_json::json!([3, unique_id, payload])).unwrap()
            }
            Self::CallError {
                unique_id,
                error_code,
                error_description,
                error_details,
            } => serde_json::to_string(&serde_json::json!([
                4,
                unique_id,
                error_code,
                error_description,
                error_details
            ]))
            .unwrap(),
        }
    }
}
